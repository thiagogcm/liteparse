use liteparse_pdfium::{BitmapFormat, Color, Library, PageObject, PageObjectKind, SegmentKind};

use crate::document::DocumentState;
use crate::handle::{
    LiteParseByteView, LiteParseStr, Pool, array_ptr, bytes_view, free_handle, opaque_handles,
    packed_len, view_of, view_state,
};
use crate::records::{LiteParsePageGeometry, flag_bits};
use crate::render::{load_document, map_pages, page_facts};
use crate::status::{FfiError, FfiResult};

/// `LiteParsePageObject.kind` values.
pub const LITEPARSE_PAGE_OBJECT_TEXT: u32 = 0;
pub const LITEPARSE_PAGE_OBJECT_PATH: u32 = 1;
pub const LITEPARSE_PAGE_OBJECT_IMAGE: u32 = 2;
pub const LITEPARSE_PAGE_OBJECT_SHADING: u32 = 3;
pub const LITEPARSE_PAGE_OBJECT_FORM: u32 = 4;
pub const LITEPARSE_PAGE_OBJECT_UNKNOWN: u32 = 5;

/// `LiteParsePageObject.flags` bits.
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_MATRIX: u32 = 1 << 0;
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_BOUNDS: u32 = 1 << 1;
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_DRAW_MODE: u32 = 1 << 2;
pub const LITEPARSE_PAGE_OBJECT_FLAG_PATH_FILLED: u32 = 1 << 3;
pub const LITEPARSE_PAGE_OBJECT_FLAG_PATH_STROKED: u32 = 1 << 4;
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_WIDTH: u32 = 1 << 5;
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_FILL_COLOR: u32 = 1 << 6;
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_COLOR: u32 = 1 << 7;
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_IMAGE_METADATA: u32 = 1 << 8;

/// `LiteParsePathSegment.kind` values.
pub const LITEPARSE_PATH_SEGMENT_UNKNOWN: u32 = 0;
pub const LITEPARSE_PATH_SEGMENT_MOVETO: u32 = 1;
pub const LITEPARSE_PATH_SEGMENT_LINETO: u32 = 2;
pub const LITEPARSE_PATH_SEGMENT_BEZIERTO: u32 = 3;
/// `LiteParsePathSegment.flags` bits.
pub const LITEPARSE_PATH_SEGMENT_FLAG_CLOSE: u32 = 1 << 0;
pub const LITEPARSE_PATH_SEGMENT_FLAG_HAS_POINT: u32 = 1 << 1;

/// `LiteParsePageObject.bitmap_format` values.
pub const LITEPARSE_BITMAP_FORMAT_UNKNOWN: u32 = 0;
pub const LITEPARSE_BITMAP_FORMAT_GRAY: u32 = 1;
pub const LITEPARSE_BITMAP_FORMAT_BGR: u32 = 2;
pub const LITEPARSE_BITMAP_FORMAT_BGRX: u32 = 3;
pub const LITEPARSE_BITMAP_FORMAT_BGRA: u32 = 4;
pub const LITEPARSE_BITMAP_FORMAT_BGRA_PREMUL: u32 = 5;

/// `liteparse_document_page_objects` flag: copy the image stream as stored
/// (`FPDFImageObj_GetImageDataRaw`).
pub const LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW: u32 = 1 << 0;
/// Flag: copy the stream after lossless filters
/// (`FPDFImageObj_GetImageDataDecoded`).
pub const LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED: u32 = 1 << 1;
/// Flag: copy the image's own pixels (`FPDFImageObj_GetBitmap`), not
/// matrix-rendered.
pub const LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP: u32 = 1 << 2;

const IMAGE_PAYLOAD_FLAGS: u32 = LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW
    | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED
    | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP;

/// `LiteParsePageObjectPage.flags` bits.
pub const LITEPARSE_OBJECT_PAGE_FLAG_HAS_GEOMETRY: u32 = 1 << 0;
pub const LITEPARSE_OBJECT_PAGE_FLAG_HAS_ROTATION: u32 = 1 << 1;

