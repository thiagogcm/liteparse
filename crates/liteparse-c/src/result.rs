use liteparse::extract::ExtractedPages;
use liteparse::types::OutlineTarget;
use liteparse::{LiteParseConfig as CoreConfig, ParseResult};

use crate::budget::check_result_bytes;
use crate::content::LiteParseContentArrays;
use crate::handle::{LiteParseStr, array_ptr, free_handle, opaque_handles, view_of, view_state};
use crate::pack::{Packed, PageParts};
use crate::records::*;
use crate::render::effective_dpi;
use crate::screenshots::pack_screenshots;
use crate::status::FfiResult;

/// A parse or extract result. Views borrow from it until it is freed.
pub struct LiteParseResult {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseResult => ResultState, "result";
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
/// Extraction read an AcroForm-repaired copy of the source: its page widgets
/// were orphaned from a missing `/AcroForm`, which was rebuilt in memory to
/// adopt their fields. Everything extracted, form fields and the form type
/// included, comes from that copy; see `repaired_page_numbers`.
pub const LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM: u32 = 1 << 6;

/// How extraction recovered the document's form content: the facts behind
/// `LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS` and
/// `LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM`.
#[derive(Default)]
pub(crate) struct FormRecovery {
    /// Pages flattened onto a temporary document to recover widget text.
    pub(crate) flattened_pages: Vec<u32>,
    /// Present when extraction read an AcroForm-repaired copy: the result's
    /// pages holding the adopted fields' widgets, possibly none.
    pub(crate) repaired_pages: Option<Vec<u32>>,
}

impl FormRecovery {
    /// What extraction of `pages` did; `repaired` says it read the
    /// AcroForm-repaired copy.
    ///
    /// Repair runs only on a source with no `/AcroForm`, where PDFium reads
    /// no widget as a field, and the rebuilt one adopts the fields of the
    /// pages' widgets. So every form field extraction found in the copy is
    /// one the repair made readable, and the pages holding them are the
    /// pages holding adopted widgets.
    pub(crate) fn of(pages: &ExtractedPages, repaired: bool) -> Self {
        Self {
            flattened_pages: pages.flattened_page_numbers.clone(),
            repaired_pages: repaired.then(|| {
                pages
                    .pages
                    .iter()
                    .filter(|page| page.form_fields.as_ref().is_some_and(|f| !f.is_empty()))
                    .map(|page| page.page_number as u32)
                    .collect()
            }),
        }
    }

    fn flags(&self) -> u32 {
        flag_bits(&[
            (
                !self.flattened_pages.is_empty(),
                LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS,
            ),
            (
                self.repaired_pages.is_some(),
                LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM,
            ),
        ])
    }

    fn pack(self, packed: &mut Packed) -> u32 {
        let flags = self.flags();
        packed.flattened_page_numbers = self.flattened_pages;
        packed.repaired_page_numbers = self.repaired_pages.unwrap_or_default();
        flags
    }
}

/// Everything a result exposes. `content` is the page-content model accepted
/// by `liteparse_parser_parse_content`; the remaining arrays are result only.
/// Projected spans share `content.words` and `content.char_codes`; every
/// `LiteParseStr` in the view indexes `content.arenas.pool` and every
/// `LiteParseBlobRef` indexes `content.arenas.binary`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseResultView {
    pub content: LiteParseContentArrays,
    /// Parallel to `content.pages` for parse results; empty for extract-only
    /// results.
    pub page_outputs: *const LiteParsePageOutput,
    pub page_outputs_len: usize,
    /// Full-document plain text or Markdown, per the output format.
    pub text: LiteParseStr,
    pub creator: LiteParseStr,
    pub producer: LiteParseStr,
    pub doc_meta: LiteParseDocumentMeta,
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
    /// Ascending 1-based numbers of the pages extraction flattened onto a
    /// temporary document to recover form-widget text.
    pub flattened_page_numbers: *const u32,
    pub flattened_page_numbers_len: usize,
    /// With `LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM`, the ascending 1-based
    /// numbers of the result's pages holding widgets the repair adopted:
    /// the pages whose form fields only the repair made readable. Empty when
    /// the selection holds none of them.
    pub repaired_page_numbers: *const u32,
    pub repaired_page_numbers_len: usize,
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
}

pub(crate) struct ResultState {
    /// Backing storage for the pointers in `view`.
    #[allow(dead_code)]
    packed: Packed,
    view: LiteParseResultView,
}

view_state!(ResultState => LiteParseResultView, view);

pub(crate) type PageGeometries = Vec<Option<(LiteParsePageGeometry, bool)>>;

