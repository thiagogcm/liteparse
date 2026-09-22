use std::sync::OnceLock;

use liteparse::extract::ExtractedPages;
use liteparse::search::{SearchOptions, search_items};
use liteparse::types::TextItem;
use liteparse::{LiteParseConfig as CoreConfig, ParseResult};
use serde_json::Value;

use crate::content::LiteParseContent;
use crate::handle::{
    LiteParseByteView, LiteParseStr, array_ptr, bytes_view, create_handle, free_handle,
    opaque_handles, required_str, state_ref, view_of, view_state, write_out,
};
use crate::pack::{Packed, PageParts};
use crate::records::*;
use crate::render::effective_dpi;
use crate::screenshots::pack_screenshots;
use crate::status::{FfiError, FfiResult, LiteParseStatus, boundary};

/// A parse or extract result. Views borrow from it until it is freed.
pub struct LiteParseResult {
    _opaque: [u8; 0],
}

/// Phrase matches copied out of a result; they outlive it.
pub struct LiteParseSearchMatches {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseResult => ResultState, "result";
    LiteParseSearchMatches => SearchState, "matches";
}

/// `LiteParseResultView.flags` bits.
pub const LITEPARSE_RESULT_FLAG_HAS_DOC_META: u32 = 1 << 0;
pub const LITEPARSE_RESULT_FLAG_HAS_FORM_TYPE: u32 = 1 << 1;
pub const LITEPARSE_RESULT_FLAG_HAS_XFA_PACKETS: u32 = 1 << 2;
/// Text metadata extraction was requested in the parser configuration.
pub const LITEPARSE_RESULT_FLAG_TEXT_METADATA: u32 = 1 << 3;
/// Produced by `liteparse_document_extract`: no projection, text, or Markdown.
pub const LITEPARSE_RESULT_FLAG_EXTRACT_ONLY: u32 = 1 << 4;
/// Extraction flattened at least one page to recover form-widget text; see
/// `flattened_page_numbers`.
pub const LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS: u32 = 1 << 5;

/// `liteparse_result_search` flags.
pub const LITEPARSE_SEARCH_FLAG_CASE_SENSITIVE: u32 = 1 << 0;

/// Everything a result exposes. `content` is the page-content model shared
/// with `liteparse_parser_parse_content`; the remaining arrays are result
/// only. Projected spans share `content.words` and `content.char_codes`, and
/// every `LiteParseStr` in the view indexes `content.pool`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseResultView {
    pub content: LiteParseContent,
    /// Full-document plain text or Markdown, per the output format.
    pub text: LiteParseStr,
    pub creator: LiteParseStr,
    pub producer: LiteParseStr,
    pub doc_meta: LiteParseDocumentMeta,
    pub total_pages: u32,
    pub image_error_count: u32,
    /// `LITEPARSE_FORM_TYPE_*`, meaningful with `HAS_FORM_TYPE`.
    pub form_type: i32,
    /// `LITEPARSE_RESULT_FLAG_*` bits.
    pub flags: u32,
    pub screenshots: *const LiteParseScreenshot,
    pub screenshots_len: usize,
    pub screenshot_rects: *const LiteParseScreenshotRect,
    pub screenshot_rects_len: usize,
    pub xfa_packets: *const LiteParseXfaPacket,
    pub xfa_packets_len: usize,
    pub figures: *const LiteParseRect,
    pub figures_len: usize,
    pub item_frames: *const LiteParseItemFrame,
    pub item_frames_len: usize,
    pub projected_lines: *const LiteParseProjectedLine,
    pub projected_lines_len: usize,
    pub projected_spans: *const LiteParseTextItem,
    pub projected_spans_len: usize,
    pub region_paths: *const u16,
    pub region_paths_len: usize,
    pub regions: *const LiteParseProjectedRegion,
    pub regions_len: usize,
    pub region_children: *const u32,
    pub region_children_len: usize,
    pub flattened_page_numbers: *const u32,
    pub flattened_page_numbers_len: usize,
}

/// Text items copied out of a result by `liteparse_result_search`. Strings
/// index the view's own `pool`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseSearchView {
    pub pool: *const u8,
    pub pool_len: usize,
    pub items: *const LiteParseTextItem,
    pub items_len: usize,
    pub words: *const LiteParseWordBox,
    pub words_len: usize,
    pub char_codes: *const u32,
    pub char_codes_len: usize,
}

enum Source {
    Parsed(Box<ParseResult>),
    Extracted(ExtractedPages),
}

