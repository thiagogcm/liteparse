use liteparse::types::PageGeometry;
use liteparse::RawTextItem;
use liteparse::{append_raw_widget_text_items, extract_raw_text_items};
use liteparse_pdfium::{Library, RectF};

use crate::document::DocumentState;
use crate::handle::{
    bytes_view, free_handle, opaque_handles, optional_str_view, slice_out, state_ref,
    LiteParseByteView,
};
use crate::render::load_document;
use crate::status::{FfiResult, LiteParseStatus, LITEPARSE_STATUS_PARSE_ERROR};
use crate::views::{LiteParsePageGeometry, LiteParseRect, LiteParseRectValue};

/// A document's heuristic-free text runs. Views borrow from this handle.
pub struct LiteParseRawText {
    _opaque: [u8; 0],
}

/// The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseRawTextNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseRawText,
}

opaque_handles! {
    LiteParseRawText => RawTextState, "raw_text";
}

/// One extracted page. `page_label` and `geometry` borrow from the handle.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseRawTextPage {
    pub page_number: u32,
    pub page_label: LiteParseByteView,
    pub page_width: f32,
    pub page_height: f32,
    pub geometry: LiteParsePageGeometry,
    pub has_geometry: bool,
}

/// One heuristic-free text run. Glyph arrays borrow from the handle.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseRawTextItem {
    pub text: LiteParseByteView,
    pub font_name: LiteParseByteView,
    pub char_codes: *const u32,
    pub char_codes_len: usize,
    /// Present only for Type3 fonts; parallel to `char_codes`.
    pub glyph_names: *const LiteParseByteView,
    pub glyph_names_len: usize,
    pub has_glyph_names: bool,
    /// Counter-clockwise radians in `[0, 2π)` with page rotation folded in.
    pub angle_radians: f32,
    pub text_width: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Tight viewport-space bounds of non-generated, non-space glyphs.
    pub grounding_bounds: LiteParseRectValue,
    /// Advance gap across a lone generated space, in page points.
    pub baseline_gap: f64,
    pub has_baseline_gap: bool,
    pub mcid: i32,
    pub has_mcid: bool,
    pub font_size: f32,
    pub font_weight: i32,
    pub font_height: f32,
    pub font_ascent: f32,
    pub font_descent: f32,
    /// Packed ARGB when the colour space is reportable as RGB.
    pub fill_color: u32,
    pub stroke_color: u32,
    pub has_fill_color: bool,
    pub has_stroke_color: bool,
    pub font_is_buggy: bool,
    pub trailing_space_generated: bool,
}

struct OwnedPage {
    page_number: u32,
    page_label: Option<String>,
    page_width: f32,
    page_height: f32,
    geometry: PageGeometry,
    items: Vec<RawTextItem>,
}

pub(crate) struct RawTextState {
    pages: Vec<OwnedPage>,
    page_views: Vec<LiteParseRawTextPage>,
    item_views: Vec<Vec<LiteParseRawTextItem>>,
    glyph_name_views: Vec<Vec<Vec<LiteParseByteView>>>,
}