/// Form XObject nesting beyond this is recorded as the form object with an
/// empty child range.
const MAX_FORM_DEPTH: usize = 32;

/// Unfiltered page content objects. Views borrow from the handle.
pub struct LiteParsePageObjects {
    _opaque: [u8; 0],
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

/// One extracted page. `object_offset/count` index the view's `objects`
/// for top-level content objects.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageObjectPage {
    pub label: LiteParseStr,
    pub geometry: LiteParsePageGeometry,
    pub page_number: u32,
    /// `LITEPARSE_OBJECT_PAGE_FLAG_*` bits.
    pub flags: u32,
    pub width: f32,
    pub height: f32,
    pub object_offset: u32,
    pub object_count: u32,
}

/// One path segment in the object's own coordinate space.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePathSegment {
    /// `LITEPARSE_PATH_SEGMENT_*`.
    pub kind: u32,
    /// `LITEPARSE_PATH_SEGMENT_FLAG_*` bits.
    pub flags: u32,
    pub x: f32,
    pub y: f32,
}

/// One content object. Ranges index the view's `objects` (direct Form
/// XObject children), `segments`, and `filters` arrays. Image payloads are
/// null unless requested.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageObject {
    pub image_raw: LiteParseByteView,
    pub image_decoded: LiteParseByteView,
    pub image_bitmap: LiteParseByteView,
    pub matrix: LiteParseMatrix,
    pub bounds: LiteParsePdfBounds,
    /// `LITEPARSE_PAGE_OBJECT_*`.
    pub kind: u32,
    /// `LITEPARSE_PAGE_OBJECT_FLAG_*` bits.
    pub flags: u32,
    pub child_offset: u32,
    pub child_count: u32,
    pub segment_offset: u32,
    pub segment_count: u32,
    pub filter_offset: u32,
    pub filter_count: u32,
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
    pub bitmap_width: i32,
    pub bitmap_height: i32,
    pub bitmap_stride: i32,
    /// `LITEPARSE_BITMAP_FORMAT_*`.
    pub bitmap_format: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParsePageObjectsView {
    /// String pool behind every `LiteParseStr` in this view.
    pub pool: *const u8,
    pub pool_len: usize,
    pub pages: *const LiteParsePageObjectPage,
    pub pages_len: usize,
    pub objects: *const LiteParsePageObject,
    pub objects_len: usize,
    pub segments: *const LiteParsePathSegment,
    pub segments_len: usize,
    pub filters: *const LiteParseStr,
    pub filters_len: usize,
}

struct OwnedObject {
    record: LiteParsePageObject,
    segments: Vec<LiteParsePathSegment>,
    filters: Vec<String>,
    image_raw: Vec<u8>,
    image_decoded: Vec<u8>,
    image_bitmap: Vec<u8>,
}

struct OwnedPage {
    page_number: u32,
    page_label: Option<String>,
    page_width: f32,
    page_height: f32,
    geometry: Option<(LiteParsePageGeometry, bool)>,
    object_offset: usize,
    object_count: usize,
}

pub(crate) struct PageObjectsState {
    /// Owns the image payloads the records borrow.
    #[allow(dead_code)]
    source: (Vec<OwnedPage>, Vec<OwnedObject>),
    /// Backing storage for `view`.
    #[allow(dead_code)]
    pool: Pool,
    #[allow(dead_code)]
    pages: Vec<LiteParsePageObjectPage>,
    #[allow(dead_code)]
    objects: Vec<LiteParsePageObject>,
    #[allow(dead_code)]
    segments: Vec<LiteParsePathSegment>,
    #[allow(dead_code)]
    filters: Vec<LiteParseStr>,
    view: LiteParsePageObjectsView,
}

view_state!(PageObjectsState => LiteParsePageObjectsView, view);