/// Result-level scalars that differ between parse and extract sources.
struct ResultFacts {
    text: LiteParseStr,
    creator: LiteParseStr,
    producer: LiteParseStr,
    doc_meta: Option<LiteParseDocumentMeta>,
    total_pages: u32,
    image_error_count: u32,
    form_type: Option<i32>,
    flags: u32,
    flattened_page_numbers: Vec<u32>,
}

pub(crate) struct ResultState {
    source: Source,
    /// Backing storage for the pointers in `view`.
    #[allow(dead_code)]
    packed: Packed,
    #[allow(dead_code)]
    screenshots: Vec<LiteParseScreenshot>,
    #[allow(dead_code)]
    screenshot_rects: Vec<LiteParseScreenshotRect>,
    #[allow(dead_code)]
    xfa_packets: Vec<LiteParseXfaPacket>,
    #[allow(dead_code)]
    flattened_page_numbers: Vec<u32>,
    view: LiteParseResultView,
    extract_text_metadata: bool,
    descriptive: Option<DescriptiveInfo>,
    json: OnceLock<Result<String, String>>,
}

view_state!(ResultState => LiteParseResultView, view);

pub(crate) type PageGeometries = Vec<Option<(LiteParsePageGeometry, bool)>>;

impl ResultState {
    /// Wrap a parse result. `geometries` is parallel to `result.pages`.
    pub(crate) fn parsed(
        result: ParseResult,
        config: &CoreConfig,
        descriptive: Option<DescriptiveInfo>,
        geometries: PageGeometries,
    ) -> Self {
        let mut packed = Packed::default();
        for (index, page) in result.pages.iter().enumerate() {
            packed.push_page(PageParts::parsed(
                page,
                geometries.get(index).copied().flatten(),
            ));
        }
        packed.images = pack_all(&mut packed.pool, &result.images, LiteParseImage::pack);
        packed.outline = pack_all(
            &mut packed.pool,
            &result.outline,
            LiteParseOutlineEntry::pack,
        );
        packed.page_errors = pack_all(
            &mut packed.pool,
            &result.page_errors,
            LiteParsePageError::pack,
        );
        let requested_dpi = config.dpi;
        let screenshots_with_dpi = result.screenshots.iter().map(|shot| {
            let dpi = result
                .pages
                .iter()
                .find(|page| page.page_number == shot.page_num as usize)
                .map_or(requested_dpi, |page| {
                    effective_dpi(requested_dpi, page.page_width, page.page_height)
                });
            (shot, dpi)
        });
        let (screenshots, screenshot_rects) = pack_screenshots(screenshots_with_dpi);
        let xfa_packets = pack_all(
            &mut packed.pool,
            result.xfa_packets.as_deref().unwrap_or_default(),
            LiteParseXfaPacket::pack,
        );
        let facts = ResultFacts {
            text: packed.pool.push(&result.text),
            creator: packed.pool.push_opt(result.creator.as_deref()),
            producer: packed.pool.push_opt(result.producer.as_deref()),
            doc_meta: result.doc_meta.as_ref().map(|meta| {
                LiteParseDocumentMeta::pack(&mut packed.pool, meta, descriptive.as_ref())
            }),
            total_pages: result.total_pages,
            image_error_count: result.image_error_count,
            form_type: result.form_type,
            flags: flag_bits(&[(
                result.xfa_packets.is_some(),
                LITEPARSE_RESULT_FLAG_HAS_XFA_PACKETS,
            )]),
            flattened_page_numbers: Vec::new(),
        };
        Self::assemble(
            Source::Parsed(Box::new(result)),
            packed,
            screenshots,
            screenshot_rects,
            xfa_packets,
            facts,
            config.extract_text_metadata,
            descriptive,
        )
    }

    /// Wrap pre-projection pages from `extract_pages_and_images`.
    pub(crate) fn extracted(
        pages: ExtractedPages,
        form_type: Option<i32>,
        config: &CoreConfig,
        geometries: PageGeometries,
    ) -> Self {
        let mut packed = Packed::default();
        for (index, page) in pages.pages.iter().enumerate() {
            packed.push_page(PageParts::extracted(
                page,
                geometries.get(index).copied().flatten(),
            ));
        }
        packed.images = pack_all(&mut packed.pool, &pages.images, LiteParseImage::pack);
        packed.page_errors = pack_all(
            &mut packed.pool,
            &pages.page_errors,
            LiteParsePageError::pack,
        );
        let facts = ResultFacts {
            text: LiteParseStr::default(),
            creator: LiteParseStr::default(),
            producer: LiteParseStr::default(),
            doc_meta: None,
            total_pages: 0,
            image_error_count: pages.image_error_count,
            form_type,
            flags: flag_bits(&[
                (true, LITEPARSE_RESULT_FLAG_EXTRACT_ONLY),
                (
                    pages.flattened_form_widgets,
                    LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS,
                ),
            ]),
            flattened_page_numbers: pages.flattened_page_numbers.clone(),
        };
        Self::assemble(
            Source::Extracted(pages),
            packed,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            facts,
            config.extract_text_metadata,
            None,
        )
    }

