//! `repr(C)` records shared by every handle view. Records hold no `bool` and
//! no `size_t`: optionals and boolean properties are bits in a per-record
//! `flags`, collections are `u32` offset/count ranges into flat arrays owned
//! by the handle, and strings are `LiteParseStr` ranges into the handle's
//! pool. The only pointers are the `LiteParseByteView` binary payloads of
//! images and screenshots.

use liteparse::layout::LayoutCell;
use liteparse::ocr_merge::{
    ComplexityReason, LayoutComplexityReason, LayoutComplexityStats, PageComplexityStats,
};
use liteparse::types::{
    Anchor, DocumentAnnotation, DocumentMetadata, ExtractedImage, FormField, ImageRef,
    OutlineTarget, PageError, Rect, ScreenshotRect, StructureAttributeValue, VectorLine,
    VectorShape, WordBox, XfaPacket,
};

use crate::handle::{LiteParseByteView, LiteParseStr, Pool, bytes_view};

/// A rectangle in top-left-origin 72-DPI viewport space.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct LiteParseRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Visible PDF box in bottom-left-origin page space, before `user_unit`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParsePageGeometry {
    pub box_left: f32,
    pub box_bottom: f32,
    pub box_right: f32,
    pub box_top: f32,
    /// Page `/UserUnit`, normally 1.0.
    pub user_unit: f32,
    /// Clockwise quarter turns, `0..=3`; meaningful only with
    /// `LITEPARSE_PAGE_FLAG_HAS_ROTATION`.
    pub rotation_quarter_turns: u32,
}

// ---------------------------------------------------------------------------
// Constants

/// `LiteParseResultView.form_type` values (PDFium `FPDF_FORMTYPE_*`).
pub const LITEPARSE_FORM_TYPE_NONE: i32 = 0;
pub const LITEPARSE_FORM_TYPE_ACRO_FORM: i32 = 1;
pub const LITEPARSE_FORM_TYPE_XFA_FULL: i32 = 2;
pub const LITEPARSE_FORM_TYPE_XFA_FOREGROUND: i32 = 3;

/// `LiteParsePageComplexity.reasons` bits.
pub const LITEPARSE_REASON_SCANNED: u32 = 1 << 0;
pub const LITEPARSE_REASON_NO_TEXT: u32 = 1 << 1;
pub const LITEPARSE_REASON_SPARSE_TEXT: u32 = 1 << 2;
pub const LITEPARSE_REASON_EMBEDDED_IMAGES: u32 = 1 << 3;
pub const LITEPARSE_REASON_GARBLED: u32 = 1 << 4;
pub const LITEPARSE_REASON_VECTOR_TEXT: u32 = 1 << 5;
pub const LITEPARSE_REASON_ANNOTATION_TEXT: u32 = 1 << 6;

/// `LiteParsePageComplexity.layout_reasons` bits.
pub const LITEPARSE_LAYOUT_REASON_MULTI_COLUMN: u32 = 1 << 0;
pub const LITEPARSE_LAYOUT_REASON_TABLE_LIKELY: u32 = 1 << 1;
pub const LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS: u32 = 1 << 2;

/// `LiteParsePageComplexity.flags` bits.
pub const LITEPARSE_COMPLEXITY_FLAG_HAS_SUBSTANTIAL_IMAGES: u32 = 1 << 0;
pub const LITEPARSE_COMPLEXITY_FLAG_FULL_PAGE_IMAGE: u32 = 1 << 1;
pub const LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED: u32 = 1 << 2;
pub const LITEPARSE_COMPLEXITY_FLAG_NEEDS_OCR: u32 = 1 << 3;
pub const LITEPARSE_COMPLEXITY_FLAG_HAS_UNCOVERED_VECTOR_AREA: u32 = 1 << 4;
pub const LITEPARSE_COMPLEXITY_FLAG_HAS_LAYOUT: u32 = 1 << 5;
pub const LITEPARSE_COMPLEXITY_FLAG_LAYOUT_IS_COMPLEX: u32 = 1 << 6;

/// `LiteParseTextItem.flags` bits.
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE: u32 = 1 << 0;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_CONFIDENCE: u32 = 1 << 1;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_FLAGS: u32 = 1 << 2;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_HEIGHT: u32 = 1 << 3;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_ASCENT: u32 = 1 << 4;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_DESCENT: u32 = 1 << 5;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_WEIGHT: u32 = 1 << 6;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_TEXT_WIDTH: u32 = 1 << 7;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_MCID: u32 = 1 << 8;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_FILL_COLOR: u32 = 1 << 9;
pub const LITEPARSE_TEXT_ITEM_FLAG_HAS_STROKE_COLOR: u32 = 1 << 10;
pub const LITEPARSE_TEXT_ITEM_FLAG_STRIKE: u32 = 1 << 11;
pub const LITEPARSE_TEXT_ITEM_FLAG_UNICODE_MAP_ERROR: u32 = 1 << 12;
pub const LITEPARSE_TEXT_ITEM_FLAG_FONT_IS_BUGGY: u32 = 1 << 13;
pub const LITEPARSE_TEXT_ITEM_FLAG_TRAILING_SPACE_GENERATED: u32 = 1 << 14;

/// `LiteParseGraphic.kind` values.
pub const LITEPARSE_GRAPHIC_STROKE: u32 = 0;
pub const LITEPARSE_GRAPHIC_RECT: u32 = 1;
/// `LiteParseGraphic.flags` bits.
pub const LITEPARSE_GRAPHIC_FLAG_HAS_FILL_COLOR: u32 = 1 << 0;
pub const LITEPARSE_GRAPHIC_FLAG_HAS_STROKE_COLOR: u32 = 1 << 1;

/// `LiteParseStructNode.flags` bits.
pub const LITEPARSE_STRUCT_NODE_FLAG_HAS_BBOX: u32 = 1 << 0;

/// `LiteParseAnnotation.flags` bits.
pub const LITEPARSE_ANNOTATION_FLAG_HAS_RECT: u32 = 1 << 0;

/// `LiteParseFormField.flags` bits.
pub const LITEPARSE_FORM_FIELD_FLAG_HAS_OBJECT_NUMBER: u32 = 1 << 0;
pub const LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_COUNT: u32 = 1 << 1;
pub const LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_INDEX: u32 = 1 << 2;
pub const LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED: u32 = 1 << 3;
pub const LITEPARSE_FORM_FIELD_FLAG_CHECKED: u32 = 1 << 4;
pub const LITEPARSE_FORM_FIELD_FLAG_HAS_RECT: u32 = 1 << 5;

/// `LiteParseStructureNode.parent_index` for a root element.
pub const LITEPARSE_NO_PARENT: u32 = u32::MAX;

/// `LiteParseStructureAttribute.kind` values.
pub const LITEPARSE_STRUCTURE_ATTR_BOOL: u32 = 0;
pub const LITEPARSE_STRUCTURE_ATTR_NUMBER: u32 = 1;
pub const LITEPARSE_STRUCTURE_ATTR_STRING: u32 = 2;

