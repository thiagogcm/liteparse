use liteparse::RawTextItem;
use liteparse::{append_raw_widget_text_items, extract_raw_text_items};
use liteparse_pdfium::Library;

use crate::document::DocumentState;
use crate::handle::{
    LiteParseStr, Pool, array_ptr, free_handle, opaque_handles, packed_len, view_of, view_state,
};
use crate::records::{LiteParsePageGeometry, LiteParseRect, flag_bits, optional};
use crate::render::{load_document, map_pages, page_facts};
use crate::status::FfiResult;

/// Heuristic-free text runs. Views borrow from the handle.
pub struct LiteParseRawText {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseRawText => RawTextState, "raw_text";
}

/// `LiteParseRawTextPage.flags` bits.
pub const LITEPARSE_RAW_PAGE_FLAG_HAS_GEOMETRY: u32 = 1 << 0;
pub const LITEPARSE_RAW_PAGE_FLAG_HAS_ROTATION: u32 = 1 << 1;

/// `LiteParseRawTextItem.flags` bits.
pub const LITEPARSE_RAW_ITEM_FLAG_HAS_GLYPH_NAMES: u32 = 1 << 0;
pub const LITEPARSE_RAW_ITEM_FLAG_HAS_GROUNDING_BOUNDS: u32 = 1 << 1;
pub const LITEPARSE_RAW_ITEM_FLAG_HAS_BASELINE_GAP: u32 = 1 << 2;
pub const LITEPARSE_RAW_ITEM_FLAG_HAS_MCID: u32 = 1 << 3;
pub const LITEPARSE_RAW_ITEM_FLAG_HAS_FILL_COLOR: u32 = 1 << 4;
pub const LITEPARSE_RAW_ITEM_FLAG_HAS_STROKE_COLOR: u32 = 1 << 5;
pub const LITEPARSE_RAW_ITEM_FLAG_FONT_IS_BUGGY: u32 = 1 << 6;
pub const LITEPARSE_RAW_ITEM_FLAG_TRAILING_SPACE_GENERATED: u32 = 1 << 7;

/// One extracted page. `item_offset/count` index the view's `items`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseRawTextPage {
    pub label: LiteParseStr,
    pub geometry: LiteParsePageGeometry,
    pub page_number: u32,
    /// `LITEPARSE_RAW_PAGE_FLAG_*` bits.
    pub flags: u32,
    pub width: f32,
    pub height: f32,
    pub item_offset: u32,
    pub item_count: u32,
}

/// One heuristic-free text run. `char_code_offset/count` index the view's
/// `char_codes`; `glyph_name_offset/count` (Type3 fonts only, parallel to
/// the char codes) index `glyph_names`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseRawTextItem {
    pub text: LiteParseStr,
    pub font_name: LiteParseStr,
    /// Advance gap across a lone generated space, in page points.
    pub baseline_gap: f64,
    /// Tight viewport-space bounds of non-generated, non-space glyphs.
    pub grounding_bounds: LiteParseRect,
    /// Counter-clockwise radians in `[0, 2π)` with page rotation folded in.
    pub angle_radians: f32,
    pub text_width: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub font_height: f32,
    pub font_ascent: f32,
    pub font_descent: f32,
    pub font_weight: i32,
    pub mcid: i32,
    /// Packed ARGB when the colour space is reportable as RGB.
    pub fill_color: u32,
    pub stroke_color: u32,
    pub char_code_offset: u32,
    pub char_code_count: u32,
    pub glyph_name_offset: u32,
    pub glyph_name_count: u32,
    /// `LITEPARSE_RAW_ITEM_FLAG_*` bits.
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseRawTextView {
    /// String pool behind every `LiteParseStr` in this view.
    pub pool: *const u8,
    pub pool_len: usize,
    pub pages: *const LiteParseRawTextPage,
    pub pages_len: usize,
    pub items: *const LiteParseRawTextItem,
    pub items_len: usize,
    pub char_codes: *const u32,
    pub char_codes_len: usize,
    pub glyph_names: *const LiteParseStr,
    pub glyph_names_len: usize,
}

struct OwnedPage {
    page_number: u32,
    page_label: Option<String>,
    page_width: f32,
    page_height: f32,
    geometry: Option<(LiteParsePageGeometry, bool)>,
    items: Vec<RawTextItem>,
}

pub(crate) struct RawTextState {
    /// Backing storage for `view`.
    #[allow(dead_code)]
    pool: Pool,
    #[allow(dead_code)]
    pages: Vec<LiteParseRawTextPage>,
    #[allow(dead_code)]
    items: Vec<LiteParseRawTextItem>,
    #[allow(dead_code)]
    char_codes: Vec<u32>,
    #[allow(dead_code)]
    glyph_names: Vec<LiteParseStr>,
    view: LiteParseRawTextView,
}

view_state!(RawTextState => LiteParseRawTextView, view);

