use liteparse::types::PageGeometry;
use liteparse_pdfium::{
    BitmapFormat, Color, Library, PageObject, PageObjectKind, RectF, SegmentKind,
};

use crate::document::DocumentState;
use crate::handle::{
    LiteParseByteView, bytes_view, free_handle, opaque_handles, optional_str_view, slice_out,
    state_ref,
};
use crate::render::load_document;
use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR, LiteParseStatus};
use crate::views::LiteParsePageGeometry;

/// Values for `LiteParsePageObject.kind`.
pub const LITEPARSE_PAGE_OBJECT_TEXT: u32 = 0;
pub const LITEPARSE_PAGE_OBJECT_PATH: u32 = 1;
pub const LITEPARSE_PAGE_OBJECT_IMAGE: u32 = 2;
pub const LITEPARSE_PAGE_OBJECT_SHADING: u32 = 3;
pub const LITEPARSE_PAGE_OBJECT_FORM: u32 = 4;
pub const LITEPARSE_PAGE_OBJECT_UNKNOWN: u32 = 5;

/// Values for `LiteParsePathSegment.kind`.
pub const LITEPARSE_PATH_SEGMENT_UNKNOWN: u32 = 0;
pub const LITEPARSE_PATH_SEGMENT_MOVETO: u32 = 1;
pub const LITEPARSE_PATH_SEGMENT_LINETO: u32 = 2;
pub const LITEPARSE_PATH_SEGMENT_BEZIERTO: u32 = 3;

/// Values for `LiteParsePageObject.bitmap_format`.
pub const LITEPARSE_BITMAP_FORMAT_UNKNOWN: u32 = 0;
pub const LITEPARSE_BITMAP_FORMAT_GRAY: u32 = 1;
pub const LITEPARSE_BITMAP_FORMAT_BGR: u32 = 2;
pub const LITEPARSE_BITMAP_FORMAT_BGRX: u32 = 3;
pub const LITEPARSE_BITMAP_FORMAT_BGRA: u32 = 4;
pub const LITEPARSE_BITMAP_FORMAT_BGRA_PREMUL: u32 = 5;

/// Copy the image stream as stored (`FPDFImageObj_GetImageDataRaw`).
pub const LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW: u32 = 1 << 0;
/// Copy the stream after lossless filters (`FPDFImageObj_GetImageDataDecoded`).
pub const LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED: u32 = 1 << 1;
/// Copy the image's own pixels (`FPDFImageObj_GetBitmap`), not matrix-rendered.
pub const LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP: u32 = 1 << 2;

const IMAGE_PAYLOAD_FLAGS: u32 = LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW
    | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED
    | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP;

/// Form XObject nesting beyond this is recorded as the form object with an
/// empty child range. High enough that hosts see the tree; not a liteparse
/// layout policy.
const MAX_FORM_DEPTH: usize = 32;

/// Unfiltered page content objects. Views borrow from this handle.
pub struct LiteParsePageObjects {
    _opaque: [u8; 0],
}

/// The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParsePageObjectsNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParsePageObjects,
}

opaque_handles! {
    LiteParsePageObjects => PageObjectsState, "page_objects";
}

/// Affine transform as pdfium reports it (`a b c d e f`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseMatrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

/// A y-up rectangle in the space pdfium reports for `FPDFPageObj_GetBounds`
/// (`top > bottom`; object matrix applied, ancestor form matrices not).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePdfBounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

/// One extracted page. `page_label` borrows from the handle.
/// `object_offset/count` indexes `liteparse_page_objects_objects` for
/// top-level content objects.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageObjectPage {
    pub page_number: u32,
    pub page_label: LiteParseByteView,
    pub page_width: f32,
    pub page_height: f32,
    pub geometry: LiteParsePageGeometry,
    pub object_offset: usize,
    pub object_count: usize,
    pub has_geometry: bool,
}

/// One path segment in the object's own coordinate space.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePathSegment {
    /// `LITEPARSE_PATH_SEGMENT_*`. Meaningful when `has_kind` is true.
    pub kind: u32,
    pub x: f32,
    pub y: f32,
    pub close: bool,
    pub has_kind: bool,
    pub has_point: bool,
}