/// `LiteParseLayoutBlock.kind` values.
pub const LITEPARSE_BLOCK_HEADING: u32 = 0;
pub const LITEPARSE_BLOCK_PARAGRAPH: u32 = 1;
pub const LITEPARSE_BLOCK_LIST_ITEM: u32 = 2;
pub const LITEPARSE_BLOCK_CODE: u32 = 3;
pub const LITEPARSE_BLOCK_TABLE: u32 = 4;
pub const LITEPARSE_BLOCK_MERGED_TABLE: u32 = 5;
pub const LITEPARSE_BLOCK_GRID_FALLBACK: u32 = 6;
pub const LITEPARSE_BLOCK_RULE: u32 = 7;
pub const LITEPARSE_BLOCK_FIGURE: u32 = 8;
/// A kind this binding does not know; rejected on input.
pub const LITEPARSE_BLOCK_UNKNOWN: u32 = 9;
/// `LiteParseLayoutBlock.flags` bits.
pub const LITEPARSE_BLOCK_FLAG_HAS_LEVEL: u32 = 1 << 0;
pub const LITEPARSE_BLOCK_FLAG_BOLD: u32 = 1 << 1;
pub const LITEPARSE_BLOCK_FLAG_ITALIC: u32 = 1 << 2;
pub const LITEPARSE_BLOCK_FLAG_HAS_ORDERED: u32 = 1 << 3;
pub const LITEPARSE_BLOCK_FLAG_ORDERED: u32 = 1 << 4;
pub const LITEPARSE_BLOCK_FLAG_HAS_BBOX: u32 = 1 << 5;
pub const LITEPARSE_BLOCK_FLAG_HAS_HEADER_ROWS: u32 = 1 << 6;
pub const LITEPARSE_BLOCK_FLAG_HAS_HEADER: u32 = 1 << 7;

/// `LiteParseLayoutCell.flags` bits.
pub const LITEPARSE_CELL_FLAG_HAS_BBOX: u32 = 1 << 0;

/// `LiteParseVectorShape.flags` / `LiteParseVectorLine.flags` bits.
pub const LITEPARSE_VECTOR_FLAG_STROKE: u32 = 1 << 0;
pub const LITEPARSE_VECTOR_FLAG_FILL: u32 = 1 << 1;
pub const LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR: u32 = 1 << 2;
pub const LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR: u32 = 1 << 3;
pub const LITEPARSE_VECTOR_FLAG_HAS_CURVE: u32 = 1 << 4;
pub const LITEPARSE_VECTOR_FLAG_HAS_STROKE_WIDTH: u32 = 1 << 5;

/// `LiteParseOutlineEntry.flags` bits.
pub const LITEPARSE_OUTLINE_FLAG_HAS_Y_PDF: u32 = 1 << 0;

/// `LiteParseXfaPacket.flags` bits.
pub const LITEPARSE_XFA_FLAG_HAS_CONTENT: u32 = 1 << 0;

/// `LiteParseDocumentMeta.flags` bits.
pub const LITEPARSE_DOC_META_FLAG_HAS_FILE_VERSION: u32 = 1 << 0;
pub const LITEPARSE_DOC_META_FLAG_HAS_IS_ENCRYPTED: u32 = 1 << 1;
pub const LITEPARSE_DOC_META_FLAG_IS_ENCRYPTED: u32 = 1 << 2;
pub const LITEPARSE_DOC_META_FLAG_HAS_SECURITY_HANDLER_REVISION: u32 = 1 << 3;
pub const LITEPARSE_DOC_META_FLAG_HAS_PERMISSIONS: u32 = 1 << 4;
pub const LITEPARSE_DOC_META_FLAG_HAS_EOF_SECTION_COUNT: u32 = 1 << 5;
pub const LITEPARSE_DOC_META_FLAG_HAS_STARTXREF_COUNT: u32 = 1 << 6;
pub const LITEPARSE_DOC_META_FLAG_HAS_TRAILER_ID_PAIR_DIFFERS: u32 = 1 << 7;
pub const LITEPARSE_DOC_META_FLAG_TRAILER_ID_PAIR_DIFFERS: u32 = 1 << 8;
pub const LITEPARSE_DOC_META_FLAG_HAS_RAW_FILE_SIZE: u32 = 1 << 9;
pub const LITEPARSE_DOC_META_FLAG_HAS_XMP_TRUNCATED: u32 = 1 << 10;
pub const LITEPARSE_DOC_META_FLAG_XMP_TRUNCATED: u32 = 1 << 11;
pub const LITEPARSE_DOC_META_FLAG_HAS_SIGNATURE_COUNT: u32 = 1 << 12;
pub const LITEPARSE_DOC_META_FLAG_HAS_SIGNATURE_BYTE_RANGE_REACHES_EOF: u32 = 1 << 13;
pub const LITEPARSE_DOC_META_FLAG_SIGNATURE_BYTE_RANGE_REACHES_EOF: u32 = 1 << 14;

/// `LiteParseScreenshot.flags` bits.
pub const LITEPARSE_SCREENSHOT_FLAG_SOLID_FILL: u32 = 1 << 0;
/// `LiteParseScreenshotRect.flags` bits.
pub const LITEPARSE_SCREENSHOT_RECT_FLAG_IS_LINE: u32 = 1 << 0;
pub const LITEPARSE_SCREENSHOT_RECT_FLAG_HAS_COLOR: u32 = 1 << 1;

/// `LiteParseProjectedLine.anchor` values.
pub const LITEPARSE_ANCHOR_LEFT: u32 = 0;
pub const LITEPARSE_ANCHOR_RIGHT: u32 = 1;
pub const LITEPARSE_ANCHOR_CENTER: u32 = 2;
pub const LITEPARSE_ANCHOR_FLOATING: u32 = 3;
/// `LiteParseProjectedLine.flags` bits.
pub const LITEPARSE_LINE_FLAG_HAS_HEADING_FONT_SIZE: u32 = 1 << 0;
pub const LITEPARSE_LINE_FLAG_HAS_MCID: u32 = 1 << 1;
pub const LITEPARSE_LINE_FLAG_ALL_BOLD: u32 = 1 << 2;
pub const LITEPARSE_LINE_FLAG_ALL_ITALIC: u32 = 1 << 3;
pub const LITEPARSE_LINE_FLAG_ALL_MONO: u32 = 1 << 4;
pub const LITEPARSE_LINE_FLAG_ALL_STRIKE: u32 = 1 << 5;
pub const LITEPARSE_LINE_FLAG_FONT_SIZE_ESTIMATED: u32 = 1 << 6;
pub const LITEPARSE_LINE_FLAG_RTL: u32 = 1 << 7;
pub const LITEPARSE_LINE_FLAG_IN_FIGURE: u32 = 1 << 8;

/// `LiteParseProjectedRegion.flags` bits.
pub const LITEPARSE_REGION_FLAG_LEAF: u32 = 1 << 0;
pub const LITEPARSE_REGION_FLAG_SPLIT: u32 = 1 << 1;
pub const LITEPARSE_REGION_FLAG_HORIZONTAL: u32 = 1 << 2;
pub const LITEPARSE_REGION_FLAG_VERTICAL: u32 = 1 << 3;

