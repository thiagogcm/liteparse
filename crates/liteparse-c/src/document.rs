use std::sync::{Arc, OnceLock};

use liteparse::conversion::{PdfInputGuard, resolve_pdf_input};
use liteparse::ocr::OcrEngine;
use liteparse::ocr_merge::PageComplexityStats;
use liteparse::types::PdfInput;
use liteparse::{GlyphResolver, LiteParseConfig as CoreConfig, ParseResult};
use liteparse_pdfium::{Document, Library};

use crate::complexity::{ComplexityState, LiteParseComplexity};
use crate::extract::extract_pages;
use crate::handle::{
    Pool, array_ptr, as_slice, copy_array, create_handle, free_handle, opaque_handles,
    required_str, state_ref, view_of, view_state,
};
use crate::page_objects::{LiteParsePageObjects, extract_page_objects};
use crate::parser::{LiteParseParser, ParserState, build_parser};
use crate::raw_text::{LiteParseRawText, extract_raw_text};
use crate::records::{DescriptiveInfo, LiteParseOutlineEntry, pack_all};
use crate::render::{RenderRequest, load_document, page_facts, render_pages};
use crate::result::{LiteParseResult, PageGeometries, ResultState};
use crate::runtime::block_on;
use crate::screenshots::{LiteParseScreenshots, ScreenshotsState};
use crate::status::{FfiError, FfiResult, LiteParseStatus};

/// Page region in top-left-origin viewport points. Must fit within the page.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseRenderRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// `LiteParseDocumentInfo.flags` bit: the source was converted to PDF (an
/// office document or image).
pub const LITEPARSE_DOCUMENT_FLAG_CONVERTED: u32 = 1 << 0;

/// Facts recorded when a document is opened. Borrowed until the document is
/// freed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseDocumentInfo {
    pub total_pages: u32,
    /// `LITEPARSE_DOCUMENT_FLAG_*` bits.
    pub flags: u32,
    /// String pool behind the outline titles.
    pub pool: *const u8,
    pub pool_len: usize,
    /// Bookmarks, walked once at open.
    pub outline: *const LiteParseOutlineEntry,
    pub outline_len: usize,
}

unsafe fn copy_page_numbers(pages: *const u32, len: usize) -> FfiResult<Option<Vec<u32>>> {
    Ok(unsafe { copy_array(pages, len, "pages") }?.filter(|pages| !pages.is_empty()))
}

const MAX_SELECTED_PAGES: usize = 100_000;

/// A document opened once for many operations. Operations may run
/// concurrently on one handle; destruction must wait for them.
pub struct LiteParseDocument {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseDocument => DocumentState, "document";
}

pub(crate) struct DocumentState {
    pub(crate) config: CoreConfig,
    // Snapshots keep the document independent of its parser.
    pub(crate) glyph_resolver: Option<Arc<dyn GlyphResolver>>,
    ocr_engine: Option<Arc<dyn OcrEngine>>,
    pub(crate) input: PdfInput,
    guard: PdfInputGuard,
    total_pages: u32,
    /// Backing storage for `info`.
    #[allow(dead_code)]
    pool: Pool,
    #[allow(dead_code)]
    outline_entries: Vec<LiteParseOutlineEntry>,
    info: LiteParseDocumentInfo,
    /// From the source PDF at open; absent for converted inputs.
    descriptive: Option<DescriptiveInfo>,
    /// Per-page geometry after orientation corrections, indexed by page
    /// number - 1. Filled on first use; the document is reopened at most once.
    geometries: OnceLock<PageGeometries>,
}

view_state!(DocumentState => LiteParseDocumentInfo, info);

impl DescriptiveInfo {
    fn read(document: &Document<'_>) -> Self {
        Self {
            title: document.meta_text("Title"),
            author: document.meta_text("Author"),
            subject: document.meta_text("Subject"),
            keywords: document.meta_text("Keywords"),
            trapped: document.meta_text("Trapped"),
        }
    }
}

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
        let mut pool = Pool::default();
        let outline_entries = pack_all(&mut pool, &outline, LiteParseOutlineEntry::pack);
        let info = LiteParseDocumentInfo {
            total_pages,
            flags: if guard.is_converted() {
                LITEPARSE_DOCUMENT_FLAG_CONVERTED
            } else {
                0
            },
            pool: pool.ptr(),
            pool_len: pool.len(),
            outline: array_ptr(&outline_entries),
            outline_len: outline_entries.len(),
        };
        Ok(Self {
            config,
            glyph_resolver: parser.glyph_resolver(),
            ocr_engine: parser.ocr_engine(),
            input,
            guard,
            total_pages,
            pool,
            outline_entries,
            info,
            descriptive,
            geometries: OnceLock::new(),
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
        render_pages(&self.input, pages.as_deref(), &request, &self.config)
            .map(ScreenshotsState::new)
    }

    /// Geometry of every page, read from `document` (already corrected) or
    /// from a fresh corrected open when no document is at hand.
    pub(crate) fn geometries(&self, document: Option<&Document<'_>>) -> &PageGeometries {
        self.geometries.get_or_init(|| {
            let read = |document: &Document<'_>| {
                (0..self.total_pages as i32)
                    .map(|index| {
                        document
                            .page(index)
                            .ok()
                            .map(|page| page_facts(&page).geometry)
                    })
                    .map(Option::flatten)
                    .collect()
            };
            if let Some(document) = document {
                return read(document);
            }
            let lib = Library::init();
            let opened = load_document(&lib, &self.input, self.config.password.as_deref())
                .and_then(|document| {
                    liteparse::extract::apply_page_orientation_corrections(
                        &document,
                        &self.config.page_orientation_corrections,
                    )?;
                    Ok(document)
                });
            match opened {
                Ok(document) => read(&document),
                Err(_) => vec![None; self.total_pages as usize],
            }
        })
    }

    /// Geometries parallel to `page_numbers` (1-based).
    pub(crate) fn geometries_for(
        &self,
        document: Option<&Document<'_>>,
        page_numbers: impl Iterator<Item = usize>,
    ) -> PageGeometries {
        let all = self.geometries(document);
        page_numbers
            .map(|number| all.get(number.wrapping_sub(1)).copied().flatten())
            .collect()
    }
}

