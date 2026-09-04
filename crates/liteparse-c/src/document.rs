use std::sync::{Arc, OnceLock};

use liteparse::conversion::{PdfInputGuard, resolve_pdf_input};
use liteparse::ocr::OcrEngine;
use liteparse::ocr_merge::PageComplexityStats;
use liteparse::types::{OutlineTarget, PdfInput};
use liteparse::{GlyphResolver, LiteParseConfig as CoreConfig, ParseResult};
use liteparse_pdfium::Library;

use crate::complexity::{ComplexityState, LiteParseComplexityNew};
use crate::handle::{
    LiteParseByteView, as_slice, build_handle, copy_array, free_handle, opaque_handles,
    required_view_str, slice_out, state_ref,
};
use crate::parser::{LiteParseParser, ParserState, build_parser};
use crate::render::{RenderRequest, load_document, render_pages};
use crate::result::{LiteParseResultNew, ResultState};
use crate::runtime::block_on;
use crate::screenshots::{LiteParseScreenshotsNew, ScreenshotsState};
use crate::status::{FfiError, FfiResult, LiteParseStatus};
use crate::views::{LiteParseOutlineEntry, views};

/// Page region in top-left-origin viewport points. Must fit within the page.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseRenderRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

unsafe fn copy_page_numbers(pages: *const u32, len: usize) -> FfiResult<Option<Vec<u32>>> {
    Ok(unsafe { copy_array(pages, len, "pages") }?.filter(|pages| !pages.is_empty()))
}

unsafe fn copy_input_bytes(data: *const u8, len: usize) -> FfiResult<Vec<u8>> {
    Ok(unsafe { as_slice(data, len, "data") }?
        .unwrap_or_default()
        .to_vec())
}

const MAX_SELECTED_PAGES: usize = 100_000;

/// A document opened once for many operations. Operations may run
/// concurrently on one handle; destruction must wait for them.
pub struct LiteParseDocument {
    _opaque: [u8; 0],
}

/// The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseDocumentNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseDocument,
}

opaque_handles! {
    LiteParseDocument => DocumentState, "document";
}

pub(crate) struct DocumentState {
    config: CoreConfig,
    // Snapshots keep the document independent of its parser.
    glyph_resolver: Option<Arc<dyn GlyphResolver>>,
    ocr_engine: Option<Arc<dyn OcrEngine>>,
    input: PdfInput,
    guard: PdfInputGuard,
    total_pages: u32,
    outline: Vec<OutlineTarget>,
    outline_views: OnceLock<Vec<LiteParseOutlineEntry>>,
    /// From the source PDF at open; absent for converted inputs.
    descriptive: Option<DescriptiveInfo>,
}

#[derive(Clone, Default)]
pub(crate) struct DescriptiveInfo {
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) subject: Option<String>,
    pub(crate) keywords: Option<String>,
    pub(crate) trapped: Option<String>,
}

impl DescriptiveInfo {
    fn read(document: &liteparse_pdfium::Document<'_>) -> Self {
        Self {
            title: document.meta_text("Title"),
            author: document.meta_text("Author"),
            subject: document.meta_text("Subject"),
            keywords: document.meta_text("Keywords"),
            trapped: document.meta_text("Trapped"),
        }
    }
}

// SAFETY: cached pointers target immutable `outline` storage owned by this state.
unsafe impl Send for DocumentState {}
unsafe impl Sync for DocumentState {}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DocumentState>();
};

impl DocumentState {
    fn open(parser: &ParserState, input: PdfInput) -> FfiResult<Self> {
        let config = parser.config().clone();
        let password = config.password.clone();
        let (input, guard) = block_on(resolve_pdf_input(input, password.as_deref(), false))??;
        let want_descriptive = config.extract_document_metadata && !guard.is_converted();
        let (total_pages, outline, descriptive) = {
            let lib = Library::init();
            let document = load_document(&lib, &input, password.as_deref())?;
            (
                document.page_count().max(0) as u32,
                liteparse::extract::extract_outline(&document),
                want_descriptive.then(|| DescriptiveInfo::read(&document)),
            )
        };
        Ok(Self {
            config,
            glyph_resolver: parser.glyph_resolver(),
            ocr_engine: parser.ocr_engine(),
            input,
            guard,
            total_pages,
            outline,
            outline_views: OnceLock::new(),
            descriptive,
        })
    }

    fn selection(&self, pages: Option<Vec<u32>>) -> FfiResult<Option<Vec<u32>>> {
        let Some(mut pages) = pages else {
            return Ok(None);
        };
        if let Some(bad) = pages
            .iter()
            .find(|page| **page < 1 || **page > self.total_pages)
        {
            return Err(FfiError::invalid_argument(format!(
                "page {bad} is out of range (document has {} pages)",
                self.total_pages
            )));
        }
        pages.sort_unstable();
        pages.dedup();
        if pages.len() > MAX_SELECTED_PAGES {
            return Err(FfiError::invalid_argument(format!(
                "selection of {} pages exceeds the limit of {MAX_SELECTED_PAGES}; \
                 parse in several calls",
                pages.len()
            )));
        }
        Ok(Some(pages))
    }