/// `LiteParsePage.flags` bits. The `HAS_*` collection bits distinguish
/// "extraction enabled, none found" from "extraction disabled".
pub const LITEPARSE_PAGE_FLAG_HAS_GEOMETRY: u32 = 1 << 0;
pub const LITEPARSE_PAGE_FLAG_HAS_ROTATION: u32 = 1 << 1;
pub const LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS: u32 = 1 << 2;
pub const LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY: u32 = 1 << 3;
pub const LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS: u32 = 1 << 4;
pub const LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS: u32 = 1 << 5;
pub const LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE: u32 = 1 << 6;
pub const LITEPARSE_PAGE_FLAG_HAS_BLOCKS: u32 = 1 << 7;
pub const LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS: u32 = 1 << 8;

/// Every defined bit of each record's `flags`; input rejects the rest.
pub(crate) const PAGE_FLAGS: u32 = LITEPARSE_PAGE_FLAG_HAS_GEOMETRY
    | LITEPARSE_PAGE_FLAG_HAS_ROTATION
    | LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS
    | LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY
    | LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS
    | LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS
    | LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE
    | LITEPARSE_PAGE_FLAG_HAS_BLOCKS
    | LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS;
pub(crate) const TEXT_ITEM_FLAGS: u32 = LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_CONFIDENCE
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_FLAGS
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_HEIGHT
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_ASCENT
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_DESCENT
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_WEIGHT
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_TEXT_WIDTH
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_MCID
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_FILL_COLOR
    | LITEPARSE_TEXT_ITEM_FLAG_HAS_STROKE_COLOR
    | LITEPARSE_TEXT_ITEM_FLAG_STRIKE
    | LITEPARSE_TEXT_ITEM_FLAG_UNICODE_MAP_ERROR
    | LITEPARSE_TEXT_ITEM_FLAG_FONT_IS_BUGGY
    | LITEPARSE_TEXT_ITEM_FLAG_TRAILING_SPACE_GENERATED;
pub(crate) const GRAPHIC_FLAGS: u32 =
    LITEPARSE_GRAPHIC_FLAG_HAS_FILL_COLOR | LITEPARSE_GRAPHIC_FLAG_HAS_STROKE_COLOR;
pub(crate) const STRUCT_NODE_FLAGS: u32 = LITEPARSE_STRUCT_NODE_FLAG_HAS_BBOX;
pub(crate) const ANNOTATION_FLAGS: u32 = LITEPARSE_ANNOTATION_FLAG_HAS_RECT;
pub(crate) const FORM_FIELD_FLAGS: u32 = LITEPARSE_FORM_FIELD_FLAG_HAS_OBJECT_NUMBER
    | LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_COUNT
    | LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_INDEX
    | LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED
    | LITEPARSE_FORM_FIELD_FLAG_CHECKED
    | LITEPARSE_FORM_FIELD_FLAG_HAS_RECT;
pub(crate) const BLOCK_FLAGS: u32 = LITEPARSE_BLOCK_FLAG_HAS_LEVEL
    | LITEPARSE_BLOCK_FLAG_BOLD
    | LITEPARSE_BLOCK_FLAG_ITALIC
    | LITEPARSE_BLOCK_FLAG_HAS_ORDERED
    | LITEPARSE_BLOCK_FLAG_ORDERED
    | LITEPARSE_BLOCK_FLAG_HAS_BBOX
    | LITEPARSE_BLOCK_FLAG_HAS_HEADER_ROWS
    | LITEPARSE_BLOCK_FLAG_HAS_HEADER;
pub(crate) const CELL_FLAGS: u32 = LITEPARSE_CELL_FLAG_HAS_BBOX;
pub(crate) const VECTOR_FLAGS: u32 = LITEPARSE_VECTOR_FLAG_STROKE
    | LITEPARSE_VECTOR_FLAG_FILL
    | LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR
    | LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR
    | LITEPARSE_VECTOR_FLAG_HAS_CURVE
    | LITEPARSE_VECTOR_FLAG_HAS_STROKE_WIDTH;
pub(crate) const OUTLINE_FLAGS: u32 = LITEPARSE_OUTLINE_FLAG_HAS_Y_PDF;
pub(crate) const COMPLEXITY_FLAGS: u32 = LITEPARSE_COMPLEXITY_FLAG_HAS_SUBSTANTIAL_IMAGES
    | LITEPARSE_COMPLEXITY_FLAG_FULL_PAGE_IMAGE
    | LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED
    | LITEPARSE_COMPLEXITY_FLAG_NEEDS_OCR
    | LITEPARSE_COMPLEXITY_FLAG_HAS_UNCOVERED_VECTOR_AREA
    | LITEPARSE_COMPLEXITY_FLAG_HAS_LAYOUT
    | LITEPARSE_COMPLEXITY_FLAG_LAYOUT_IS_COMPLEX;
pub(crate) const REASON_MASK: u32 = LITEPARSE_REASON_SCANNED
    | LITEPARSE_REASON_NO_TEXT
    | LITEPARSE_REASON_SPARSE_TEXT
    | LITEPARSE_REASON_EMBEDDED_IMAGES
    | LITEPARSE_REASON_GARBLED
    | LITEPARSE_REASON_VECTOR_TEXT
    | LITEPARSE_REASON_ANNOTATION_TEXT;
pub(crate) const LAYOUT_REASON_MASK: u32 = LITEPARSE_LAYOUT_REASON_MULTI_COLUMN
    | LITEPARSE_LAYOUT_REASON_TABLE_LIKELY
    | LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS;

// ---------------------------------------------------------------------------
// Records

/// Per-page OCR and layout complexity signals.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParsePageComplexity {
    pub page_number: u32,
    /// `LITEPARSE_COMPLEXITY_FLAG_*` bits.
    pub flags: u32,
    pub text_length: u32,
    pub image_block_count: u32,
    /// Fraction of page area covered by native text.
    pub text_coverage: f32,
    /// Summed image-bbox coverage, clamped to 1.0.
    pub image_coverage: f32,
    /// Coverage of the largest counted image.
    pub largest_image_coverage: f32,
    pub page_area: f32,
    pub uncovered_vector_area: f32,
    /// `LITEPARSE_REASON_*` bits explaining `NEEDS_OCR`.
    pub reasons: u32,
    /// Side-by-side columns; 1 means a single column.
    pub layout_column_count: u32,
    pub layout_ruled_table_count: u32,
    /// Borderless table runs found by track alignment. Overlaps
    /// `layout_ruled_table_count`; the two must not be summed.
    pub layout_text_table_run_count: u32,
    pub layout_figure_count: u32,
    pub layout_ruled_table_coverage: f32,
    pub layout_figure_coverage: f32,
    /// `LITEPARSE_LAYOUT_REASON_*` bits explaining `LAYOUT_IS_COMPLEX`.
    pub layout_reasons: u32,
}

/// One text item. Used for page items, projected spans, search matches, and
/// caller-supplied content. Ranges index the owning view's `words` and
/// `char_codes` arrays.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseTextItem {
    pub text: LiteParseStr,
    pub font_name: LiteParseStr,
    pub link: LiteParseStr,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub font_size: f32,
    pub confidence: f32,
    pub font_height: f32,
    pub font_ascent: f32,
    pub font_descent: f32,
    pub text_width: f32,
    pub font_flags: i32,
    pub font_weight: i32,
    pub mcid: i32,
    /// Packed ARGB.
    pub fill_color: u32,
    pub stroke_color: u32,
    pub char_code_offset: u32,
    pub char_code_count: u32,
    pub word_offset: u32,
    pub word_count: u32,
    /// `LITEPARSE_TEXT_ITEM_FLAG_*` bits.
    pub flags: u32,
}