impl RawTextState {
    fn pack(pages: Vec<OwnedPage>) -> Self {
        let mut state = Self {
            pages,
            page_views: Vec::new(),
            item_views: Vec::new(),
            glyph_name_views: Vec::new(),
        };
        state.page_views = state
            .pages
            .iter()
            .map(|page| {
                let (geometry, has_geometry) = LiteParsePageGeometry::from_core(&page.geometry)
                    .map(|geometry| (geometry, true))
                    .unwrap_or_default();
                LiteParseRawTextPage {
                    page_number: page.page_number,
                    page_label: optional_str_view(page.page_label.as_deref()),
                    page_width: page.page_width,
                    page_height: page.page_height,
                    geometry,
                    has_geometry,
                }
            })
            .collect();
        state.glyph_name_views = state
            .pages
            .iter()
            .map(|page| {
                page.items
                    .iter()
                    .map(|item| {
                        item.glyph_names
                            .as_ref()
                            .map(|names| {
                                names
                                    .iter()
                                    .map(|name| bytes_view(name.as_bytes()))
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        state.item_views = state
            .pages
            .iter()
            .enumerate()
            .map(|(page_index, page)| {
                page.items
                    .iter()
                    .enumerate()
                    .map(|(item_index, item)| {
                        pack_item(item, &state.glyph_name_views[page_index][item_index])
                    })
                    .collect()
            })
            .collect();
        state
    }
}

fn pack_item(item: &RawTextItem, glyph_names: &[LiteParseByteView]) -> LiteParseRawTextItem {
    let (mcid, has_mcid) = match item.mcid {
        Some(mcid) => (mcid, true),
        None => (0, false),
    };
    let (fill_color, has_fill_color) = match item.fill_color {
        Some(color) => (color, true),
        None => (0, false),
    };
    let (stroke_color, has_stroke_color) = match item.stroke_color {
        Some(color) => (color, true),
        None => (0, false),
    };
    LiteParseRawTextItem {
        text: bytes_view(item.text.as_bytes()),
        font_name: bytes_view(item.font_name.as_bytes()),
        char_codes: if item.char_codes.is_empty() {
            std::ptr::null()
        } else {
            item.char_codes.as_ptr()
        },
        char_codes_len: item.char_codes.len(),
        glyph_names: if glyph_names.is_empty() {
            std::ptr::null()
        } else {
            glyph_names.as_ptr()
        },
        glyph_names_len: glyph_names.len(),
        has_glyph_names: item.glyph_names.is_some(),
        angle_radians: item.angle_radians,
        text_width: item.text_width,
        x: item.x,
        y: item.y,
        width: item.width,
        height: item.height,
        grounding_bounds: item
            .grounding_bounds
            .as_ref()
            .map(|bounds| LiteParseRectValue {
                rect: LiteParseRect {
                    x: bounds.left,
                    y: bounds.top,
                    width: bounds.right - bounds.left,
                    height: bounds.bottom - bounds.top,
                },
                present: true,
            })
            .unwrap_or_default(),
        baseline_gap: item.baseline_gap.unwrap_or_default(),
        has_baseline_gap: item.baseline_gap.is_some(),
        mcid,
        has_mcid,
        font_size: item.font_size,
        font_weight: item.font_weight,
        font_height: item.font_height,
        font_ascent: item.font_ascent,
        font_descent: item.font_descent,
        fill_color,
        stroke_color,
        has_fill_color,
        has_stroke_color,
        font_is_buggy: item.font_is_buggy,
        trailing_space_generated: item.trailing_space_generated,
    }
}

pub(crate) fn extract_raw_text(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<RawTextState> {
    let lib = Library::init();
    let document = load_document(&lib, &state.input, state.config.password.as_deref())?;
    let page_count = document.page_count().max(0) as u32;
    let mut pages = match pages {
        Some(pages) => pages,
        None => (1..=page_count).collect(),
    };
    pages.truncate(state.config.max_pages);

    let resolver = state.glyph_resolver.as_deref();
    let mut owned = Vec::with_capacity(pages.len());
    for page_num in pages {
        let extracted = (|| -> FfiResult<OwnedPage> {
            let page_index = (page_num as i32) - 1;
            let page = document.page(page_index)?;
            let raw_page_width = page.width();
            let raw_page_height = page.height();
            let view_box = page.view_box().unwrap_or(RectF {
                left: 0.0,
                top: raw_page_height,
                right: raw_page_width,
                bottom: 0.0,
            });
            let (page_width, page_height) = page.viewport_size(&view_box);
            let text_page = page.text()?;
            let mut items = extract_raw_text_items(&page, &text_page, &view_box, resolver);
            append_raw_widget_text_items(&document, &page, page_index, resolver, &mut items);
            Ok(OwnedPage {
                page_number: page_num,
                page_label: document.page_label(page_index),
                page_width,
                page_height,
                geometry: PageGeometry {
                    box_left: view_box.left,
                    box_bottom: view_box.bottom,
                    box_right: view_box.right,
                    box_top: view_box.top,
                    user_unit: page.user_unit(),
                    rotation_quarter_turns: u8::try_from(page.rotation())
                        .ok()
                        .filter(|turns| *turns < 4),
                },
                items,
            })
        })();

        match extracted {
            Ok(page) => owned.push(page),
            Err(error)
                if state.config.continue_on_page_error
                    && error.status == LITEPARSE_STATUS_PARSE_ERROR =>
            {
                if !state.config.quiet {
                    eprintln!(
                        "[raw-text] page {page_num} failed: {} — skipping (continue_on_page_error)",
                        error.message
                    );
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(RawTextState::pack(owned))
}

/// Destroy a raw-text handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_raw_text_free(raw_text: *mut LiteParseRawText) {
    unsafe { free_handle(raw_text) };
}

/// Return the number of extracted pages.
///
/// # Safety
///
/// `raw_text` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_raw_text_page_count(raw_text: *const LiteParseRawText) -> usize {
    unsafe { state_ref(raw_text) }.map_or(0, |state| state.pages.len())
}

/// Borrow the extracted pages.
///
/// # Safety
///
/// `raw_text` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_raw_text_pages(
    raw_text: *const LiteParseRawText,
    out_len: *mut usize,
) -> *const LiteParseRawTextPage {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(raw_text)?.page_views.as_slice()))
        })
    }
}

/// Borrow one page's raw text items.
///
/// # Safety
///
/// `raw_text` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_raw_text_items(
    raw_text: *const LiteParseRawText,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseRawTextItem {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(raw_text)?
                .item_views
                .get(page_index)
                .map(Vec::as_slice))
        })
    }
}
