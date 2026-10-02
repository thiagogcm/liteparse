use liteparse_pdfium::{
    BitmapFormat, Color, Library, Page, PageObject, PageObjectKind, RawPathSegment, SegmentKind,
};

use crate::budget::{bytes_of, check_result_bytes};
use crate::document::DocumentState;
use crate::handle::{
    Arenas, LiteParseArenas, LiteParseBlobRef, LiteParseStr, array_ptr, free_handle,
    opaque_handles, packed_len, view_of, view_state,
};
use crate::records::{LiteParsePageGeometry, flag_bits, has};
use crate::render::{map_pages, page_facts};
use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR};

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
/// PDFium returned no raw image data (an empty stream or extraction failure).
pub const LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_RAW_UNAVAILABLE: u32 = 1 << 9;
/// PDFium returned no decoded image data (an empty stream or extraction failure).
pub const LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_DECODED_UNAVAILABLE: u32 = 1 << 10;
/// PDFium did not return a requested image bitmap.
pub const LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_BITMAP_UNAVAILABLE: u32 = 1 << 11;
/// Clip paths were read successfully, including an empty stack (no path clip).
/// Unavailable APIs, failed reads, or PDFium's clip-size limit leave this clear.
pub const LITEPARSE_PAGE_OBJECT_FLAG_HAS_CLIP_PATHS: u32 = 1 << 12;

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

/// Maximum number of Form XObject levels, counting a top-level form as one.
pub const LITEPARSE_MAX_OBJECT_NESTING_DEPTH: u32 = 32;
/// Maximum number of page objects retained in one operation.
pub const LITEPARSE_MAX_OBJECTS: u32 = 100_000;

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

/// One path segment. Object paths use the object's own coordinates; clip
/// paths use y-up containing-form coordinates (page coordinates at top level).
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

/// One path in an object's clip stack. Its range indexes the view's `segments`.
/// Points already include the object's matrix, but not ancestor form matrices.
/// Paths are reported as-is, including curves and compound paths, not as bounds.
/// PDFium does not expose text-based clipping or the clip's fill rule.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseClipPath {
    pub segment_offset: u32,
    pub segment_count: u32,
}

/// One content object. Ranges index the view's `objects` (direct Form
/// XObject children), `segments`, `clip_paths`, and `filters` arrays. Image payloads are
/// empty when not requested, empty, or unavailable; the `*_UNAVAILABLE`
/// flags mark requested payloads PDFium did not return.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageObject {
    pub image_raw: LiteParseBlobRef,
    pub image_decoded: LiteParseBlobRef,
    pub image_bitmap: LiteParseBlobRef,
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
    pub clip_path_offset: u32,
    pub clip_path_count: u32,
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
    pub arenas: LiteParseArenas,
    pub pages: *const LiteParsePageObjectPage,
    pub pages_len: usize,
    pub objects: *const LiteParsePageObject,
    pub objects_len: usize,
    pub segments: *const LiteParsePathSegment,
    pub segments_len: usize,
    pub clip_paths: *const LiteParseClipPath,
    pub clip_paths_len: usize,
    pub filters: *const LiteParseStr,
    pub filters_len: usize,
}

/// Final arrays and arenas, written directly while walking the pages.
#[derive(Default)]
struct Packer {
    arenas: Arenas,
    pages: Vec<LiteParsePageObjectPage>,
    objects: Vec<LiteParsePageObject>,
    segments: Vec<LiteParsePathSegment>,
    clip_paths: Vec<LiteParseClipPath>,
    filters: Vec<LiteParseStr>,
}

/// Array lengths before a page, restored when the page is skipped.
struct Mark {
    pages: usize,
    pool: usize,
    blobs: usize,
    objects: usize,
    segments: usize,
    clip_paths: usize,
    filters: usize,
}

pub(crate) struct PageObjectsState {
    /// Backing storage for `view`.
    #[allow(dead_code)]
    packer: Packer,
    view: LiteParsePageObjectsView,
}

view_state!(PageObjectsState => LiteParsePageObjectsView, view);