    // Moving vectors into the state keeps their heap buffers, so a view built
    // from the locals stays valid.
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        source: Source,
        packed: Packed,
        screenshots: Vec<LiteParseScreenshot>,
        screenshot_rects: Vec<LiteParseScreenshotRect>,
        xfa_packets: Vec<LiteParseXfaPacket>,
        facts: ResultFacts,
        extract_text_metadata: bool,
        descriptive: Option<DescriptiveInfo>,
    ) -> Self {
        let flags = facts.flags
            | flag_bits(&[
                (facts.doc_meta.is_some(), LITEPARSE_RESULT_FLAG_HAS_DOC_META),
                (
                    facts.form_type.is_some(),
                    LITEPARSE_RESULT_FLAG_HAS_FORM_TYPE,
                ),
                (extract_text_metadata, LITEPARSE_RESULT_FLAG_TEXT_METADATA),
            ]);
        let flattened_page_numbers = facts.flattened_page_numbers;
        let view = LiteParseResultView {
            content: packed.content_view(),
            text: facts.text,
            creator: facts.creator,
            producer: facts.producer,
            doc_meta: facts.doc_meta.unwrap_or_default(),
            total_pages: facts.total_pages,
            image_error_count: facts.image_error_count,
            form_type: facts.form_type.unwrap_or(LITEPARSE_FORM_TYPE_NONE),
            flags,
            screenshots: array_ptr(&screenshots),
            screenshots_len: screenshots.len(),
            screenshot_rects: array_ptr(&screenshot_rects),
            screenshot_rects_len: screenshot_rects.len(),
            xfa_packets: array_ptr(&xfa_packets),
            xfa_packets_len: xfa_packets.len(),
            figures: array_ptr(&packed.figures),
            figures_len: packed.figures.len(),
            item_frames: array_ptr(&packed.item_frames),
            item_frames_len: packed.item_frames.len(),
            projected_lines: array_ptr(&packed.projected_lines),
            projected_lines_len: packed.projected_lines.len(),
            projected_spans: array_ptr(&packed.projected_spans),
            projected_spans_len: packed.projected_spans.len(),
            region_paths: array_ptr(&packed.region_paths),
            region_paths_len: packed.region_paths.len(),
            regions: array_ptr(&packed.regions),
            regions_len: packed.regions.len(),
            region_children: array_ptr(&packed.region_children),
            region_children_len: packed.region_children.len(),
            flattened_page_numbers: array_ptr(&flattened_page_numbers),
            flattened_page_numbers_len: flattened_page_numbers.len(),
        };
        Self {
            source,
            packed,
            screenshots,
            screenshot_rects,
            xfa_packets,
            flattened_page_numbers,
            view,
            extract_text_metadata,
            descriptive,
            json: OnceLock::new(),
        }
    }

    fn json(&self) -> FfiResult<&str> {
        self.json
            .get_or_init(|| match &self.source {
                Source::Parsed(result) => format_result(
                    result,
                    self.extract_text_metadata,
                    self.descriptive.as_ref(),
                )
                .map_err(|error| error.message),
                Source::Extracted(_) => {
                    Err("extract results have no JSON form; read the view".to_owned())
                }
            })
            .as_deref()
            .map_err(FfiError::serialization)
    }

    fn page_items(&self, page_index: usize) -> FfiResult<&[TextItem]> {
        let items = match &self.source {
            Source::Parsed(result) => result
                .pages
                .get(page_index)
                .map(|p| p.text_items.as_slice()),
            Source::Extracted(pages) => {
                pages.pages.get(page_index).map(|p| p.text_items.as_slice())
            }
        };
        items.ok_or_else(|| {
            FfiError::invalid_argument(format!(
                "page_index {page_index} is out of range for {} pages",
                self.packed.pages.len()
            ))
        })
    }
}