impl ResultState {
    /// Pack a parse result. `geometries` is parallel to `result.pages`.
    pub(crate) fn parsed(
        result: &ParseResult,
        config: &CoreConfig,
        descriptive: Option<&DescriptiveInfo>,
        geometries: PageGeometries,
        forms: FormRecovery,
    ) -> FfiResult<Self> {
        let mut packed = Packed::default();
        for (index, page) in result.pages.iter().enumerate() {
            packed.push_page(PageParts::parsed(
                page,
                geometries.get(index).copied().flatten(),
            ))?;
        }
        packed.pack_sidecars(&result.images, &result.outline, &result.page_errors)?;
        let form_flags = forms.pack(&mut packed);
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
        let arrays = packed.array_bytes();
        (packed.screenshots, packed.screenshot_rects) =
            pack_screenshots(screenshots_with_dpi, &mut packed.arenas, arrays)?;
        check_result_bytes(packed.bytes() + result.text.len() as u64)?;
        let pool = &mut packed.arenas.pool;
        packed.xfa_packets = pack_all(
            pool,
            result.xfa_packets.as_deref().unwrap_or_default(),
            LiteParseXfaPacket::pack,
        );
        let facts = ResultFacts {
            text: pool.push(&result.text),
            creator: pool.push_opt(result.creator.as_deref()),
            producer: pool.push_opt(result.producer.as_deref()),
            doc_meta: result
                .doc_meta
                .as_ref()
                .map(|meta| LiteParseDocumentMeta::pack(pool, meta, descriptive)),
            total_pages: result.total_pages,
            image_error_count: result.image_error_count,
            form_type: result.form_type,
            flags: form_flags
                | flag_bits(&[(
                    result.xfa_packets.is_some(),
                    LITEPARSE_RESULT_FLAG_HAS_XFA_PACKETS,
                )]),
        };
        Self::assemble(packed, facts, config.extract_text_metadata)
    }

    /// Pack pre-projection pages from `extract_pages_and_images`.
    pub(crate) fn extracted(
        pages: &ExtractedPages,
        form_type: Option<i32>,
        forms: FormRecovery,
        config: &CoreConfig,
        geometries: PageGeometries,
        total_pages: u32,
        outline: &[OutlineTarget],
    ) -> FfiResult<Self> {
        let mut packed = Packed::default();
        for (index, page) in pages.pages.iter().enumerate() {
            packed.push_page(PageParts::extracted(
                page,
                geometries.get(index).copied().flatten(),
            ))?;
        }
        packed.page_outputs.clear();
        packed.pack_sidecars(&pages.images, outline, &pages.page_errors)?;
        let form_flags = forms.pack(&mut packed);
        let facts = ResultFacts {
            text: LiteParseStr::default(),
            creator: LiteParseStr::default(),
            producer: LiteParseStr::default(),
            doc_meta: None,
            total_pages,
            image_error_count: pages.image_error_count,
            form_type,
            flags: form_flags | LITEPARSE_RESULT_FLAG_EXTRACT_ONLY,
        };
        Self::assemble(packed, facts, config.extract_text_metadata)
    }

    // Moving the packed vectors into the state keeps their heap buffers, so
    // a view built before the move stays valid.
    fn assemble(packed: Packed, facts: ResultFacts, text_metadata: bool) -> FfiResult<Self> {
        packed.check()?;
        let flags = facts.flags
            | flag_bits(&[
                (facts.doc_meta.is_some(), LITEPARSE_RESULT_FLAG_HAS_DOC_META),
                (
                    facts.form_type.is_some(),
                    LITEPARSE_RESULT_FLAG_HAS_FORM_TYPE,
                ),
                (text_metadata, LITEPARSE_RESULT_FLAG_TEXT_METADATA),
            ]);
        let view = LiteParseResultView {
            content: packed.content_arrays(facts.total_pages),
            page_outputs: array_ptr(&packed.page_outputs),
            page_outputs_len: packed.page_outputs.len(),
            text: facts.text,
            creator: facts.creator,
            producer: facts.producer,
            doc_meta: facts.doc_meta.unwrap_or_default(),
            image_error_count: facts.image_error_count,
            form_type: facts.form_type.unwrap_or(LITEPARSE_FORM_TYPE_NONE),
            flags,
            screenshots: array_ptr(&packed.screenshots),
            screenshots_len: packed.screenshots.len(),
            screenshot_rects: array_ptr(&packed.screenshot_rects),
            screenshot_rects_len: packed.screenshot_rects.len(),
            xfa_packets: array_ptr(&packed.xfa_packets),
            xfa_packets_len: packed.xfa_packets.len(),
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
            flattened_page_numbers: array_ptr(&packed.flattened_page_numbers),
            flattened_page_numbers_len: packed.flattened_page_numbers.len(),
            repaired_page_numbers: array_ptr(&packed.repaired_page_numbers),
            repaired_page_numbers_len: packed.repaired_page_numbers.len(),
        };
        Ok(Self { packed, view })
    }
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