impl Packer {
    fn bytes(&self) -> u64 {
        self.arenas.bytes()
            + bytes_of(&self.pages)
            + bytes_of(&self.objects)
            + bytes_of(&self.segments)
            + bytes_of(&self.clip_paths)
            + bytes_of(&self.filters)
    }

    fn mark(&self) -> Mark {
        Mark {
            pages: self.pages.len(),
            pool: self.arenas.pool.len(),
            blobs: self.arenas.blobs.len(),
            objects: self.objects.len(),
            segments: self.segments.len(),
            clip_paths: self.clip_paths.len(),
            filters: self.filters.len(),
        }
    }

    fn rollback(&mut self, mark: Mark) {
        self.pages.truncate(mark.pages);
        self.arenas.pool.truncate(mark.pool);
        self.arenas.blobs.truncate(mark.blobs);
        self.objects.truncate(mark.objects);
        self.segments.truncate(mark.segments);
        self.clip_paths.truncate(mark.clip_paths);
        self.filters.truncate(mark.filters);
    }

    fn page(
        &mut self,
        document: &liteparse_pdfium::Document<'_>,
        page_num: u32,
        page: &Page<'_, '_>,
        flags: u32,
    ) -> FfiResult {
        let facts = page_facts(page);
        let siblings = page_children(page, self.objects.len())?;
        let (object_offset, object_count) = self.siblings(siblings, 0, flags)?;
        let (geometry, has_rotation) = facts.geometry.unwrap_or_default();
        let label = document.page_label(page_num as i32 - 1);
        self.pages.push(LiteParsePageObjectPage {
            label: self.arenas.pool.push_opt(label.as_deref()),
            geometry,
            page_number: page_num,
            flags: flag_bits(&[
                (
                    facts.geometry.is_some(),
                    LITEPARSE_OBJECT_PAGE_FLAG_HAS_GEOMETRY,
                ),
                (has_rotation, LITEPARSE_OBJECT_PAGE_FLAG_HAS_ROTATION),
            ]),
            width: facts.width,
            height: facts.height,
            object_offset: packed_len(object_offset),
            object_count: packed_len(object_count),
        });
        check_result_bytes(self.bytes())
    }

    fn siblings(
        &mut self,
        siblings: Vec<PageObject<'_, '_>>,
        depth: usize,
        flags: u32,
    ) -> FfiResult<(usize, usize)> {
        let offset = self.objects.len();
        for object in &siblings {
            let record = self.object(object, flags)?;
            self.objects.push(record);
        }
        for (index, object) in siblings.iter().enumerate() {
            if object.kind() != PageObjectKind::Form {
                continue;
            }
            let children = form_children(object, depth, self.objects.len())?;
            let (child_offset, child_count) = self.siblings(children, depth + 1, flags)?;
            let record = &mut self.objects[offset + index];
            record.child_offset = packed_len(child_offset);
            record.child_count = packed_len(child_count);
        }
        Ok((offset, siblings.len()))
    }