/// Word box in top-left-origin 72-DPI page space.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseWordBox {
    pub text: LiteParseStr,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A layout graphic primitive in viewport space. Strokes use `x1..y2` and
/// `line_width`; rects use `bbox`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseGraphic {
    /// `LITEPARSE_GRAPHIC_*`.
    pub kind: u32,
    /// `LITEPARSE_GRAPHIC_FLAG_*` bits.
    pub flags: u32,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub line_width: f32,
    pub bbox: LiteParseRect,
    pub fill_color: u32,
    pub stroke_color: u32,
}

/// One pre-order structure-tree node used for heading and figure detection.
/// `mcid_offset/count` index the view's `mcids` array.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStructNode {
    pub role: LiteParseStr,
    pub alt_text: LiteParseStr,
    pub bbox: LiteParseRect,
    pub mcid_offset: u32,
    pub mcid_count: u32,
    /// `LITEPARSE_STRUCT_NODE_FLAG_*` bits.
    pub flags: u32,
}

/// Per-page raster image object, present even when bytes were not extracted.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseImageRef {
    pub id: LiteParseStr,
    pub format: LiteParseStr,
    pub bbox: LiteParseRect,
    pub obj_index: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub bits_per_pixel: u32,
    /// An `FPDF_COLORSPACE_*` value.
    pub colorspace: i32,
    pub rotation: f32,
}

/// Extracted image with encoded bytes.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseImage {
    pub id: LiteParseStr,
    pub name: LiteParseStr,
    pub path: LiteParseStr,
    pub format: LiteParseStr,
    pub duplicate_of: LiteParseStr,
    pub bytes: LiteParseByteView,
    pub bbox: LiteParseRect,
    pub page: u32,
    pub width: u32,
    pub height: u32,
    pub rotation: f32,
}

/// Page or structure-node annotation. `quadpoint_offset/count` index the
/// view's `quadpoints` array.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseAnnotation {
    pub subtype: LiteParseStr,
    pub contents: LiteParseStr,
    pub created: LiteParseStr,
    pub modified: LiteParseStr,
    pub title: LiteParseStr,
    pub uri: LiteParseStr,
    pub rect: LiteParseRect,
    pub quadpoint_offset: u32,
    pub quadpoint_count: u32,
    /// `LITEPARSE_ANNOTATION_FLAG_*` bits.
    pub flags: u32,
}

/// AcroForm widget. Option ranges index the view's `strings` array.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseFormField {
    pub id: LiteParseStr,
    pub field_type: LiteParseStr,
    pub name: LiteParseStr,
    pub alternate_name: LiteParseStr,
    pub value: LiteParseStr,
    pub export_value: LiteParseStr,
    pub rect: LiteParseRect,
    pub page: u32,
    pub annotation_index: i32,
    pub widget_index: i32,
    pub object_number: i32,
    pub field_flags: i32,
    pub control_count: i32,
    pub control_index: i32,
    pub option_offset: u32,
    pub option_count: u32,
    pub selected_option_offset: u32,
    pub selected_option_count: u32,
    /// `LITEPARSE_FORM_FIELD_FLAG_*` bits.
    pub flags: u32,
}

/// One tagged-PDF structure element, flattened in pre-order (parent before
/// children). `parent_index` is an absolute index into the view's
/// `structure_nodes` array or `LITEPARSE_NO_PARENT`. Ranges index the view's
/// `mcids`, `structure_attributes`, and `annotations` arrays.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStructureNode {
    pub element_type: LiteParseStr,
    pub id: LiteParseStr,
    pub actual_text: LiteParseStr,
    pub alt_text: LiteParseStr,
    pub title: LiteParseStr,
    pub parent_index: u32,
    /// Nesting depth, 0 for roots.
    pub depth: u32,
    pub mcid_offset: u32,
    pub mcid_count: u32,
    pub attribute_offset: u32,
    pub attribute_count: u32,
    pub annotation_offset: u32,
    pub annotation_count: u32,
}

/// One `/A` attribute. Booleans are `kind == BOOL` with `number` 0 or 1.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStructureAttribute {
    pub name: LiteParseStr,
    pub string: LiteParseStr,
    /// `LITEPARSE_STRUCTURE_ATTR_*`.
    pub kind: u32,
    pub number: f32,
}

/// One classified layout block. Fields that do not apply to `kind` carry
/// their absent encoding. Tables: `header_cell_offset/count` and each row's
/// `cell_offset/count` index `cells`; `row_offset/count` index `rows`.
/// `merged_table` stores header rows first (`header_rows` of them) and puts
/// colspan/rowspan on each cell. `code`/`grid_fallback` lines are
/// `line_offset/count` into `strings`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseLayoutBlock {
    pub text: LiteParseStr,
    pub marker: LiteParseStr,
    pub lang: LiteParseStr,
    /// Figure image id and encoded format.
    pub id: LiteParseStr,
    pub format: LiteParseStr,
    pub bbox: LiteParseRect,
    /// `LITEPARSE_BLOCK_*`.
    pub kind: u32,
    /// `LITEPARSE_BLOCK_FLAG_*` bits.
    pub flags: u32,
    /// Heading level (1-6), or list nesting depth.
    pub level: u32,
    pub line_offset: u32,
    pub line_count: u32,
    pub header_cell_offset: u32,
    pub header_cell_count: u32,
    pub row_offset: u32,
    pub row_count: u32,
    pub header_rows: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseLayoutCell {
    pub text: LiteParseStr,
    pub bbox: LiteParseRect,
    /// Merge span; `0` or `1` means a single cell.
    pub colspan: u32,
    pub rowspan: u32,
    /// `LITEPARSE_CELL_FLAG_*` bits.
    pub flags: u32,
}

/// Row range into `cells`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseLayoutRow {
    pub cell_offset: u32,
    pub cell_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseVectorShape {
    pub bbox: LiteParseRect,
    pub stroke_color: u32,
    pub fill_color: u32,
    /// `LITEPARSE_VECTOR_FLAG_*` bits.
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseVectorLine {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke_width: f32,
    pub stroke_color: u32,
    pub fill_color: u32,
    /// `LITEPARSE_VECTOR_FLAG_*` bits.
    pub flags: u32,
}

/// One outline entry (bookmark). `page_index` is zero-based and `-1` when the
/// destination is not a page; `y_pdf` is PDF user space.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseOutlineEntry {
    pub title: LiteParseStr,
    pub page_index: i32,
    pub y_pdf: f32,
    /// Hierarchy depth, 1-based.
    pub level: u32,
    /// `LITEPARSE_OUTLINE_FLAG_*` bits.
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParsePageError {
    pub message: LiteParseStr,
    pub page_number: u32,
}

/// XFA packet; `content` is lossily decoded UTF-8.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseXfaPacket {
    pub name: LiteParseStr,
    pub content: LiteParseStr,
    pub index: u32,
    pub content_length: u32,
    /// `LITEPARSE_XFA_FLAG_*` bits.
    pub flags: u32,
}

