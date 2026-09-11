use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use liteparse::conversion::{PdfInputGuard, resolve_pdf_input};
use liteparse::stages;
use liteparse::types::{DocumentMetadata, OutlineTarget, PdfInput};
use liteparse::{GlyphResolver, LiteParse as CoreLiteParse, LiteParseConfig as CoreConfig};
use liteparse_pdfium::{Document, Library};

use crate::complexity::complexity;
use crate::complexity::{ComplexityState, LiteParseComplexity};
use crate::extract::extract_pages;
use crate::handle::{
    Pool, array_ptr, as_slice, create_handle, free_handle, opaque_handles, required_str, state_ref,
    view_of, view_state,
};
use crate::job::Job;
use crate::page_objects::{LiteParsePageObjects, extract_page_objects};
use crate::parser::{LiteParseParser, OcrEngineChoice, ParserState, build_parser};
use crate::raw_text::{LiteParseRawText, extract_raw_text};
use crate::records::{DescriptiveInfo, LiteParseOutlineEntry, pack_all};
use crate::render::{RenderRequest, render_pages};
use crate::result::{LiteParseResult, ResultState};
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

pub(crate) unsafe fn copy_page_numbers(
    pages: *const u32,
    len: usize,
) -> FfiResult<Option<Vec<u32>>> {
    let pages = unsafe { as_slice(pages, len, "pages") }?;
    check_selected_pages(len, "selection")?;
    Ok(pages.filter(|pages| !pages.is_empty()).map(<[u32]>::to_vec))
}

pub(crate) fn check_selected_pages(len: usize, name: &str) -> FfiResult {
    if len > LITEPARSE_MAX_SELECTED_PAGES as usize {
        return Err(FfiError::resource_limit(format!(
            "{name} of {len} pages exceeds the limit of {LITEPARSE_MAX_SELECTED_PAGES}"
        )));
    }
    Ok(())
}

/// Maximum number of explicit page numbers accepted in one operation.
pub const LITEPARSE_MAX_SELECTED_PAGES: u32 = 100_000;

/// A document opened once for many operations. Operations may run
/// concurrently on one handle; destruction must wait for them.
pub struct LiteParseDocument {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseDocument => DocumentState, "document";
}

/// The prepared source and the facts read at open, shared by the document
/// and its jobs.
pub(crate) struct Source {
    pub(crate) config: CoreConfig,
    // Snapshots keep the document independent of its parser.
    pub(crate) glyph_resolver: Option<Arc<dyn GlyphResolver>>,
    /// The parser's configured OCR engine choice.
    pub(crate) ocr_engine: OcrEngineChoice,
    pub(crate) input: PdfInput,
    guard: PdfInputGuard,
    pub(crate) total_pages: u32,
    pub(crate) outline: Vec<OutlineTarget>,
    /// From the source PDF at open; absent for converted inputs.
    pub(crate) descriptive: Option<DescriptiveInfo>,
    /// Provenance of the source file, read at open when requested; absent
    /// for converted inputs.
    pub(crate) doc_meta: Option<DocumentMetadata>,
    /// The AcroForm-repaired copy, when repair rewrote the source; made on
    /// first use by an operation that extracts form fields.
    repaired: OnceLock<Option<PdfInput>>,
}

impl Source {
    /// Convert `input` to PDF once and read the facts every operation
    /// reuses: page count, outline, and descriptive metadata.
    pub(crate) fn prepare(
        config: CoreConfig,
        glyph_resolver: Option<Arc<dyn GlyphResolver>>,
        ocr_engine: OcrEngineChoice,
        input: PdfInput,
    ) -> FfiResult<Self> {
        let password = config.password.as_deref();
        let (input, guard) = block_on(resolve_pdf_input(input, password, false))??;
        let want_metadata = config.extract_document_metadata && !guard.is_converted();
        let (total_pages, outline, descriptive, doc_meta) = {
            let lib = Library::try_init()?;
            let document =
                stages::open(&lib, &input, password, &config.page_orientation_corrections)?;
            (
                document.page_count().max(0) as u32,
                stages::outline(&document),
                want_metadata.then(|| DescriptiveInfo::read(&document)),
                want_metadata.then(|| stages::document_metadata(&input, &document)),
            )
        };
        Ok(Self {
            config,
            glyph_resolver,
            ocr_engine,
            input,
            guard,
            total_pages,
            outline,
            descriptive,
            doc_meta,
            repaired: OnceLock::new(),
        })
    }

    /// Reject pages outside the document; sort and de-duplicate the rest.
    pub(crate) fn selection(&self, pages: Option<Vec<u32>>) -> FfiResult<Option<Vec<u32>>> {
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
        Ok(Some(pages))
    }