/// Open a path, converting non-PDF input once for the document's lifetime.
/// `path` is `path_len` bytes of UTF-8, not NUL-terminated.
///
/// `parser` must be live, `path` readable for `path_len` bytes, and `out`
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_open_path(
    parser: *const LiteParseParser,
    path: *const u8,
    path_len: usize,
    out: *mut *mut LiteParseDocument,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let parser = state_ref(parser)?;
            let path = required_str(path, path_len, "path")?.to_owned();
            DocumentState::open(parser, PdfInput::Path(path))
        })
    }
}

/// Open and copy in-memory input. Prefer paths for large documents: bytes
/// are copied at open and again per parse.
///
/// `parser` must be live; `data` must be readable, or null with zero
/// length; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_open_bytes(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
    out: *mut *mut LiteParseDocument,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let parser = state_ref(parser)?;
            let bytes = as_slice(data, data_len, "data")?
                .unwrap_or_default()
                .to_vec();
            DocumentState::open(parser, PdfInput::Bytes(bytes))
        })
    }
}

/// Destroy a document handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_free(document: *mut LiteParseDocument) {
    unsafe { free_handle(document) };
}

/// Borrow the facts recorded at open; null for a null handle.
///
/// `document` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_info(
    document: *const LiteParseDocument,
) -> *const LiteParseDocumentInfo {
    unsafe { view_of(document) }
}

/// Parse 1-based pages; null with zero length selects all. Selections are
/// validated against the page count, de-duplicated, and processed in
/// ascending order. `max_pages` caps either form.
///
/// `document` must be live, `pages` readable or null with zero length, and
/// `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_parse(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    out: *mut *mut LiteParseResult,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = copy_page_numbers(pages, pages_len)?;
            let result = state.parse(pages)?;
            let descriptive = result
                .doc_meta
                .is_some()
                .then(|| state.descriptive.clone())
                .flatten();
            let geometries =
                state.geometries_for(None, result.pages.iter().map(|page| page.page_number));
            Ok(ResultState::parsed(
                result,
                &state.config,
                descriptive,
                geometries,
            ))
        })
    }
}

/// Extract pre-projection pages: heuristic text items, graphics, and the
/// configured extras, with no projection, OCR, text, or Markdown. Link
/// stamping and word boxes follow the same rules as parse (links only under
/// Markdown; word boxes when requested or under Markdown). Feed the view's
/// `content` to `liteparse_parser_parse_content` to project and classify.
///
/// See `liteparse_document_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_extract(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    out: *mut *mut LiteParseResult,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = state.selection(copy_page_numbers(pages, pages_len)?)?;
            extract_pages(state, pages)
        })
    }
}

/// Render selected pages to PNG. Zero DPI uses the configured value. A
/// non-null `region` crops each page; detected rectangles are then clipped
/// and made region-relative. The whole page is rasterized before cropping.
///
/// `document` must be live; non-null inputs must be readable; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_screenshot(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    dpi_override: f32,
    region: *const LiteParseRenderRegion,
    out: *mut *mut LiteParseScreenshots,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = copy_page_numbers(pages, pages_len)?;
            state.screenshot(pages, dpi_override, region.as_ref().copied())
        })
    }
}

/// Compute pre-OCR complexity signals for selected pages.
///
/// See `liteparse_document_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_complexity(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    out: *mut *mut LiteParseComplexity,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = copy_page_numbers(pages, pages_len)?;
            state.complexity(pages).map(ComplexityState::new)
        })
    }
}

/// Extract heuristic-free PDFium text runs: no gap merge, projection, OCR,
/// or Markdown; every glyph lands in exactly one item.
///
/// See `liteparse_document_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_raw_text(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    out: *mut *mut LiteParseRawText,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = state.selection(copy_page_numbers(pages, pages_len)?)?;
            extract_raw_text(state, pages)
        })
    }
}

/// Snapshot unfiltered page content objects: kinds, matrices, PDFium y-up
/// bounds, form children, path segments, and image metadata. No size
/// filters, viewport transforms, or form-matrix composition. `flags` is a
/// mask of `LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_*`; unknown bits are
/// `LITEPARSE_STATUS_INVALID_ARGUMENT`.
///
/// See `liteparse_document_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_page_objects(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    flags: u32,
    out: *mut *mut LiteParsePageObjects,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = state.selection(copy_page_numbers(pages, pages_len)?)?;
            extract_page_objects(state, pages, flags)
        })
    }
}