/// Document metadata. Absent strings are empty; scalars use flags.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseDocumentMeta {
    /// Authored `/Info` values read at document open.
    pub title: LiteParseStr,
    pub author: LiteParseStr,
    pub subject: LiteParseStr,
    pub keywords: LiteParseStr,
    pub trapped: LiteParseStr,
    pub creation_date: LiteParseStr,
    pub mod_date: LiteParseStr,
    pub xmp: LiteParseStr,
    pub permissions: u64,
    pub raw_file_size: u64,
    pub file_version: i32,
    pub security_handler_revision: i32,
    pub eof_section_count: u32,
    pub startxref_count: u32,
    pub signature_count: u32,
    /// `LITEPARSE_DOC_META_FLAG_*` bits.
    pub flags: u32,
}

/// Rendered page PNG. `rect_offset/count` index the view's
/// `screenshot_rects` array.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseScreenshot {
    pub png: LiteParseByteView,
    pub page_number: u32,
    pub width: u32,
    pub height: u32,
    /// Resolution the page was actually rendered at: the requested DPI unless
    /// the renderer lowered it to keep the long edge under 30,000 pixels. For
    /// a region render it still describes the page.
    pub effective_dpi: f32,
    pub rect_offset: u32,
    pub rect_count: u32,
    /// `LITEPARSE_SCREENSHOT_FLAG_*` bits.
    pub flags: u32,
}

/// A solid rectangle or line in top-left-origin 72-DPI viewport space. For a
/// region render the coordinates are relative to the region's origin.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseScreenshotRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Packed ARGB.
    pub color: u32,
    /// `LITEPARSE_SCREENSHOT_RECT_FLAG_*` bits.
    pub flags: u32,
}

/// One projected text line. `span_offset/count` index the view's
/// `projected_spans` array; `region_path_offset/count` index `region_paths`
/// (child ordinals from the page's region root).
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseProjectedLine {
    pub text: LiteParseStr,
    pub dominant_font_name: LiteParseStr,
    pub bbox: LiteParseRect,
    pub indent_x: f32,
    pub dominant_font_size: f32,
    pub heading_font_size: f32,
    pub mcid: i32,
    /// `LITEPARSE_ANCHOR_*`.
    pub anchor: u32,
    /// `LITEPARSE_LINE_FLAG_*` bits.
    pub flags: u32,
    pub span_offset: u32,
    pub span_count: u32,
    pub region_path_offset: u32,
    pub region_path_count: u32,
}

/// One XY-cut region, flattened in pre-order. `parent_index` and the entries
/// of `region_children` are absolute indices into `regions`. Leaves
/// correspond to the `region_paths` on projected lines; the core keeps no
/// leaf-to-item mapping that the result can expose.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseProjectedRegion {
    pub bbox: LiteParseRect,
    pub parent_index: u32,
    pub child_offset: u32,
    pub child_count: u32,
    /// `LITEPARSE_REGION_FLAG_*` bits.
    pub flags: u32,
}

/// Projected and original geometry of one text item on pages where rotation
/// handling displaced content.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseItemFrame {
    pub projected: LiteParseRect,
    pub original: LiteParseRect,
}

/// One page. Ranges index the flat arrays of the owning view; result-only
/// ranges (`figure_*`, `item_frame_*`, `projected_line_*`, `region_*`) are
/// zero on extract and content input. `text`/`markdown` are output only.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParsePage {
    /// 1-based source page number.
    pub page_number: u32,
    /// `LITEPARSE_PAGE_FLAG_*` bits.
    pub flags: u32,
    /// Viewport size in 72-DPI points.
    pub width: f32,
    pub height: f32,
    /// The PDF `/PageLabels` entry, or empty.
    pub label: LiteParseStr,
    pub text: LiteParseStr,
    pub markdown: LiteParseStr,
    pub geometry: LiteParsePageGeometry,
    pub content_bounds: LiteParseRect,
    pub complexity: LiteParsePageComplexity,
    pub item_offset: u32,
    pub item_count: u32,
    pub graphic_offset: u32,
    pub graphic_count: u32,
    pub struct_node_offset: u32,
    pub struct_node_count: u32,
    pub image_ref_offset: u32,
    pub image_ref_count: u32,
    pub annotation_offset: u32,
    pub annotation_count: u32,
    pub form_field_offset: u32,
    pub form_field_count: u32,
    pub structure_node_offset: u32,
    pub structure_node_count: u32,
    pub block_offset: u32,
    pub block_count: u32,
    pub vector_shape_offset: u32,
    pub vector_shape_count: u32,
    pub vector_line_offset: u32,
    pub vector_line_count: u32,
    pub figure_offset: u32,
    pub figure_count: u32,
    pub item_frame_offset: u32,
    pub item_frame_count: u32,
    pub projected_line_offset: u32,
    pub projected_line_count: u32,
    pub region_offset: u32,
    pub region_count: u32,
}

// ---------------------------------------------------------------------------
// Helpers shared by packers and unpackers

pub(crate) fn flag_bits(bits: &[(bool, u32)]) -> u32 {
    bits.iter()
        .filter(|(set, _)| *set)
        .fold(0, |flags, (_, bit)| flags | bit)
}

pub(crate) fn has(flags: u32, bit: u32) -> bool {
    flags & bit != 0
}

pub(crate) fn optional<T: Default + Copy>(value: Option<T>) -> (T, bool) {
    (value.unwrap_or_default(), value.is_some())
}