impl RawTextState {
    fn pack(source: Vec<OwnedPage>) -> Self {
        let mut pool = Pool::default();
        let mut pages = Vec::with_capacity(source.len());
        let mut items = Vec::new();
        let mut char_codes = Vec::new();
        let mut glyph_names = Vec::new();
        for page in &source {
            let item_offset = items.len();
            for item in &page.items {
                let char_code_offset = char_codes.len();
                char_codes.extend_from_slice(&item.char_codes);
                let glyph_name_offset = glyph_names.len();
                glyph_names.extend(
                    item.glyph_names
                        .iter()
                        .flatten()
                        .map(|name| pool.push(name)),
                );
                let (mcid, has_mcid) = optional(item.mcid);
                let (fill_color, has_fill_color) = optional(item.fill_color);
                let (stroke_color, has_stroke_color) = optional(item.stroke_color);
                let (baseline_gap, has_baseline_gap) = optional(item.baseline_gap);
                let grounding_bounds = item.grounding_bounds.as_ref().map(|bounds| LiteParseRect {
                    x: bounds.left,
                    y: bounds.top,
                    width: bounds.right - bounds.left,
                    height: bounds.bottom - bounds.top,
                });
                items.push(LiteParseRawTextItem {
                    text: pool.push(&item.text),
                    font_name: pool.push(&item.font_name),
                    baseline_gap,
                    grounding_bounds: grounding_bounds.unwrap_or_default(),
                    angle_radians: item.angle_radians,
                    text_width: item.text_width,
                    x: item.x,
                    y: item.y,
                    width: item.width,
                    height: item.height,
                    font_size: item.font_size,
                    font_height: item.font_height,
                    font_ascent: item.font_ascent,
                    font_descent: item.font_descent,
                    font_weight: item.font_weight,
                    mcid,
                    fill_color,
                    stroke_color,
                    char_code_offset: packed_len(char_code_offset),
                    char_code_count: packed_len(char_codes.len() - char_code_offset),
                    glyph_name_offset: packed_len(glyph_name_offset),
                    glyph_name_count: packed_len(glyph_names.len() - glyph_name_offset),
                    flags: flag_bits(&[
                        (
                            item.glyph_names.is_some(),
                            LITEPARSE_RAW_ITEM_FLAG_HAS_GLYPH_NAMES,
                        ),
                        (
                            grounding_bounds.is_some(),
                            LITEPARSE_RAW_ITEM_FLAG_HAS_GROUNDING_BOUNDS,
                        ),
                        (has_baseline_gap, LITEPARSE_RAW_ITEM_FLAG_HAS_BASELINE_GAP),
                        (has_mcid, LITEPARSE_RAW_ITEM_FLAG_HAS_MCID),
                        (has_fill_color, LITEPARSE_RAW_ITEM_FLAG_HAS_FILL_COLOR),
                        (has_stroke_color, LITEPARSE_RAW_ITEM_FLAG_HAS_STROKE_COLOR),
                        (item.font_is_buggy, LITEPARSE_RAW_ITEM_FLAG_FONT_IS_BUGGY),
                        (
                            item.trailing_space_generated,
                            LITEPARSE_RAW_ITEM_FLAG_TRAILING_SPACE_GENERATED,
                        ),
                    ]),
                });
            }
            let (geometry, has_rotation) = page.geometry.unwrap_or_default();
            pages.push(LiteParseRawTextPage {
                label: pool.push_opt(page.page_label.as_deref()),
                geometry,
                page_number: page.page_number,
                flags: flag_bits(&[
                    (
                        page.geometry.is_some(),
                        LITEPARSE_RAW_PAGE_FLAG_HAS_GEOMETRY,
                    ),
                    (has_rotation, LITEPARSE_RAW_PAGE_FLAG_HAS_ROTATION),
                ]),
                width: page.page_width,
                height: page.page_height,
                item_offset: packed_len(item_offset),
                item_count: packed_len(page.items.len()),
            });
        }
        let view = LiteParseRawTextView {
            pool: pool.ptr(),
            pool_len: pool.len(),
            pages: array_ptr(&pages),
            pages_len: pages.len(),
            items: array_ptr(&items),
            items_len: items.len(),
            char_codes: array_ptr(&char_codes),
            char_codes_len: char_codes.len(),
            glyph_names: array_ptr(&glyph_names),
            glyph_names_len: glyph_names.len(),
        };
        Self {
            pool,
            pages,
            items,
            char_codes,
            glyph_names,
            view,
        }
    }
}

pub(crate) fn extract_raw_text(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<RawTextState> {
    let lib = Library::init();
    let document = load_document(&lib, &state.input, state.config.password.as_deref())?;
    liteparse::extract::apply_page_orientation_corrections(
        &document,
        &state.config.page_orientation_corrections,
    )?;
    let resolver = state.glyph_resolver.as_deref();
    let owned = map_pages(
        &document,
        pages,
        state.config.max_pages,
        &state.config,
        "raw-text",
        |page_num, page| {
            let page_index = page_num as i32 - 1;
            let facts = page_facts(page);
            let text_page = page.text()?;
            let mut items = extract_raw_text_items(page, &text_page, &facts.view_box, resolver);
            append_raw_widget_text_items(&document, page, page_index, resolver, &mut items);
            Ok(OwnedPage {
                page_number: page_num,
                page_label: document.page_label(page_index),
                page_width: facts.width,
                page_height: facts.height,
                geometry: facts.geometry,
                items,
            })
        },
    )?;
    Ok(RawTextState::pack(owned))
}

/// Destroy a raw-text handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_raw_text_free(raw_text: *mut LiteParseRawText) {
    unsafe { free_handle(raw_text) };
}

/// Borrow the extracted runs; null for a null handle.
///
/// `raw_text` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_raw_text_view(
    raw_text: *const LiteParseRawText,
) -> *const LiteParseRawTextView {
    unsafe { view_of(raw_text) }
}
