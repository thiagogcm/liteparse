use liteparse::LiteParse as CoreLiteParse;
use liteparse::ocr_merge::PageComplexityStats;
use liteparse::stages::{self, ExtractionOutputOptions};
use liteparse::types::{Page, PageError};
use liteparse_pdfium::{Document, Library};

use crate::budget::{bytes_of, check_result_bytes};
use crate::document::Source;
use crate::handle::{
    Arenas, LiteParseArenas, array_ptr, free_handle, opaque_handles, view_of, view_state,
};
use crate::job::page_error;
use crate::records::{LiteParsePageComplexity, views};
use crate::status::FfiResult;

/// Per-page complexity signals. Views borrow from the handle.
pub struct LiteParseComplexity {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseComplexity => ComplexityState, "complexity";
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseComplexityView {
    pub arenas: LiteParseArenas,
    pub pages: *const LiteParsePageComplexity,
    pub pages_len: usize,
}

pub(crate) struct ComplexityState {
    /// Backing storage for `view`.
    #[allow(dead_code)]
    arenas: Arenas,
    #[allow(dead_code)]
    pages: Vec<LiteParsePageComplexity>,
    view: LiteParseComplexityView,
}

view_state!(ComplexityState => LiteParseComplexityView, view);

impl ComplexityState {
    pub(crate) fn new(stats: &[PageComplexityStats], errors: &[PageError]) -> FfiResult<Self> {
        let mut arenas = Arenas::default();
        arenas.push_errors(errors);
        let pages: Vec<LiteParsePageComplexity> = views(stats);
        check_result_bytes(arenas.bytes() + bytes_of(&pages))?;
        let view = LiteParseComplexityView {
            arenas: arenas.view(),
            pages: array_ptr(&pages),
            pages_len: pages.len(),
        };
        Ok(Self {
            arenas,
            pages,
            view,
        })
    }
}

/// Complexity of one extracted page. Under tolerant processing a failure
/// becomes a page error and `None`.
pub(crate) fn page_complexity(
    document: &Document<'_>,
    page: &Page,
    tolerant: bool,
    errors: &mut Vec<PageError>,
) -> FfiResult<Option<PageComplexityStats>> {
    match stages::page_complexity(document, page) {
        Ok(stats) => Ok(Some(stats)),
        Err(error) if tolerant => {
            errors.push(page_error(page.page_number, &error));
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

/// Complexity of the selected pages: extraction's text facts plus page
/// objects under one open, then layout signals from projection.
pub(crate) fn complexity(
    source: &Source,
    core: &CoreLiteParse,
    pages: Option<&[u32]>,
) -> FfiResult<(Vec<PageComplexityStats>, Vec<PageError>)> {
    let config = &source.config;
    let (pages, mut stats, errors) = {
        let lib = Library::try_init()?;
        let document = source.open(&lib, &source.input)?;
        // Images and links are irrelevant to complexity, so skip them.
        let request = stages::ExtractRequest {
            extract_links: false,
            output: ExtractionOutputOptions {
                continue_on_page_error: config.continue_on_page_error,
                ..Default::default()
            },
            ..core.extract_request(pages, config.max_pages)
        };
        let extracted = stages::extract(&document, &request)?;
        let mut errors = extracted.page_errors;
        let mut kept = Vec::with_capacity(extracted.pages.len());
        let mut stats = Vec::with_capacity(extracted.pages.len());
        for page in extracted.pages {
            let tolerant = config.continue_on_page_error;
            if let Some(page_stats) = page_complexity(&document, &page, tolerant, &mut errors)? {
                stats.push(page_stats);
                kept.push(page);
            }
        }
        errors.sort_by_key(|error| error.page_number);
        (kept, stats, errors)
    };
    for (page_stats, page) in stats.iter_mut().zip(stages::project(pages)) {
        page_stats.layout = Some(stages::layout_complexity(&page));
    }
    Ok((stats, errors))
}

/// Destroy a complexity handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_free(complexity: *mut LiteParseComplexity) {
    unsafe { free_handle(complexity) };
}

/// Borrow the analyzed pages; null for a null handle.
///
/// `complexity` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_view(
    complexity: *const LiteParseComplexity,
) -> *const LiteParseComplexityView {
    unsafe { view_of(complexity) }
}