/// Core colours are eight lowercase ARGB hex digits.
pub(crate) fn argb_from_hex(value: &str) -> Option<u32> {
    (value.len() == 8 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

pub(crate) fn hex_from_argb(value: u32) -> String {
    format!("{value:08x}")
}

pub(crate) fn optional_color(value: Option<&str>) -> (u32, bool) {
    optional(value.and_then(argb_from_hex))
}

impl From<&Rect> for LiteParseRect {
    fn from(rect: &Rect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl From<&LiteParseRect> for Rect {
    fn from(rect: &LiteParseRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl LiteParseRect {
    pub(crate) fn is_finite(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
    }
}

pub(crate) fn optional_rect(rect: Option<&Rect>) -> (LiteParseRect, bool) {
    optional(rect.map(LiteParseRect::from))
}

pub(crate) fn anchor_value(anchor: &Anchor) -> u32 {
    match anchor {
        Anchor::Left => LITEPARSE_ANCHOR_LEFT,
        Anchor::Right => LITEPARSE_ANCHOR_RIGHT,
        Anchor::Center => LITEPARSE_ANCHOR_CENTER,
        Anchor::Floating => LITEPARSE_ANCHOR_FLOATING,
    }
}

const REASON_BITS: [(ComplexityReason, u32); 7] = [
    (ComplexityReason::Scanned, LITEPARSE_REASON_SCANNED),
    (ComplexityReason::NoText, LITEPARSE_REASON_NO_TEXT),
    (ComplexityReason::SparseText, LITEPARSE_REASON_SPARSE_TEXT),
    (
        ComplexityReason::EmbeddedImages,
        LITEPARSE_REASON_EMBEDDED_IMAGES,
    ),
    (ComplexityReason::Garbled, LITEPARSE_REASON_GARBLED),
    (ComplexityReason::VectorText, LITEPARSE_REASON_VECTOR_TEXT),
    (
        ComplexityReason::AnnotationText,
        LITEPARSE_REASON_ANNOTATION_TEXT,
    ),
];

const LAYOUT_REASON_BITS: [(LayoutComplexityReason, u32); 3] = [
    (
        LayoutComplexityReason::MultiColumn,
        LITEPARSE_LAYOUT_REASON_MULTI_COLUMN,
    ),
    (
        LayoutComplexityReason::TableLikely,
        LITEPARSE_LAYOUT_REASON_TABLE_LIKELY,
    ),
    (
        LayoutComplexityReason::DenseGraphics,
        LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS,
    ),
];

fn reason_mask<R: PartialEq + Copy>(reasons: &[R], table: &[(R, u32)]) -> u32 {
    table
        .iter()
        .filter(|(reason, _)| reasons.contains(reason))
        .fold(0, |mask, (_, bit)| mask | bit)
}

fn reasons_from_mask<R: Copy>(mask: u32, table: &[(R, u32)]) -> Vec<R> {
    table
        .iter()
        .filter(|(_, bit)| mask & bit != 0)
        .map(|(reason, _)| *reason)
        .collect()
}

pub(crate) fn clamp_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

impl From<&PageComplexityStats> for LiteParsePageComplexity {
    fn from(stats: &PageComplexityStats) -> Self {
        let (uncovered_vector_area, has_uncovered) = optional(stats.uncovered_vector_area);
        let layout = stats.layout.as_ref();
        Self {
            page_number: clamp_u32(stats.page_number),
            flags: flag_bits(&[
                (
                    stats.has_substantial_images,
                    LITEPARSE_COMPLEXITY_FLAG_HAS_SUBSTANTIAL_IMAGES,
                ),
                (
                    stats.full_page_image,
                    LITEPARSE_COMPLEXITY_FLAG_FULL_PAGE_IMAGE,
                ),
                (stats.is_garbled, LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED),
                (stats.needs_ocr, LITEPARSE_COMPLEXITY_FLAG_NEEDS_OCR),
                (
                    has_uncovered,
                    LITEPARSE_COMPLEXITY_FLAG_HAS_UNCOVERED_VECTOR_AREA,
                ),
                (layout.is_some(), LITEPARSE_COMPLEXITY_FLAG_HAS_LAYOUT),
                (
                    layout.is_some_and(|l| l.is_complex),
                    LITEPARSE_COMPLEXITY_FLAG_LAYOUT_IS_COMPLEX,
                ),
            ]),
            text_length: clamp_u32(stats.text_length),
            image_block_count: clamp_u32(stats.image_block_count),
            text_coverage: stats.text_coverage,
            image_coverage: stats.image_coverage,
            largest_image_coverage: stats.largest_image_coverage,
            page_area: stats.page_area,
            uncovered_vector_area,
            reasons: reason_mask(&stats.reasons, &REASON_BITS),
            layout_column_count: layout.map_or(0, |l| clamp_u32(l.column_count)),
            layout_ruled_table_count: layout.map_or(0, |l| clamp_u32(l.ruled_table_count)),
            layout_text_table_run_count: layout.map_or(0, |l| clamp_u32(l.text_table_run_count)),
            layout_figure_count: layout.map_or(0, |l| clamp_u32(l.figure_count)),
            layout_ruled_table_coverage: layout.map_or(0.0, |l| l.ruled_table_coverage),
            layout_figure_coverage: layout.map_or(0.0, |l| l.figure_coverage),
            layout_reasons: layout.map_or(0, |l| reason_mask(&l.reasons, &LAYOUT_REASON_BITS)),
        }
    }
}

impl From<&LiteParsePageComplexity> for PageComplexityStats {
    fn from(stats: &LiteParsePageComplexity) -> Self {
        let flags = stats.flags;
        Self {
            page_number: stats.page_number as usize,
            text_length: stats.text_length as usize,
            text_coverage: stats.text_coverage,
            has_substantial_images: has(flags, LITEPARSE_COMPLEXITY_FLAG_HAS_SUBSTANTIAL_IMAGES),
            image_block_count: stats.image_block_count as usize,
            image_coverage: stats.image_coverage,
            largest_image_coverage: stats.largest_image_coverage,
            full_page_image: has(flags, LITEPARSE_COMPLEXITY_FLAG_FULL_PAGE_IMAGE),
            uncovered_vector_area: has(flags, LITEPARSE_COMPLEXITY_FLAG_HAS_UNCOVERED_VECTOR_AREA)
                .then_some(stats.uncovered_vector_area),
            is_garbled: has(flags, LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED),
            page_area: stats.page_area,
            needs_ocr: has(flags, LITEPARSE_COMPLEXITY_FLAG_NEEDS_OCR),
            reasons: reasons_from_mask(stats.reasons, &REASON_BITS),
            layout: has(flags, LITEPARSE_COMPLEXITY_FLAG_HAS_LAYOUT).then(|| {
                LayoutComplexityStats {
                    column_count: stats.layout_column_count as usize,
                    ruled_table_count: stats.layout_ruled_table_count as usize,
                    ruled_table_coverage: stats.layout_ruled_table_coverage,
                    text_table_run_count: stats.layout_text_table_run_count as usize,
                    figure_count: stats.layout_figure_count as usize,
                    figure_coverage: stats.layout_figure_coverage,
                    is_complex: has(flags, LITEPARSE_COMPLEXITY_FLAG_LAYOUT_IS_COMPLEX),
                    reasons: reasons_from_mask(stats.layout_reasons, &LAYOUT_REASON_BITS),
                }
            }),
        }
    }
}

impl LiteParseWordBox {
    pub(crate) fn pack(pool: &mut Pool, word: &WordBox) -> Self {
        Self {
            text: pool.push(&word.text),
            x: word.x,
            y: word.y,
            width: word.width,
            height: word.height,
        }
    }
}

impl LiteParseImageRef {
    pub(crate) fn pack(pool: &mut Pool, image: &ImageRef) -> Self {
        Self {
            id: pool.push(&image.id),
            format: pool.push(&image.format),
            bbox: LiteParseRect::from(&image.bbox),
            obj_index: clamp_u32(image.obj_index),
            pixel_width: image.pixel_width,
            pixel_height: image.pixel_height,
            bits_per_pixel: image.bits_per_pixel,
            colorspace: image.colorspace,
            rotation: image.rotation,
        }
    }
}

impl LiteParseImage {
    pub(crate) fn pack(pool: &mut Pool, image: &ExtractedImage) -> Self {
        Self {
            id: pool.push(&image.id),
            name: pool.push(&image.name),
            path: pool.push_opt(image.path.as_deref()),
            format: pool.push(&image.format),
            duplicate_of: pool.push_opt(image.duplicate_of.as_deref()),
            bytes: bytes_view(&image.bytes),
            bbox: LiteParseRect::from(&image.bbox),
            page: image.page,
            width: image.width,
            height: image.height,
            rotation: image.rotation,
        }
    }
}

impl From<&ScreenshotRect> for LiteParseScreenshotRect {
    fn from(rect: &ScreenshotRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            color: argb_from_hex(&rect.color).unwrap_or(0),
            flags: flag_bits(&[
                (rect.is_line, LITEPARSE_SCREENSHOT_RECT_FLAG_IS_LINE),
                (
                    argb_from_hex(&rect.color).is_some(),
                    LITEPARSE_SCREENSHOT_RECT_FLAG_HAS_COLOR,
                ),
            ]),
        }
    }
}

impl LiteParseOutlineEntry {
    pub(crate) fn pack(pool: &mut Pool, entry: &OutlineTarget) -> Self {
        let (y_pdf, has_y_pdf) = optional(entry.y_pdf);
        Self {
            title: pool.push(&entry.title),
            page_index: entry.page_index,
            y_pdf,
            level: u32::from(entry.level),
            flags: flag_bits(&[(has_y_pdf, LITEPARSE_OUTLINE_FLAG_HAS_Y_PDF)]),
        }
    }
}

impl LiteParsePageError {
    pub(crate) fn pack(pool: &mut Pool, error: &PageError) -> Self {
        Self {
            message: pool.push(&error.message),
            page_number: error.page_number,
        }
    }
}

impl LiteParseXfaPacket {
    pub(crate) fn pack(pool: &mut Pool, packet: &XfaPacket) -> Self {
        Self {
            name: pool.push_opt(packet.name.as_deref()),
            content: pool.push_opt(packet.content.as_deref()),
            index: packet.index,
            content_length: packet.content_length,
            flags: flag_bits(&[(packet.content.is_some(), LITEPARSE_XFA_FLAG_HAS_CONTENT)]),
        }
    }
}

/// `/Info` strings read at document open; the core `DocumentMetadata` does
/// not carry them.
#[derive(Clone, Default)]
pub(crate) struct DescriptiveInfo {
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) subject: Option<String>,
    pub(crate) keywords: Option<String>,
    pub(crate) trapped: Option<String>,
}

impl LiteParseDocumentMeta {
    pub(crate) fn pack(
        pool: &mut Pool,
        meta: &DocumentMetadata,
        descriptive: Option<&DescriptiveInfo>,
    ) -> Self {
        let (file_version, has_file_version) = optional(meta.file_version);
        let (security_handler_revision, has_shr) = optional(meta.security_handler_revision);
        let (permissions, has_permissions) = optional(meta.permissions);
        let (eof_section_count, has_eof) = optional(meta.eof_section_count);
        let (startxref_count, has_startxref) = optional(meta.startxref_count);
        let (raw_file_size, has_raw_file_size) = optional(meta.raw_file_size);
        let (signature_count, has_signature_count) = optional(meta.signature_count);
        let mut describe = |pick: fn(&DescriptiveInfo) -> &Option<String>| {
            pool.push_opt(descriptive.and_then(|info| pick(info).as_deref()))
        };
        let title = describe(|info| &info.title);
        let author = describe(|info| &info.author);
        let subject = describe(|info| &info.subject);
        let keywords = describe(|info| &info.keywords);
        let trapped = describe(|info| &info.trapped);
        let flags = flag_bits(&[
            (has_file_version, LITEPARSE_DOC_META_FLAG_HAS_FILE_VERSION),
            (
                meta.is_encrypted.is_some(),
                LITEPARSE_DOC_META_FLAG_HAS_IS_ENCRYPTED,
            ),
            (
                meta.is_encrypted == Some(true),
                LITEPARSE_DOC_META_FLAG_IS_ENCRYPTED,
            ),
            (
                has_shr,
                LITEPARSE_DOC_META_FLAG_HAS_SECURITY_HANDLER_REVISION,
            ),
            (has_permissions, LITEPARSE_DOC_META_FLAG_HAS_PERMISSIONS),
            (has_eof, LITEPARSE_DOC_META_FLAG_HAS_EOF_SECTION_COUNT),
            (has_startxref, LITEPARSE_DOC_META_FLAG_HAS_STARTXREF_COUNT),
            (
                meta.trailer_id_pair_differs.is_some(),
                LITEPARSE_DOC_META_FLAG_HAS_TRAILER_ID_PAIR_DIFFERS,
            ),
            (
                meta.trailer_id_pair_differs == Some(true),
                LITEPARSE_DOC_META_FLAG_TRAILER_ID_PAIR_DIFFERS,
            ),
            (has_raw_file_size, LITEPARSE_DOC_META_FLAG_HAS_RAW_FILE_SIZE),
            (
                meta.xmp_truncated.is_some(),
                LITEPARSE_DOC_META_FLAG_HAS_XMP_TRUNCATED,
            ),
            (
                meta.xmp_truncated == Some(true),
                LITEPARSE_DOC_META_FLAG_XMP_TRUNCATED,
            ),
            (
                has_signature_count,
                LITEPARSE_DOC_META_FLAG_HAS_SIGNATURE_COUNT,
            ),
            (
                meta.signature_byte_range_reaches_eof.is_some(),
                LITEPARSE_DOC_META_FLAG_HAS_SIGNATURE_BYTE_RANGE_REACHES_EOF,
            ),
            (
                meta.signature_byte_range_reaches_eof == Some(true),
                LITEPARSE_DOC_META_FLAG_SIGNATURE_BYTE_RANGE_REACHES_EOF,
            ),
        ]);
        Self {
            title,
            author,
            subject,
            keywords,
            trapped,
            creation_date: pool.push_opt(meta.creation_date.as_deref()),
            mod_date: pool.push_opt(meta.mod_date.as_deref()),
            xmp: pool.push_opt(meta.xmp.as_deref()),
            permissions,
            raw_file_size,
            file_version,
            security_handler_revision,
            eof_section_count,
            startxref_count,
            signature_count,
            flags,
        }
    }
}

impl LiteParseAnnotation {
    pub(crate) fn pack(
        pool: &mut Pool,
        annotation: &DocumentAnnotation,
        quadpoint_offset: u32,
    ) -> Self {
        let (rect, has_rect) = optional_rect(annotation.rect.as_ref());
        Self {
            subtype: pool.push(&annotation.subtype),
            contents: pool.push_opt(annotation.contents.as_deref()),
            created: pool.push_opt(annotation.created.as_deref()),
            modified: pool.push_opt(annotation.modified.as_deref()),
            title: pool.push_opt(annotation.title.as_deref()),
            uri: pool.push_opt(annotation.uri.as_deref()),
            rect,
            quadpoint_offset,
            quadpoint_count: clamp_u32(annotation.quadpoint_rects.len()),
            flags: flag_bits(&[(has_rect, LITEPARSE_ANNOTATION_FLAG_HAS_RECT)]),
        }
    }
}

impl LiteParseFormField {
    pub(crate) fn pack(
        pool: &mut Pool,
        field: &FormField,
        option_offset: u32,
        selected_option_offset: u32,
    ) -> Self {
        let (object_number, has_object_number) = optional(field.object_number);
        let (control_count, has_control_count) = optional(field.control_count);
        let (control_index, has_control_index) = optional(field.control_index);
        let (rect, has_rect) = optional_rect(field.rect.as_ref());
        Self {
            id: pool.push(&field.id),
            field_type: pool.push(&field.field_type),
            name: pool.push_opt(field.name.as_deref()),
            alternate_name: pool.push_opt(field.alternate_name.as_deref()),
            value: pool.push_opt(field.value.as_deref()),
            export_value: pool.push_opt(field.export_value.as_deref()),
            rect,
            page: field.page,
            annotation_index: field.annotation_index,
            widget_index: field.widget_index,
            object_number,
            field_flags: field.field_flags,
            control_count,
            control_index,
            option_offset,
            option_count: clamp_u32(field.options.len()),
            selected_option_offset,
            selected_option_count: clamp_u32(field.selected_options.len()),
            flags: flag_bits(&[
                (
                    has_object_number,
                    LITEPARSE_FORM_FIELD_FLAG_HAS_OBJECT_NUMBER,
                ),
                (
                    has_control_count,
                    LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_COUNT,
                ),
                (
                    has_control_index,
                    LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_INDEX,
                ),
                (
                    field.checked.is_some(),
                    LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED,
                ),
                (
                    field.checked == Some(true),
                    LITEPARSE_FORM_FIELD_FLAG_CHECKED,
                ),
                (has_rect, LITEPARSE_FORM_FIELD_FLAG_HAS_RECT),
            ]),
        }
    }
}

pub(crate) fn structure_attribute(
    pool: &mut Pool,
    name: &str,
    value: &StructureAttributeValue,
) -> LiteParseStructureAttribute {
    let (kind, number, string) = match value {
        StructureAttributeValue::Boolean(value) => (
            LITEPARSE_STRUCTURE_ATTR_BOOL,
            f32::from(u8::from(*value)),
            LiteParseStr::default(),
        ),
        StructureAttributeValue::Number(value) => (
            LITEPARSE_STRUCTURE_ATTR_NUMBER,
            *value,
            LiteParseStr::default(),
        ),
        StructureAttributeValue::String(value) => {
            (LITEPARSE_STRUCTURE_ATTR_STRING, 0.0, pool.push(value))
        }
    };
    LiteParseStructureAttribute {
        name: pool.push(name),
        string,
        kind,
        number,
    }
}

pub(crate) fn layout_cell(pool: &mut Pool, cell: &LayoutCell) -> LiteParseLayoutCell {
    let (bbox, has_bbox) = optional_rect(cell.bbox.as_ref());
    LiteParseLayoutCell {
        text: pool.push(&cell.text),
        bbox,
        colspan: u32::from(cell.colspan.unwrap_or(0)),
        rowspan: u32::from(cell.rowspan.unwrap_or(0)),
        flags: flag_bits(&[(has_bbox, LITEPARSE_CELL_FLAG_HAS_BBOX)]),
    }
}

pub(crate) const BLOCK_KINDS: [(&str, u32); 9] = [
    ("heading", LITEPARSE_BLOCK_HEADING),
    ("paragraph", LITEPARSE_BLOCK_PARAGRAPH),
    ("list_item", LITEPARSE_BLOCK_LIST_ITEM),
    ("code", LITEPARSE_BLOCK_CODE),
    ("table", LITEPARSE_BLOCK_TABLE),
    ("merged_table", LITEPARSE_BLOCK_MERGED_TABLE),
    ("grid_fallback", LITEPARSE_BLOCK_GRID_FALLBACK),
    ("rule", LITEPARSE_BLOCK_RULE),
    ("figure", LITEPARSE_BLOCK_FIGURE),
];

pub(crate) fn block_kind(kind: &str) -> Option<u32> {
    BLOCK_KINDS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, value)| *value)
}

impl From<&VectorShape> for LiteParseVectorShape {
    fn from(shape: &VectorShape) -> Self {
        let (stroke_color, has_stroke_color) = optional_color(shape.stroke_color.as_deref());
        let (fill_color, has_fill_color) = optional_color(shape.fill_color.as_deref());
        Self {
            bbox: LiteParseRect::from(&shape.bbox),
            stroke_color,
            fill_color,
            flags: flag_bits(&[
                (shape.stroke, LITEPARSE_VECTOR_FLAG_STROKE),
                (shape.fill, LITEPARSE_VECTOR_FLAG_FILL),
                (has_stroke_color, LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR),
                (has_fill_color, LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR),
                (shape.has_curve, LITEPARSE_VECTOR_FLAG_HAS_CURVE),
            ]),
        }
    }
}

impl From<&LiteParseVectorShape> for VectorShape {
    fn from(shape: &LiteParseVectorShape) -> Self {
        Self {
            bbox: Rect::from(&shape.bbox),
            stroke: has(shape.flags, LITEPARSE_VECTOR_FLAG_STROKE),
            stroke_color: has(shape.flags, LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR)
                .then(|| hex_from_argb(shape.stroke_color)),
            fill: has(shape.flags, LITEPARSE_VECTOR_FLAG_FILL),
            fill_color: has(shape.flags, LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR)
                .then(|| hex_from_argb(shape.fill_color)),
            has_curve: has(shape.flags, LITEPARSE_VECTOR_FLAG_HAS_CURVE),
        }
    }
}

impl From<&VectorLine> for LiteParseVectorLine {
    fn from(line: &VectorLine) -> Self {
        let (stroke_width, has_stroke_width) = optional(line.stroke_width);
        let (stroke_color, has_stroke_color) = optional_color(line.stroke_color.as_deref());
        let (fill_color, has_fill_color) = optional_color(line.fill_color.as_deref());
        Self {
            x1: line.x1,
            y1: line.y1,
            x2: line.x2,
            y2: line.y2,
            stroke_width,
            stroke_color,
            fill_color,
            flags: flag_bits(&[
                (line.stroke, LITEPARSE_VECTOR_FLAG_STROKE),
                (line.fill, LITEPARSE_VECTOR_FLAG_FILL),
                (has_stroke_color, LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR),
                (has_fill_color, LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR),
                (has_stroke_width, LITEPARSE_VECTOR_FLAG_HAS_STROKE_WIDTH),
            ]),
        }
    }
}

impl From<&LiteParseVectorLine> for VectorLine {
    fn from(line: &LiteParseVectorLine) -> Self {
        Self {
            x1: line.x1,
            y1: line.y1,
            x2: line.x2,
            y2: line.y2,
            stroke: has(line.flags, LITEPARSE_VECTOR_FLAG_STROKE),
            stroke_width: has(line.flags, LITEPARSE_VECTOR_FLAG_HAS_STROKE_WIDTH)
                .then_some(line.stroke_width),
            stroke_color: has(line.flags, LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR)
                .then(|| hex_from_argb(line.stroke_color)),
            fill: has(line.flags, LITEPARSE_VECTOR_FLAG_FILL),
            fill_color: has(line.flags, LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR)
                .then(|| hex_from_argb(line.fill_color)),
        }
    }
}

pub(crate) fn views<'a, C, V: From<&'a C>>(items: &'a [C]) -> Vec<V> {
    items.iter().map(V::from).collect()
}

/// Pack every item through the pool.
pub(crate) fn pack_all<'a, C, V>(
    pool: &mut Pool,
    items: &'a [C],
    pack: fn(&mut Pool, &'a C) -> V,
) -> Vec<V> {
    items.iter().map(|item| pack(pool, item)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_round_trips_core_hex_strings() {
        assert_eq!(argb_from_hex("ff1a2b3c"), Some(0xff1a_2b3c));
        assert_eq!(hex_from_argb(0x0012_abcd), "0012abcd");
        assert_eq!(
            argb_from_hex(&hex_from_argb(0x8000_0001)),
            Some(0x8000_0001)
        );
        assert_eq!(argb_from_hex("+1234567"), None);
        assert_eq!(argb_from_hex("FF1A2B3C"), Some(0xff1a_2b3c));
        assert_eq!(argb_from_hex("abc"), None);
    }
}