    fn object(
        &mut self,
        object: &PageObject<'_, '_>,
        flags: u32,
    ) -> FfiResult<LiteParsePageObject> {
        let is_image = object.kind() == PageObjectKind::Image;
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
        let segment_offset = self.segments.len();
        self.segments.extend(
            (0..object.path_segment_count().unwrap_or(0))
                .filter_map(|index| object.path_segment(index))
                .map(path_segment),
        );
        let segment_count = self.segments.len() - segment_offset;
        let clip_path_offset = self.clip_paths.len();
        let clips = object.clip_paths();
        if let Some(paths) = &clips {
            for path in paths {
                let segment_offset = self.segments.len();
                self.segments.extend(path.iter().copied().map(path_segment));
                self.clip_paths.push(LiteParseClipPath {
                    segment_offset: packed_len(segment_offset),
                    segment_count: packed_len(path.len()),
                });
            }
        }
        // Clip records and segments count against the same result budget.
        check_result_bytes(self.bytes())?;
        let metadata = object.image_metadata();
        let filter_offset = self.filters.len();
        for index in 0..object.image_filter_count().unwrap_or(0) {
            if let Some(name) = object.image_filter(index) {
                let packed = self.arenas.pool.push(&name);
                self.filters.push(packed);
            }
        }
        let want_raw = is_image && has(flags, LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW);
        let want_decoded = is_image && has(flags, LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED);
        let want_bitmap = is_image && has(flags, LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP);
        let image_raw = want_raw.then(|| object.image_data_raw()).flatten();
        let image_decoded = want_decoded.then(|| object.image_data_decoded()).flatten();
        let bitmap = want_bitmap.then(|| object.image_bitmap()).flatten();
        let arrays = self.bytes() - self.arenas.bytes();
        let arenas = &mut self.arenas;
        let mut payload =
            |bytes: Option<&[u8]>| arenas.push_blob(bytes.unwrap_or_default(), arrays);
        let image_raw_ref = payload(image_raw.as_deref())?;
        let image_decoded_ref = payload(image_decoded.as_deref())?;
        let image_bitmap_ref = payload(bitmap.as_ref().map(|b| b.buffer()))?;
        let stroke_width = object.stroke_width();
        let fill_color = object.fill_color().map(pack_argb);
        let stroke_color = object.stroke_color().map(pack_argb);
        Ok(LiteParsePageObject {
            image_raw: image_raw_ref,
            image_decoded: image_decoded_ref,
            image_bitmap: image_bitmap_ref,
            matrix: matrix.unwrap_or_default(),
            bounds: bounds.unwrap_or_default(),
            kind: object_kind(object.kind()),
            flags: flag_bits(&[
                (matrix.is_some(), LITEPARSE_PAGE_OBJECT_FLAG_HAS_MATRIX),
                (bounds.is_some(), LITEPARSE_PAGE_OBJECT_FLAG_HAS_BOUNDS),
                (clips.is_some(), LITEPARSE_PAGE_OBJECT_FLAG_HAS_CLIP_PATHS),
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
                (
                    want_raw && image_raw.is_none(),
                    LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_RAW_UNAVAILABLE,
                ),
                (
                    want_decoded && image_decoded.is_none(),
                    LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_DECODED_UNAVAILABLE,
                ),
                (
                    want_bitmap && bitmap.is_none(),
                    LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_BITMAP_UNAVAILABLE,
                ),
            ]),
            segment_offset: packed_len(segment_offset),
            segment_count: packed_len(segment_count),
            clip_path_offset: packed_len(clip_path_offset),
            clip_path_count: packed_len(self.clip_paths.len() - clip_path_offset),
            filter_offset: packed_len(filter_offset),
            filter_count: packed_len(self.filters.len() - filter_offset),
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
        })
    }

    fn finish(mut self, errors: &[liteparse::types::PageError]) -> FfiResult<PageObjectsState> {
        self.arenas.push_errors(errors);
        check_result_bytes(self.bytes())?;
        let view = LiteParsePageObjectsView {
            arenas: self.arenas.view(),
            pages: array_ptr(&self.pages),
            pages_len: self.pages.len(),
            objects: array_ptr(&self.objects),
            objects_len: self.objects.len(),
            segments: array_ptr(&self.segments),
            segments_len: self.segments.len(),
            clip_paths: array_ptr(&self.clip_paths),
            clip_paths_len: self.clip_paths.len(),
            filters: array_ptr(&self.filters),
            filters_len: self.filters.len(),
        };
        Ok(PageObjectsState { packer: self, view })
    }
}