impl PageObjectsState {
    fn pack(owned_pages: Vec<OwnedPage>, owned_objects: Vec<OwnedObject>) -> Self {
        let mut pool = Pool::default();
        let mut segments = Vec::new();
        let mut filters = Vec::new();
        let mut objects = Vec::with_capacity(owned_objects.len());
        for object in &owned_objects {
            let segment_offset = segments.len();
            segments.extend_from_slice(&object.segments);
            let filter_offset = filters.len();
            filters.extend(object.filters.iter().map(|name| pool.push(name)));
            let payload = |bytes: &Vec<u8>| {
                if bytes.is_empty() {
                    LiteParseByteView::default()
                } else {
                    bytes_view(bytes)
                }
            };
            objects.push(LiteParsePageObject {
                image_raw: payload(&object.image_raw),
                image_decoded: payload(&object.image_decoded),
                image_bitmap: payload(&object.image_bitmap),
                segment_offset: packed_len(segment_offset),
                segment_count: packed_len(object.segments.len()),
                filter_offset: packed_len(filter_offset),
                filter_count: packed_len(object.filters.len()),
                ..object.record
            });
        }
        let pages: Vec<LiteParsePageObjectPage> = owned_pages
            .iter()
            .map(|page| {
                let (geometry, has_rotation) = page.geometry.unwrap_or_default();
                LiteParsePageObjectPage {
                    label: pool.push_opt(page.page_label.as_deref()),
                    geometry,
                    page_number: page.page_number,
                    flags: flag_bits(&[
                        (
                            page.geometry.is_some(),
                            LITEPARSE_OBJECT_PAGE_FLAG_HAS_GEOMETRY,
                        ),
                        (has_rotation, LITEPARSE_OBJECT_PAGE_FLAG_HAS_ROTATION),
                    ]),
                    width: page.page_width,
                    height: page.page_height,
                    object_offset: packed_len(page.object_offset),
                    object_count: packed_len(page.object_count),
                }
            })
            .collect();
        let view = LiteParsePageObjectsView {
            pool: pool.ptr(),
            pool_len: pool.len(),
            pages: array_ptr(&pages),
            pages_len: pages.len(),
            objects: array_ptr(&objects),
            objects_len: objects.len(),
            segments: array_ptr(&segments),
            segments_len: segments.len(),
            filters: array_ptr(&filters),
            filters_len: filters.len(),
        };
        Self {
            source: (owned_pages, owned_objects),
            pool,
            pages,
            objects,
            segments,
            filters,
            view,
        }
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
            let kind = match segment.kind {
                Some(SegmentKind::MoveTo) => LITEPARSE_PATH_SEGMENT_MOVETO,
                Some(SegmentKind::LineTo) => LITEPARSE_PATH_SEGMENT_LINETO,
                Some(SegmentKind::BezierTo) => LITEPARSE_PATH_SEGMENT_BEZIERTO,
                None => LITEPARSE_PATH_SEGMENT_UNKNOWN,
            };
            let (x, y) = segment.point.unwrap_or_default();
            LiteParsePathSegment {
                kind,
                flags: flag_bits(&[
                    (segment.close, LITEPARSE_PATH_SEGMENT_FLAG_CLOSE),
                    (
                        segment.point.is_some(),
                        LITEPARSE_PATH_SEGMENT_FLAG_HAS_POINT,
                    ),
                ]),
                x,
                y,
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
    let bitmap = (flags & LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP != 0)
        .then(|| object.image_bitmap())
        .flatten();
    let stroke_width = object.stroke_width();
    let fill_color = object.fill_color().map(pack_argb);
    let stroke_color = object.stroke_color().map(pack_argb);
    let record = LiteParsePageObject {
        matrix: matrix.unwrap_or_default(),
        bounds: bounds.unwrap_or_default(),
        kind: object_kind(object.kind()),
        flags: flag_bits(&[
            (matrix.is_some(), LITEPARSE_PAGE_OBJECT_FLAG_HAS_MATRIX),
            (bounds.is_some(), LITEPARSE_PAGE_OBJECT_FLAG_HAS_BOUNDS),
            (
                draw_mode.is_some(),
                LITEPARSE_PAGE_OBJECT_FLAG_HAS_DRAW_MODE,
            ),
            (
                draw_mode.is_some_and(|mode| mode.filled),
                LITEPARSE_PAGE_OBJECT_FLAG_PATH_FILLED,
            ),
            (
                draw_mode.is_some_and(|mode| mode.stroked),
                LITEPARSE_PAGE_OBJECT_FLAG_PATH_STROKED,
            ),
            (
                stroke_width.is_some(),
                LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_WIDTH,
            ),
            (
                fill_color.is_some(),
                LITEPARSE_PAGE_OBJECT_FLAG_HAS_FILL_COLOR,
            ),
            (
                stroke_color.is_some(),
                LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_COLOR,
            ),
            (
                metadata.is_some(),
                LITEPARSE_PAGE_OBJECT_FLAG_HAS_IMAGE_METADATA,
            ),
        ]),
        stroke_width: stroke_width.unwrap_or(0.0),
        fill_color: fill_color.unwrap_or(0),
        stroke_color: stroke_color.unwrap_or(0),
        image_width: metadata.map_or(0, |meta| meta.width),
        image_height: metadata.map_or(0, |meta| meta.height),
        image_horizontal_dpi: metadata.map_or(0.0, |meta| meta.horizontal_dpi),
        image_vertical_dpi: metadata.map_or(0.0, |meta| meta.vertical_dpi),
        image_bits_per_pixel: metadata.map_or(0, |meta| meta.bits_per_pixel),
        image_colorspace: metadata.map_or(0, |meta| meta.colorspace),
        image_marked_content_id: metadata.map_or(0, |meta| meta.marked_content_id),
        bitmap_width: bitmap.as_ref().map_or(0, |b| b.width()),
        bitmap_height: bitmap.as_ref().map_or(0, |b| b.height()),
        bitmap_stride: bitmap.as_ref().map_or(0, |b| b.stride()),
        bitmap_format: bitmap
            .as_ref()
            .map_or(LITEPARSE_BITMAP_FORMAT_UNKNOWN, |b| {
                bitmap_format(b.format())
            }),
        ..Default::default()
    };
    OwnedObject {
        record,
        segments,
        filters,
        image_raw,
        image_decoded,
        image_bitmap: bitmap.map_or_else(Vec::new, |b| b.buffer().to_vec()),
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
        objects[offset + index].record.child_offset = packed_len(child_offset);
        objects[offset + index].record.child_count = packed_len(child_count);
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
    liteparse::extract::apply_page_orientation_corrections(
        &document,
        &state.config.page_orientation_corrections,
    )?;
    let mut owned_objects = Vec::new();
    let owned_pages = map_pages(
        &document,
        pages,
        state.config.max_pages,
        &state.config,
        "page-objects",
        |page_num, page| {
            let facts = page_facts(page);
            let siblings: Vec<_> = (0..page.object_count())
                .filter_map(|index| page.object(index))
                .collect();
            let (object_offset, object_count) =
                pack_siblings(siblings, 0, flags, &mut owned_objects);
            Ok(OwnedPage {
                page_number: page_num,
                page_label: document.page_label(page_num as i32 - 1),
                page_width: facts.width,
                page_height: facts.height,
                geometry: facts.geometry,
                object_offset,
                object_count,
            })
        },
    )?;
    Ok(PageObjectsState::pack(owned_pages, owned_objects))
}

/// Destroy a page-objects handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_free(page_objects: *mut LiteParsePageObjects) {
    unsafe { free_handle(page_objects) };
}

/// Borrow the content objects; null for a null handle.
///
/// `page_objects` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_page_objects_view(
    page_objects: *const LiteParsePageObjects,
) -> *const LiteParsePageObjectsView {
    unsafe { view_of(page_objects) }
}