    /// The input extraction reads: the AcroForm-repaired copy when form
    /// fields are extracted and repair rewrote the source, else the source.
    pub(crate) fn extraction_input(&self, lib: &Library) -> &PdfInput {
        if !self.config.extract_form_fields {
            return &self.input;
        }
        self.repaired
            .get_or_init(|| {
                stages::repair_acroform(lib, &self.input, self.config.password.as_deref())
            })
            .as_ref()
            .unwrap_or(&self.input)
    }

    #[cfg(test)]
    pub(crate) fn with_ocr_engine(mut self, engine: Arc<dyn liteparse::ocr::OcrEngine>) -> Self {
        self.ocr_engine = Ok(Some(engine));
        self
    }

    pub(crate) fn is_converted(&self) -> bool {
        self.guard.is_converted()
    }

    /// The core parser whose configuration derives every stage's options.
    pub(crate) fn core(&self) -> CoreLiteParse {
        let engine = self.ocr_engine.as_ref().ok().cloned().flatten();
        build_parser(self.config.clone(), engine, self.glyph_resolver.clone())
    }

    /// Open `input` (the source or its repaired copy) with the configured
    /// password and orientation corrections.
    pub(crate) fn open<'lib>(
        &self,
        lib: &'lib Library,
        input: &PdfInput,
    ) -> FfiResult<Document<'lib>> {
        Ok(stages::open(
            lib,
            input,
            self.config.password.as_deref(),
            &self.config.page_orientation_corrections,
        )?)
    }
}

pub(crate) struct DocumentState {
    source: Arc<Source>,
    /// Backing storage for `info`.
    #[allow(dead_code)]
    pool: Pool,
    #[allow(dead_code)]
    outline_entries: Vec<LiteParseOutlineEntry>,
    info: LiteParseDocumentInfo,
}

view_state!(DocumentState => LiteParseDocumentInfo, info);

impl Deref for DocumentState {
    type Target = Source;
    fn deref(&self) -> &Source {
        &self.source
    }
}

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
        let source = Source::prepare(
            parser.config().clone(),
            parser.glyph_resolver(),
            parser.ocr_engine(),
            input,
        )?;
        let mut pool = Pool::default();
        let outline_entries = pack_all(&mut pool, &source.outline, LiteParseOutlineEntry::pack);
        let info = LiteParseDocumentInfo {
            total_pages: source.total_pages,
            flags: if source.is_converted() {
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
            source: Arc::new(source),
            pool,
            outline_entries,
            info,
        })
    }

    pub(crate) fn source(&self) -> Arc<Source> {
        self.source.clone()
    }

    /// Parse through the composed stages, running OCR rounds with the
    /// configured engine.
    fn parse(&self, pages: Option<Vec<u32>>) -> FfiResult<ResultState> {
        let pages = self.selection(pages)?;
        let core = self.core();
        let engine = self.ocr_engine.clone()?;
        let mut job = Job::begin(self.source.clone(), core, pages.as_deref())?;
        if let Some(engine) = engine {
            job.run_ocr(&engine)?;
        }
        job.into_result()
    }

    fn screenshot(
        &self,
        pages: Option<Vec<u32>>,
        dpi_override: f32,
        region: Option<LiteParseRenderRegion>,
    ) -> FfiResult<ScreenshotsState> {
        let request = RenderRequest::from_config(&self.config, dpi_override, region)?;
        let pages = self.selection(pages)?;
        render_pages(self, pages.as_deref(), &request)
            .and_then(|(shots, errors)| ScreenshotsState::new(&shots, &errors))
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
            state.parse(copy_page_numbers(pages, pages_len)?)
        })
    }
}

/// Extract pre-projection pages: heuristic text items, graphics, and the
/// configured extras, with no projection, OCR, text, or Markdown. Link
/// stamping and word boxes follow the same rules as parse (links only under
/// Markdown; word boxes when requested or under Markdown). The view's
/// `content` is valid `LiteParseContentInput.content` for
/// `liteparse_parser_parse_content` to project and classify.
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
/// non-null `region` renders only that part of each page, at the size and
/// scale of the matching crop of a whole-page render; glyph anti-aliasing
/// can differ slightly from the crop. Rectangle detection reads the whole
/// page, so with it the page is rendered in full and detected rectangles
/// are clipped and made region-relative.
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
            let pages = state.selection(copy_page_numbers(pages, pages_len)?)?;
            let (stats, errors) = complexity(state, &state.core(), pages.as_deref())?;
            ComplexityState::new(&stats, &errors)
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