    fn parser_for(&self, pages: Option<&[u32]>) -> liteparse::LiteParse {
        let mut config = self.config.clone();
        config.target_pages = pages.map(|pages| {
            pages
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        });
        build_parser(config, self.ocr_engine.clone(), self.glyph_resolver.clone())
    }

    fn parse(&self, pages: Option<Vec<u32>>) -> FfiResult<ParseResult> {
        let parser = self.parser_for(self.selection(pages)?.as_deref());
        let mut result = block_on(parser.parse_input(self.input.clone()))??;
        if self.guard.is_converted() {
            result.doc_meta = None;
        }
        Ok(result)
    }

    fn complexity(&self, pages: Option<Vec<u32>>) -> FfiResult<Vec<PageComplexityStats>> {
        let parser = self.parser_for(self.selection(pages)?.as_deref());
        Ok(block_on(parser.is_complex(self.input.clone()))??)
    }

    fn screenshot(
        &self,
        pages: Option<Vec<u32>>,
        dpi_override: f32,
        region: Option<LiteParseRenderRegion>,
    ) -> FfiResult<ScreenshotsState> {
        let request = RenderRequest::from_config(&self.config, dpi_override, region)?;
        let pages = self.selection(pages)?;
        render_pages(&self.input, pages.as_deref(), &request).map(ScreenshotsState::new)
    }

    fn outline_views(&self) -> &[LiteParseOutlineEntry] {
        self.outline_views.get_or_init(|| views(&self.outline))
    }
}

/// Open a path, converting non-PDF input once for the document's lifetime.
///
/// # Safety
///
/// `parser` must be live and `path` readable UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_open_path(
    parser: *const LiteParseParser,
    path: LiteParseByteView,
) -> LiteParseDocumentNew {
    let (status, handle) = build_handle(|| unsafe {
        let parser = state_ref(parser)?;
        let path = required_view_str(path, "path")?;
        DocumentState::open(parser, PdfInput::Path(path))
    });
    LiteParseDocumentNew { status, handle }
}

/// Open and copy in-memory input. Prefer paths for large documents.
///
/// # Safety
///
/// `parser` must be live; `data` must be readable, or null with zero length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_open_bytes(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
) -> LiteParseDocumentNew {
    let (status, handle) = build_handle(|| unsafe {
        let parser = state_ref(parser)?;
        let bytes = copy_input_bytes(data, data_len)?;
        DocumentState::open(parser, PdfInput::Bytes(bytes))
    });
    LiteParseDocumentNew { status, handle }
}

/// Destroy a document handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_free(document: *mut LiteParseDocument) {
    unsafe { free_handle(document) };
}

/// Return the source page count recorded at open.
///
/// # Safety
///
/// `document` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_total_pages(document: *const LiteParseDocument) -> u32 {
    unsafe { state_ref(document) }.map_or(0, |state| state.total_pages)
}

/// Return whether the source was converted to PDF.
///
/// # Safety
///
/// `document` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_is_converted(
    document: *const LiteParseDocument,
) -> bool {
    unsafe { state_ref(document) }.is_ok_and(|state| state.guard.is_converted())
}

/// Borrow the document outline.
///
/// # Safety
///
/// `document` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_outline(
    document: *const LiteParseDocument,
    out_len: *mut usize,
) -> *const LiteParseOutlineEntry {
    unsafe { slice_out(out_len, || Ok(Some(state_ref(document)?.outline_views()))) }
}

/// Parse sorted, unique, 1-based pages. Null with zero length selects all.
///
/// # Safety
///
/// `document` must be live and `pages` readable, or null with zero length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_parse(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
) -> LiteParseResultNew {
    let (status, handle) = build_handle(|| unsafe {
        let state = state_ref(document)?;
        let pages = copy_page_numbers(pages, pages_len)?;
        let result = state.parse(pages)?;
        let descriptive = result
            .doc_meta
            .is_some()
            .then(|| state.descriptive.clone())
            .flatten();
        Ok(ResultState::new(
            result,
            state.config.extract_text_metadata,
            state.config.dpi,
            descriptive,
        ))
    });
    LiteParseResultNew { status, handle }
}

/// Render selected pages to PNG. Zero DPI uses the configured value; region
/// rectangles are clipped and made region-relative.
///
/// # Safety
///
/// `document` must be live; non-null inputs must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_screenshot(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    dpi_override: f32,
    region: *const LiteParseRenderRegion,
) -> LiteParseScreenshotsNew {
    let (status, handle) = build_handle(|| unsafe {
        let state = state_ref(document)?;
        let pages = copy_page_numbers(pages, pages_len)?;
        state.screenshot(pages, dpi_override, region.as_ref().copied())
    });
    LiteParseScreenshotsNew { status, handle }
}

/// Compute complexity for selected pages; null with zero length selects all.
///
/// # Safety
///
/// `document` must be live and `pages` readable, or null with zero length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_complexity(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
) -> LiteParseComplexityNew {
    let (status, handle) = build_handle(|| unsafe {
        let state = state_ref(document)?;
        let pages = copy_page_numbers(pages, pages_len)?;
        state.complexity(pages).map(ComplexityState::new)
    });
    LiteParseComplexityNew { status, handle }
}