/// One content object. Path segments and image filter names are offset/count
/// ranges into the handle's shared arrays. Image payload views borrow here.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageObject {
    /// `LITEPARSE_PAGE_OBJECT_*`.
    pub kind: u32,
    pub matrix: LiteParseMatrix,
    pub bounds: LiteParsePdfBounds,
    /// Direct Form XObject children in `liteparse_page_objects_objects`.
    pub child_offset: usize,
    pub child_count: usize,
    pub segment_offset: usize,
    pub segment_count: usize,
    pub stroke_width: f32,
    /// Packed ARGB when the colour space is reportable as RGB.
    pub fill_color: u32,
    pub stroke_color: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub image_horizontal_dpi: f32,
    pub image_vertical_dpi: f32,
    pub image_bits_per_pixel: u32,
    /// An `FPDF_COLORSPACE_*` value.
    pub image_colorspace: i32,
    /// `-1` when the image is not in marked content.
    pub image_marked_content_id: i32,
    pub filter_offset: usize,
    pub filter_count: usize,
    pub image_raw: LiteParseByteView,
    pub image_decoded: LiteParseByteView,
    pub image_bitmap: LiteParseByteView,
    pub bitmap_width: i32,
    pub bitmap_height: i32,
    pub bitmap_stride: i32,
    /// `LITEPARSE_BITMAP_FORMAT_*`.
    pub bitmap_format: u32,
    pub has_matrix: bool,
    pub has_bounds: bool,
    pub path_filled: bool,
    pub path_stroked: bool,
    pub has_draw_mode: bool,
    pub has_stroke_width: bool,
    pub has_fill_color: bool,
    pub has_stroke_color: bool,
    pub has_image_metadata: bool,
}

struct OwnedObject {
    kind: u32,
    matrix: Option<LiteParseMatrix>,
    bounds: Option<LiteParsePdfBounds>,
    child_offset: usize,
    child_count: usize,
    segments: Vec<LiteParsePathSegment>,
    stroke_width: Option<f32>,
    fill_color: Option<u32>,
    stroke_color: Option<u32>,
    path_filled: bool,
    path_stroked: bool,
    has_draw_mode: bool,
    image_width: u32,
    image_height: u32,
    image_horizontal_dpi: f32,
    image_vertical_dpi: f32,
    image_bits_per_pixel: u32,
    image_colorspace: i32,
    image_marked_content_id: i32,
    has_image_metadata: bool,
    filters: Vec<String>,
    image_raw: Vec<u8>,
    image_decoded: Vec<u8>,
    image_bitmap: Vec<u8>,
    bitmap_width: i32,
    bitmap_height: i32,
    bitmap_stride: i32,
    bitmap_format: u32,
}

struct OwnedPage {
    page_number: u32,
    page_label: Option<String>,
    page_width: f32,
    page_height: f32,
    geometry: PageGeometry,
    object_offset: usize,
    object_count: usize,
}

pub(crate) struct PageObjectsState {
    pages: Vec<OwnedPage>,
    page_views: Vec<LiteParsePageObjectPage>,
    objects: Vec<LiteParsePageObject>,
    segments: Vec<LiteParsePathSegment>,
    filter_strings: Vec<String>,
    filter_views: Vec<LiteParseByteView>,
    image_raw: Vec<Vec<u8>>,
    image_decoded: Vec<Vec<u8>>,
    image_bitmap: Vec<Vec<u8>>,
}