fn format_result(
    result: &ParseResult,
    extract_text_metadata: bool,
    descriptive: Option<&DescriptiveInfo>,
) -> FfiResult<String> {
    let json = liteparse::output::json::format_json_result(result, extract_text_metadata)
        .map_err(FfiError::serialization)?;
    let mut value: Value = serde_json::from_str(&json).map_err(FfiError::serialization)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| FfiError::serialization("core parse JSON was not an object"))?;
    object.insert("text".into(), Value::String(result.text.clone()));
    if let Some(creator) = &result.creator {
        object.insert("creator".into(), Value::String(creator.clone()));
    }
    if let Some(producer) = &result.producer {
        object.insert("producer".into(), Value::String(producer.clone()));
    }
    if let Some(metadata) = &result.doc_meta {
        let mut metadata = serde_json::to_value(metadata).map_err(FfiError::serialization)?;
        if let (Some(descriptive), Some(object)) = (descriptive, metadata.as_object_mut()) {
            for (key, value) in [
                ("title", &descriptive.title),
                ("author", &descriptive.author),
                ("subject", &descriptive.subject),
                ("keywords", &descriptive.keywords),
                ("trapped", &descriptive.trapped),
            ] {
                if let Some(value) = value {
                    object.insert(key.into(), Value::String(value.clone()));
                }
            }
        }
        object.insert("doc_meta".into(), metadata);
    }
    if !result.outline.is_empty() {
        let entries = result
            .outline
            .iter()
            .map(|entry| {
                let mut entry_json = serde_json::json!({
                    "level": entry.level,
                    "title": entry.title,
                    "page_index": entry.page_index,
                });
                if let Some(y) = entry.y_pdf {
                    entry_json["y_pdf"] = Value::from(y);
                }
                entry_json
            })
            .collect();
        object.insert("outline".into(), Value::Array(entries));
    }
    serde_json::to_string_pretty(&value).map_err(FfiError::serialization)
}

/// Destroy a result handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_free(result: *mut LiteParseResult) {
    unsafe { free_handle(result) };
}

/// Borrow the result view; null for a null handle. Valid until
/// `liteparse_result_free`.
///
/// `result` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_view(
    result: *const LiteParseResult,
) -> *const LiteParseResultView {
    unsafe { view_of(result) }
}

/// Borrow the cached pretty JSON form of a parse result. Extract results
/// have none.
///
/// `result` must be live and `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_to_json(
    result: *const LiteParseResult,
    out: *mut LiteParseByteView,
) -> LiteParseStatus {
    unsafe { write_out(out, LiteParseByteView::default()) };
    boundary(|| unsafe {
        let json = state_ref(result)?.json()?;
        write_out(out, bytes_view(json.as_bytes()));
        Ok(())
    })
}

pub(crate) struct SearchState {
    /// Backing storage for `view`.
    #[allow(dead_code)]
    packed: Packed,
    view: LiteParseSearchView,
}

view_state!(SearchState => LiteParseSearchView, view);

/// Find phrase matches on one page as merged text items. `page_index` is a
/// 0-based index into `content.pages`, not a source page number. `phrase`
/// is `phrase_len` bytes of UTF-8. `flags` is a mask of
/// `LITEPARSE_SEARCH_FLAG_*`. Matches outlive the result.
///
/// `result` must be live, `phrase` readable for `phrase_len` bytes, and
/// `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_search(
    result: *const LiteParseResult,
    page_index: usize,
    phrase: *const u8,
    phrase_len: usize,
    flags: u32,
    out: *mut *mut LiteParseSearchMatches,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            if flags & !LITEPARSE_SEARCH_FLAG_CASE_SENSITIVE != 0 {
                return Err(FfiError::invalid_argument(
                    "search flags contain unknown bits",
                ));
            }
            let phrase = required_str(phrase, phrase_len, "phrase")?.to_owned();
            let state = state_ref(result)?;
            let options = SearchOptions {
                phrase,
                case_sensitive: flags & LITEPARSE_SEARCH_FLAG_CASE_SENSITIVE != 0,
            };
            let source = search_items(state.page_items(page_index)?, &options);
            let mut packed = Packed::default();
            for item in &source {
                let record = packed.text_item(item);
                packed.items.push(record);
            }
            let view = LiteParseSearchView {
                pool: packed.pool.ptr(),
                pool_len: packed.pool.len(),
                items: array_ptr(&packed.items),
                items_len: packed.items.len(),
                words: array_ptr(&packed.words),
                words_len: packed.words.len(),
                char_codes: array_ptr(&packed.char_codes),
                char_codes_len: packed.char_codes.len(),
            };
            Ok(SearchState { packed, view })
        })
    }
}

/// Borrow the matches; null for a null handle.
///
/// `matches` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_view(
    matches: *const LiteParseSearchMatches,
) -> *const LiteParseSearchView {
    unsafe { view_of(matches) }
}

/// Destroy a search-match handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_free(matches: *mut LiteParseSearchMatches) {
    unsafe { free_handle(matches) };
}