fn path_segment(segment: RawPathSegment) -> LiteParsePathSegment {
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

fn check_object_count(existing: usize, additional: usize) -> FfiResult {
    if existing
        .checked_add(additional)
        .is_none_or(|total| total > LITEPARSE_MAX_OBJECTS as usize)
    {
        return Err(FfiError::resource_limit(
            "page-object count exceeds the operation limit",
        ));
    }
    Ok(())
}

fn check_object_depth(depth: usize) -> FfiResult {
    if depth >= LITEPARSE_MAX_OBJECT_NESTING_DEPTH as usize {
        return Err(FfiError::resource_limit(
            "page-object nesting exceeds the operation limit",
        ));
    }
    Ok(())
}

/// Borrow `count` sibling handles once `existing + count` fits the object
/// limit, so the walk never collects past it.
fn collect_objects<'page, 'lib>(
    existing: usize,
    count: usize,
    get: impl Fn(usize) -> Option<PageObject<'page, 'lib>>,
) -> FfiResult<Vec<PageObject<'page, 'lib>>> {
    check_object_count(existing, count)?;
    (0..count)
        .map(|index| {
            get(index).ok_or_else(|| {
                FfiError::new(
                    LITEPARSE_STATUS_PARSE_ERROR,
                    format!("PDFium failed to read page object at index {index}"),
                )
            })
        })
        .collect()
}

fn page_children<'page, 'lib>(
    page: &'page Page<'_, 'lib>,
    existing: usize,
) -> FfiResult<Vec<PageObject<'page, 'lib>>> {
    collect_objects(existing, page.object_count(), |index| page.object(index))
}

fn form_children<'page, 'lib>(
    form: &PageObject<'page, 'lib>,
    depth: usize,
    existing: usize,
) -> FfiResult<Vec<PageObject<'page, 'lib>>> {
    check_object_depth(depth)?;
    let count = form.form_object_count().ok_or_else(|| {
        FfiError::new(
            LITEPARSE_STATUS_PARSE_ERROR,
            "PDFium failed to count form children",
        )
    })?;
    collect_objects(existing, count, |index| form.form_object(index))
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
    let lib = Library::try_init()?;
    let document = state.open(&lib, &state.input)?;
    let mut packer = Packer::default();
    let (_, errors) = map_pages(
        &document,
        pages,
        state.config.max_pages,
        &state.config,
        |page_num, page| {
            let mark = packer.mark();
            let packed = packer.page(&document, page_num, page, flags);
            if packed.is_err() {
                packer.rollback(mark);
            }
            packed
        },
    )?;
    packer.finish(&errors)
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

#[cfg(test)]
mod budget_tests {
    use super::*;
    use crate::status::LITEPARSE_STATUS_RESOURCE_LIMIT;

    #[test]
    fn rollback_restores_every_array_and_arena() {
        let mut packer = Packer::default();
        let fill = |packer: &mut Packer| {
            packer.pages.push(LiteParsePageObjectPage::default());
            packer.objects.push(LiteParsePageObject::default());
            packer.segments.push(LiteParsePathSegment::default());
            packer.clip_paths.push(LiteParseClipPath::default());
            let name = packer.arenas.pool.push("DCTDecode");
            packer.filters.push(name);
            packer.arenas.push_blob(&[1, 2, 3], 0).unwrap();
        };
        fill(&mut packer);
        let lengths = |packer: &Packer| {
            [
                packer.pages.len(),
                packer.objects.len(),
                packer.segments.len(),
                packer.clip_paths.len(),
                packer.filters.len(),
                packer.arenas.pool.len(),
                packer.arenas.blobs.len(),
            ]
        };
        let before = lengths(&packer);
        let mark = packer.mark();
        fill(&mut packer);
        assert_ne!(lengths(&packer), before);
        packer.rollback(mark);
        assert_eq!(lengths(&packer), before);
    }

    #[test]
    fn object_limits_reject_overflow_and_depth_without_truncating() {
        check_object_count(0, LITEPARSE_MAX_OBJECTS as usize).unwrap();
        assert_eq!(
            check_object_count(1, LITEPARSE_MAX_OBJECTS as usize)
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
        assert_eq!(
            check_object_count(usize::MAX, 1).unwrap_err().status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
        check_object_depth(LITEPARSE_MAX_OBJECT_NESTING_DEPTH as usize - 1).unwrap();
        assert_eq!(
            check_object_depth(LITEPARSE_MAX_OBJECT_NESTING_DEPTH as usize)
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
    }

    #[test]
    fn clip_paths_and_their_segments_count_against_the_result_budget() {
        let mut packer = Packer::default();
        let before = packer.bytes();
        packer.clip_paths.push(LiteParseClipPath {
            segment_offset: 0,
            segment_count: 4,
        });
        packer.segments.resize(4, LiteParsePathSegment::default());
        assert_eq!(
            packer.bytes() - before,
            (size_of::<LiteParseClipPath>() + 4 * size_of::<LiteParsePathSegment>()) as u64
        );
    }
}