impl PageObjectsState {
    fn pack(pages: Vec<OwnedPage>, owned_objects: Vec<OwnedObject>) -> Self {
        let mut segments = Vec::new();
        let mut filter_strings = Vec::new();
        let mut image_raw = Vec::with_capacity(owned_objects.len());
        let mut image_decoded = Vec::with_capacity(owned_objects.len());
        let mut image_bitmap = Vec::with_capacity(owned_objects.len());
        let mut objects = Vec::with_capacity(owned_objects.len());

        for object in owned_objects {
            let segment_offset = segments.len();
            let segment_count = object.segments.len();
            segments.extend(object.segments);
            let filter_offset = filter_strings.len();
            let filter_count = object.filters.len();
            filter_strings.extend(object.filters);
            image_raw.push(object.image_raw);
            image_decoded.push(object.image_decoded);
            image_bitmap.push(object.image_bitmap);
            objects.push(LiteParsePageObject {
                kind: object.kind,
                matrix: object.matrix.unwrap_or_default(),
                bounds: object.bounds.unwrap_or_default(),
                child_offset: object.child_offset,
                child_count: object.child_count,
                segment_offset,
                segment_count,
                stroke_width: object.stroke_width.unwrap_or(0.0),
                fill_color: object.fill_color.unwrap_or(0),
                stroke_color: object.stroke_color.unwrap_or(0),
                image_width: object.image_width,
                image_height: object.image_height,
                image_horizontal_dpi: object.image_horizontal_dpi,
                image_vertical_dpi: object.image_vertical_dpi,
                image_bits_per_pixel: object.image_bits_per_pixel,
                image_colorspace: object.image_colorspace,
                image_marked_content_id: object.image_marked_content_id,
                filter_offset,
                filter_count,
                image_raw: LiteParseByteView::default(),
                image_decoded: LiteParseByteView::default(),
                image_bitmap: LiteParseByteView::default(),
                bitmap_width: object.bitmap_width,
                bitmap_height: object.bitmap_height,
                bitmap_stride: object.bitmap_stride,
                bitmap_format: object.bitmap_format,
                has_matrix: object.matrix.is_some(),
                has_bounds: object.bounds.is_some(),
                path_filled: object.path_filled,
                path_stroked: object.path_stroked,
                has_draw_mode: object.has_draw_mode,
                has_stroke_width: object.stroke_width.is_some(),
                has_fill_color: object.fill_color.is_some(),
                has_stroke_color: object.stroke_color.is_some(),
                has_image_metadata: object.has_image_metadata,
            });
        }

        let mut state = Self {
            pages,
            page_views: Vec::new(),
            objects,
            segments,
            filter_strings,
            filter_views: Vec::new(),
            image_raw,
            image_decoded,
            image_bitmap,
        };
        state.filter_views = state
            .filter_strings
            .iter()
            .map(|name| bytes_view(name.as_bytes()))
            .collect();
        for index in 0..state.objects.len() {
            state.objects[index].image_raw = payload_view(&state.image_raw[index]);
            state.objects[index].image_decoded = payload_view(&state.image_decoded[index]);
            state.objects[index].image_bitmap = payload_view(&state.image_bitmap[index]);
        }
        state.page_views = state
            .pages
            .iter()
            .map(|page| {
                let (geometry, has_geometry) = LiteParsePageGeometry::from_core(&page.geometry)
                    .map(|geometry| (geometry, true))
                    .unwrap_or_default();
                LiteParsePageObjectPage {
                    page_number: page.page_number,
                    page_label: optional_str_view(page.page_label.as_deref()),
                    page_width: page.page_width,
                    page_height: page.page_height,
                    geometry,
                    object_offset: page.object_offset,
                    object_count: page.object_count,
                    has_geometry,
                }
            })
            .collect();
        state
    }
}

fn payload_view(value: &[u8]) -> LiteParseByteView {
    if value.is_empty() {
        LiteParseByteView::default()
    } else {
        bytes_view(value)
    }
}

fn pack_argb(color: Color) -> u32 {
    u32::from_be_bytes([color.a, color.r, color.g, color.b])
}

fn object_kind(kind: PageObjectKind) -> u32 {
    match kind {
        PageObjectKind::Text => LITEPARSE_PAGE_OBJECT_TEXT,
        PageObjectKind::Path => LITEPARSE_PAGE_OBJECT_PATH,
        PageObjectKind::Image => LITEPARSE_PAGE_OBJECT_IMAGE,
        PageObjectKind::Shading => LITEPARSE_PAGE_OBJECT_SHADING,
        PageObjectKind::Form => LITEPARSE_PAGE_OBJECT_FORM,
        PageObjectKind::Unknown => LITEPARSE_PAGE_OBJECT_UNKNOWN,
    }
}

fn bitmap_format(format: BitmapFormat) -> u32 {
    match format {
        BitmapFormat::Unknown => LITEPARSE_BITMAP_FORMAT_UNKNOWN,
        BitmapFormat::Gray => LITEPARSE_BITMAP_FORMAT_GRAY,
        BitmapFormat::Bgr => LITEPARSE_BITMAP_FORMAT_BGR,
        BitmapFormat::Bgrx => LITEPARSE_BITMAP_FORMAT_BGRX,
        BitmapFormat::Bgra => LITEPARSE_BITMAP_FORMAT_BGRA,
        BitmapFormat::BgraPremul => LITEPARSE_BITMAP_FORMAT_BGRA_PREMUL,
    }
}

fn pack_object(object: &PageObject<'_, '_>, flags: u32) -> OwnedObject {
    let matrix = object.matrix().map(|matrix| LiteParseMatrix {
        a: matrix.a,
        b: matrix.b,
        c: matrix.c,
        d: matrix.d,
        e: matrix.e,
        f: matrix.f,
    });
    let bounds = object.bounds().map(|bounds| LiteParsePdfBounds {
        left: bounds.left,
        bottom: bounds.bottom,
        right: bounds.right,
        top: bounds.top,
    });
    let draw_mode = object.path_draw_mode();
    let segments = (0..object.path_segment_count().unwrap_or(0))
        .filter_map(|index| object.path_segment(index))
        .map(|segment| {
            let (kind, has_kind) = match segment.kind {
                Some(SegmentKind::MoveTo) => (LITEPARSE_PATH_SEGMENT_MOVETO, true),
                Some(SegmentKind::LineTo) => (LITEPARSE_PATH_SEGMENT_LINETO, true),
                Some(SegmentKind::BezierTo) => (LITEPARSE_PATH_SEGMENT_BEZIERTO, true),
                None => (LITEPARSE_PATH_SEGMENT_UNKNOWN, false),
            };
            let (x, y, has_point) = match segment.point {
                Some((x, y)) => (x, y, true),
                None => (0.0, 0.0, false),
            };
            LiteParsePathSegment {
                kind,
                x,
                y,
                close: segment.close,
                has_kind,
                has_point,
            }
        })
        .collect();
    let metadata = object.image_metadata();
    let filters = (0..object.image_filter_count().unwrap_or(0))
        .filter_map(|index| object.image_filter(index))
        .collect();
    let image_raw = if flags & LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW != 0 {
        object.image_data_raw().unwrap_or_default()
    } else {
        Vec::new()
    };
    let image_decoded = if flags & LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED != 0 {
        object.image_data_decoded().unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut image_bitmap = Vec::new();
    let mut bitmap_width = 0;
    let mut bitmap_height = 0;
    let mut bitmap_stride = 0;
    let mut bitmap_format_value = LITEPARSE_BITMAP_FORMAT_UNKNOWN;
    if flags & LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP != 0 {
        if let Some(bitmap) = object.image_bitmap() {
            image_bitmap = bitmap.buffer().to_vec();
            bitmap_width = bitmap.width();
            bitmap_height = bitmap.height();
            bitmap_stride = bitmap.stride();
            bitmap_format_value = bitmap_format(bitmap.format());
        }
    }

    OwnedObject {
        kind: object_kind(object.kind()),
        matrix,
        bounds,
        child_offset: 0,
        child_count: 0,
        segments,
        stroke_width: object.stroke_width(),
        fill_color: object.fill_color().map(pack_argb),
        stroke_color: object.stroke_color().map(pack_argb),
        path_filled: draw_mode.is_some_and(|mode| mode.filled),
        path_stroked: draw_mode.is_some_and(|mode| mode.stroked),
        has_draw_mode: draw_mode.is_some(),
        image_width: metadata.map_or(0, |meta| meta.width),
        image_height: metadata.map_or(0, |meta| meta.height),
        image_horizontal_dpi: metadata.map_or(0.0, |meta| meta.horizontal_dpi),
        image_vertical_dpi: metadata.map_or(0.0, |meta| meta.vertical_dpi),
        image_bits_per_pixel: metadata.map_or(0, |meta| meta.bits_per_pixel),
        image_colorspace: metadata.map_or(0, |meta| meta.colorspace),
        image_marked_content_id: metadata.map_or(0, |meta| meta.marked_content_id),
        has_image_metadata: metadata.is_some(),
        filters,
        image_raw,
        image_decoded,
        image_bitmap,
        bitmap_width,
        bitmap_height,
        bitmap_stride,
        bitmap_format: bitmap_format_value,
    }
}

fn pack_siblings(
    siblings: Vec<PageObject<'_, '_>>,
    depth: usize,
    flags: u32,
    objects: &mut Vec<OwnedObject>,
) -> (usize, usize) {
    let offset = objects.len();
    let count = siblings.len();
    for object in &siblings {
        objects.push(pack_object(object, flags));
    }
    if depth >= MAX_FORM_DEPTH {
        return (offset, count);
    }
    for (index, object) in siblings.iter().enumerate() {
        if object.kind() != PageObjectKind::Form {
            continue;
        }
        let children: Vec<_> = (0..object.form_object_count().unwrap_or(0))
            .filter_map(|child| object.form_object(child))
            .collect();
        let (child_offset, child_count) = pack_siblings(children, depth + 1, flags, objects);
        objects[offset + index].child_offset = child_offset;
        objects[offset + index].child_count = child_count;
    }
    (offset, count)
}

pub(crate) fn extract_page_objects(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
    flags: u32,
) -> FfiResult<PageObjectsState> {
    if flags & !IMAGE_PAYLOAD_FLAGS != 0 {
        return Err(FfiError::invalid_argument(
            "page object flags contain unknown bits; use LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_*",
        ));
    }

    let lib = Library::init();
    let document = load_document(&lib, &state.input, state.config.password.as_deref())?;
    let page_count = document.page_count().max(0) as u32;
    let mut pages = match pages {
        Some(pages) => pages,
        None => (1..=page_count).collect(),
    };
    pages.truncate(state.config.max_pages);

    let mut owned_pages = Vec::with_capacity(pages.len());
    let mut owned_objects = Vec::new();
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
            let siblings: Vec<_> = (0..page.object_count())
                .filter_map(|index| page.object(index))
                .collect();
            let (object_offset, object_count) =
                pack_siblings(siblings, 0, flags, &mut owned_objects);
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
                object_offset,
                object_count,
            })
        })();

        match extracted {
            Ok(page) => owned_pages.push(page),
            Err(error)
                if state.config.continue_on_page_error
                    && error.status == LITEPARSE_STATUS_PARSE_ERROR =>
            {
                if !state.config.quiet {
                    eprintln!(
                        "[page-objects] page {page_num} failed: {} — skipping (continue_on_page_error)",
                        error.message
                    );
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(PageObjectsState::pack(owned_pages, owned_objects))
}

/// Destroy a page-objects handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_free(page_objects: *mut LiteParsePageObjects) {
    unsafe { free_handle(page_objects) };
}

/// Return the number of extracted pages.
///
/// # Safety
///
/// `page_objects` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_page_count(
    page_objects: *const LiteParsePageObjects,
) -> usize {
    unsafe { state_ref(page_objects) }.map_or(0, |state| state.pages.len())
}

/// Borrow the extracted pages.
///
/// # Safety
///
/// `page_objects` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_pages(
    page_objects: *const LiteParsePageObjects,
    out_len: *mut usize,
) -> *const LiteParsePageObjectPage {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(page_objects)?.page_views.as_slice()))
        })
    }
}

/// Borrow the flattened content objects. Page and form ranges index this array.
///
/// # Safety
///
/// `page_objects` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_objects(
    page_objects: *const LiteParsePageObjects,
    out_len: *mut usize,
) -> *const LiteParsePageObject {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(page_objects)?.objects.as_slice()))
        })
    }
}

/// Borrow the flattened path segments. Object `segment_offset/count` indexes here.
///
/// # Safety
///
/// `page_objects` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_segments(
    page_objects: *const LiteParsePageObjects,
    out_len: *mut usize,
) -> *const LiteParsePathSegment {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(page_objects)?.segments.as_slice()))
        })
    }
}

/// Borrow image filter names. Object `filter_offset/count` indexes here.
///
/// # Safety
///
/// `page_objects` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_image_filters(
    page_objects: *const LiteParsePageObjects,
    out_len: *mut usize,
) -> *const LiteParseByteView {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(page_objects)?.filter_views.as_slice()))
        })
    }
}
