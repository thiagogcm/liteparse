use liteparse::ScreenshotResult;
use liteparse::layout::{LayoutBlock, LayoutCell};
use liteparse::ocr_merge::{ComplexityReason, LayoutComplexityReason, PageComplexityStats};
use liteparse::types::{
    CutAxis, DocumentAnnotation, DocumentMetadata, ExtractedImage, FormField, ImageRef,
    OutlineTarget, PageError, Rect, Region, RegionKind, ScreenshotRect, StructNode,
    StructureAttributeValue, StructureTree, StructureTreeElement, TextItem, VectorGraphics,
    WordBox, XfaPacket,
};

use crate::document::DescriptiveInfo;
use crate::handle::{LiteParseByteView, bytes_view, optional_str_view};

/// Page viewport size in 72-DPI points.
#[repr(C)]
#[derive(Default)]
pub struct LiteParsePageSize {
    pub width: f32,
    pub height: f32,
}

/// Visible PDF box in bottom-left-origin page space, before `user_unit`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageGeometry {
    pub box_left: f32,
    pub box_bottom: f32,
    pub box_right: f32,
    pub box_top: f32,
    /// Page `/UserUnit`, normally 1.0.
    pub user_unit: f32,
    /// Clockwise quarter turns, `0..=3`. Zero is an ordinary rotation, so
    /// absence is `has_rotation`, never this field.
    pub rotation_quarter_turns: u32,
    pub has_rotation: bool,
}

/// Optional page geometry; present values are finite with positive `user_unit`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParsePageGeometryValue {
    pub geometry: LiteParsePageGeometry,
    pub present: bool,
}

/// A rectangle in top-left-origin 72-DPI viewport space.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseRectValue {
    pub rect: LiteParseRect,
    pub present: bool,
}

/// Values for `LiteParseFormTypeValue.value`.
pub const LITEPARSE_FORM_TYPE_NONE: i32 = 0;
pub const LITEPARSE_FORM_TYPE_ACRO_FORM: i32 = 1;
pub const LITEPARSE_FORM_TYPE_XFA_FULL: i32 = 2;
pub const LITEPARSE_FORM_TYPE_XFA_FOREGROUND: i32 = 3;

#[repr(C)]
#[derive(Default)]
pub struct LiteParseFormTypeValue {
    /// One of the `LITEPARSE_FORM_TYPE_*` values.
    pub value: i32,
    pub present: bool,
}

/// Reason bits reported in `LiteParsePageComplexity.reasons_mask`.
pub const LITEPARSE_REASON_SCANNED: u32 = 1 << 0;
pub const LITEPARSE_REASON_NO_TEXT: u32 = 1 << 1;
pub const LITEPARSE_REASON_SPARSE_TEXT: u32 = 1 << 2;
pub const LITEPARSE_REASON_EMBEDDED_IMAGES: u32 = 1 << 3;
pub const LITEPARSE_REASON_GARBLED: u32 = 1 << 4;
pub const LITEPARSE_REASON_VECTOR_TEXT: u32 = 1 << 5;
pub const LITEPARSE_REASON_ANNOTATION_TEXT: u32 = 1 << 6;

/// Reason bits reported in `LiteParsePageComplexity.layout_reasons_mask`.
pub const LITEPARSE_LAYOUT_REASON_MULTI_COLUMN: u32 = 1 << 0;
pub const LITEPARSE_LAYOUT_REASON_TABLE_LIKELY: u32 = 1 << 1;
pub const LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS: u32 = 1 << 2;

/// Per-page OCR and layout complexity signals.
#[repr(C)]
#[derive(Default)]
pub struct LiteParsePageComplexity {
    pub page_number: usize,
    pub text_length: usize,
    /// Fraction of page area covered by native text.
    pub text_coverage: f32,
    pub image_block_count: usize,
    /// Summed image-bbox coverage, clamped to 1.0.
    pub image_coverage: f32,
    /// Coverage of the largest counted image.
    pub largest_image_coverage: f32,
    pub page_area: f32,
    pub uncovered_vector_area: f32,
    /// `LITEPARSE_REASON_*` bits explaining `needs_ocr`.
    pub reasons_mask: u32,
    /// Side-by-side columns; 1 means a single column.
    pub layout_column_count: usize,
    pub layout_ruled_table_count: usize,
    /// Borderless table runs found by track alignment. Overlaps
    /// `layout_ruled_table_count`; the two must not be summed.
    pub layout_text_table_run_count: usize,
    pub layout_figure_count: usize,
    /// Combined validated ruled-table area over page area, clamped to 1.0.
    pub layout_ruled_table_coverage: f32,
    /// Combined figure area over page area, clamped to 1.0.
    pub layout_figure_coverage: f32,
    /// `LITEPARSE_LAYOUT_REASON_*` bits explaining `layout_is_complex`.
    pub layout_reasons_mask: u32,
    pub has_substantial_images: bool,
    pub full_page_image: bool,
    pub is_garbled: bool,
    pub needs_ocr: bool,
    pub has_uncovered_vector_area: bool,
    pub has_layout: bool,
    pub layout_is_complex: bool,
}

#[repr(C)]
#[derive(Default)]
pub struct LiteParsePageComplexityValue {
    pub stats: LiteParsePageComplexity,
    pub present: bool,
}

/// Borrows data from its result handle. Rich metadata requires
/// `LITEPARSE_FLAG_EXTRACT_TEXT_METADATA`. As parse-content input the same
/// layout is filled by the caller; views are copied during the call.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseTextItem {
    pub text: LiteParseByteView,
    pub font_name: LiteParseByteView,
    pub link: LiteParseByteView,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub font_size: f32,
    pub confidence: f32,
    pub font_flags: i32,
    pub font_height: f32,
    pub font_ascent: f32,
    pub font_descent: f32,
    pub font_weight: i32,
    pub text_width: f32,
    pub mcid: i32,
    /// ARGB hex strings such as "ff000000".
    pub fill_color: LiteParseByteView,
    pub stroke_color: LiteParseByteView,
    /// Borrowed raw content-stream character codes.
    pub char_codes: *const u32,
    pub char_codes_len: usize,
    /// Range into `LiteParseContent.words` when this item is content input.
    /// Result accessors leave these zero and use `liteparse_result_word_boxes`.
    pub word_offset: usize,
    pub word_count: usize,
    pub has_font_size: bool,
    pub has_confidence: bool,
    pub strike: bool,
    pub has_unicode_map_error: bool,
    pub has_font_flags: bool,
    pub has_font_height: bool,
    pub has_font_ascent: bool,
    pub has_font_descent: bool,
    pub has_font_weight: bool,
    pub has_text_width: bool,
    pub font_is_buggy: bool,
    pub has_font_is_buggy: bool,
    pub has_mcid: bool,
    pub trailing_space_generated: bool,
    pub has_trailing_space_generated: bool,
}

/// Fixed-width borrowed bytes used by the projected-layout snapshot.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseLayoutBytes {
    pub ptr: *const u8,
    pub len: u64,
}

pub const LITEPARSE_PROJECTED_LAYOUT_COORDINATES: u32 = 1;
pub const LITEPARSE_PROJECTED_ANCHOR_LEFT: u32 = 0;
pub const LITEPARSE_PROJECTED_ANCHOR_RIGHT: u32 = 1;
pub const LITEPARSE_PROJECTED_ANCHOR_CENTER: u32 = 2;
pub const LITEPARSE_PROJECTED_ANCHOR_FLOATING: u32 = 3;
pub const LITEPARSE_PROJECTED_REGION_NO_PARENT: u64 = u64::MAX;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_RICH_METADATA: u32 = 1 << 0;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_WORDS: u32 = 1 << 1;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_CHAR_CODES: u32 = 1 << 2;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_REGION_TREE: u32 = 1 << 3;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_SOURCE_PROVENANCE_UNAVAILABLE: u32 = 1 << 4;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_BLOCK_ASSOCIATIONS_UNAVAILABLE: u32 = 1 << 5;
pub const LITEPARSE_PROJECTED_LAYOUT_FLAG_WORDS_SOURCE_COORDINATES: u32 = 1 << 6;
pub const LITEPARSE_PROJECTED_LAYOUT_SNAPSHOT_VERSION: u32 = 1;

pub const LITEPARSE_PROJECTED_LINE_FLAG_HAS_HEADING_FONT_SIZE: u32 = 1 << 0;
pub const LITEPARSE_PROJECTED_LINE_FLAG_HAS_DOMINANT_FONT_NAME: u32 = 1 << 1;
pub const LITEPARSE_PROJECTED_LINE_FLAG_HAS_MCID: u32 = 1 << 2;
pub const LITEPARSE_PROJECTED_LINE_FLAG_ALL_BOLD: u32 = 1 << 3;
pub const LITEPARSE_PROJECTED_LINE_FLAG_ALL_ITALIC: u32 = 1 << 4;
pub const LITEPARSE_PROJECTED_LINE_FLAG_ALL_MONO: u32 = 1 << 5;
pub const LITEPARSE_PROJECTED_LINE_FLAG_ALL_STRIKE: u32 = 1 << 6;
pub const LITEPARSE_PROJECTED_LINE_FLAG_FONT_SIZE_ESTIMATED: u32 = 1 << 7;
pub const LITEPARSE_PROJECTED_LINE_FLAG_RTL: u32 = 1 << 8;
pub const LITEPARSE_PROJECTED_LINE_FLAG_IN_FIGURE: u32 = 1 << 9;

pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_SIZE: u32 = 1 << 0;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_CONFIDENCE: u32 = 1 << 1;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_STRIKE: u32 = 1 << 2;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_UNICODE_MAP_ERROR: u32 = 1 << 3;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_FLAGS: u32 = 1 << 4;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_HEIGHT: u32 = 1 << 5;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_ASCENT: u32 = 1 << 6;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_DESCENT: u32 = 1 << 7;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_WEIGHT: u32 = 1 << 8;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_TEXT_WIDTH: u32 = 1 << 9;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_FONT_IS_BUGGY: u32 = 1 << 10;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_MCID: u32 = 1 << 11;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_TRAILING_SPACE_GENERATED: u32 = 1 << 12;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_CHAR_CODES: u32 = 1 << 13;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_WORDS: u32 = 1 << 14;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_NAME: u32 = 1 << 15;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_LINK: u32 = 1 << 16;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FILL_COLOR: u32 = 1 << 17;
pub const LITEPARSE_PROJECTED_SPAN_FLAG_HAS_STROKE_COLOR: u32 = 1 << 18;

pub const LITEPARSE_PROJECTED_REGION_FLAG_LEAF: u32 = 1 << 0;
pub const LITEPARSE_PROJECTED_REGION_FLAG_SPLIT: u32 = 1 << 1;
pub const LITEPARSE_PROJECTED_REGION_FLAG_HORIZONTAL: u32 = 1 << 2;
pub const LITEPARSE_PROJECTED_REGION_FLAG_VERTICAL: u32 = 1 << 3;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedSpan {
    pub text: LiteParseLayoutBytes,
    pub font_name: LiteParseLayoutBytes,
    pub link: LiteParseLayoutBytes,
    pub fill_color: LiteParseLayoutBytes,
    pub stroke_color: LiteParseLayoutBytes,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub font_size: f32,
    pub confidence: f32,
    pub font_flags: i32,
    pub font_height: f32,
    pub font_ascent: f32,
    pub font_descent: f32,
    pub font_weight: i32,
    pub text_width: f32,
    pub mcid: i32,
    pub char_code_offset: u64,
    pub char_code_count: u64,
    pub word_offset: u64,
    pub word_count: u64,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedWord {
    pub text: LiteParseLayoutBytes,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedLine {
    pub text: LiteParseLayoutBytes,
    pub dominant_font_name: LiteParseLayoutBytes,
    pub bbox: LiteParseRect,
    pub indent_x: f32,
    pub dominant_font_size: f32,
    pub heading_font_size: f32,
    pub mcid: i32,
    pub anchor: u32,
    pub flags: u32,
    pub span_offset: u64,
    pub span_count: u64,
    pub region_path_offset: u64,
    pub region_path_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedRegion {
    pub bbox: LiteParseRect,
    pub parent_index: u64,
    pub child_offset: u64,
    pub child_count: u64,
    pub item_offset: u64,
    pub item_count: u64,
    pub flags: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedLayoutPage {
    pub page_number: u32,
    pub coordinate_space: u32,
    pub flags: u32,
    pub width: f32,
    pub height: f32,
    pub line_offset: u64,
    pub line_count: u64,
    pub span_offset: u64,
    pub span_count: u64,
    pub word_offset: u64,
    pub word_count: u64,
    pub char_code_offset: u64,
    pub char_code_count: u64,
    pub region_path_offset: u64,
    pub region_path_count: u64,
    pub region_offset: u64,
    pub region_count: u64,
    pub region_item_offset: u64,
    pub region_item_count: u64,
    pub region_child_offset: u64,
    pub region_child_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedLayoutSnapshot {
    pub version: u32,
    pub coordinate_space: u32,
    pub flags: u32,
    pub reserved: u32,
    pub page_count: u64,
    pub line_count: u64,
    pub span_count: u64,
    pub word_count: u64,
    pub char_code_count: u64,
    pub region_path_count: u64,
    pub region_count: u64,
    pub region_item_count: u64,
    pub region_child_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseProjectedLayoutAbi {
    pub version: u32,
    pub layout_bytes_size: u32,
    pub layout_bytes_align: u32,
    pub projected_page_size: u32,
    pub projected_page_align: u32,
    pub projected_line_size: u32,
    pub projected_line_align: u32,
    pub projected_span_size: u32,
    pub projected_span_align: u32,
    pub projected_word_size: u32,
    pub projected_word_align: u32,
    pub projected_region_size: u32,
    pub projected_region_align: u32,
    pub line_span_offset: u32,
    pub line_region_path_offset: u32,
    pub span_char_code_offset: u32,
    pub span_word_offset: u32,
    pub page_region_offset: u32,
    pub region_child_offset: u32,
    pub layout_bytes_len_offset: u32,
}

pub const LITEPARSE_PROJECTED_LAYOUT_ABI_VERSION: u32 = 1;

/// Word box in top-left-origin 72-DPI page space.
#[repr(C)]
pub struct LiteParseWordBox {
    pub text: LiteParseByteView,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Extracted image borrowing data from its result handle.
#[repr(C)]
pub struct LiteParseImage {
    pub id: LiteParseByteView,
    pub name: LiteParseByteView,
    pub path: LiteParseByteView,
    pub format: LiteParseByteView,
    pub duplicate_of: LiteParseByteView,
    pub page: u32,
    pub width: u32,
    pub height: u32,
    pub rotation: f32,
    pub bbox: LiteParseRect,
    pub bytes: LiteParseByteView,
}

/// Per-page image object. Present even when decoded bytes were not requested.
#[repr(C)]
pub struct LiteParseImageRef {
    pub id: LiteParseByteView,
    pub format: LiteParseByteView,
    pub bbox: LiteParseRect,
    pub obj_index: usize,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rotation: f32,
}

/// One pre-order structure-tree node used for heading and figure detection.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseStructNode {
    pub role: LiteParseByteView,
    pub alt_text: LiteParseByteView,
    pub mcids: *const i32,
    pub mcids_len: usize,
    pub bbox: LiteParseRect,
    pub has_bbox: bool,
}

/// Screenshot borrowing PNG data from its owning handle.
#[repr(C)]
pub struct LiteParseScreenshot {
    pub page_number: u32,
    pub width: u32,
    pub height: u32,
    pub png: LiteParseByteView,
    /// Resolution the page was actually rendered at: the requested DPI unless
    /// the renderer lowered it to keep the long edge under 30,000 pixels. For
    /// a region render it still describes the page, so viewport geometry
    /// scales by it either way.
    pub effective_dpi: f32,
    pub is_solid_fill: bool,
}

/// A solid rectangle or line in top-left-origin 72-DPI viewport space. For a
/// region render the coordinates are relative to the region's origin.
#[repr(C)]
pub struct LiteParseScreenshotRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: LiteParseByteView,
    pub is_line: bool,
}

/// One outline entry (bookmark). `page_index` is zero-based and `-1` when the
/// destination is not a page; `y_pdf` is PDF user space.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseOutlineEntry {
    pub level: u8,
    pub title: LiteParseByteView,
    pub page_index: i32,
    pub y_pdf: f32,
    pub has_y_pdf: bool,
}

#[repr(C)]
pub struct LiteParsePageError {
    pub page_number: u32,
    pub message: LiteParseByteView,
}

/// XFA packet borrowing data from its result handle.
#[repr(C)]
pub struct LiteParseXfaPacket {
    pub index: u32,
    pub name: LiteParseByteView,
    pub content_length: u32,
    /// Packet content, lossily decoded UTF-8; null view when unreadable.
    pub content: LiteParseByteView,
}

/// Optional scalars use `has_*`; absent strings are null views.
#[repr(C)]
#[derive(Default)]
pub struct LiteParseDocumentMeta {
    /// Authored `/Info` values read at document open.
    pub title: LiteParseByteView,
    pub author: LiteParseByteView,
    pub subject: LiteParseByteView,
    pub keywords: LiteParseByteView,
    pub trapped: LiteParseByteView,
    pub creation_date: LiteParseByteView,
    pub mod_date: LiteParseByteView,
    pub file_version: i32,
    pub security_handler_revision: i32,
    pub permissions: u64,
    pub eof_section_count: u32,
    pub startxref_count: u32,
    pub raw_file_size: u64,
    pub xmp: LiteParseByteView,
    pub signature_count: u32,
    pub has_file_version: bool,
    pub is_encrypted: bool,
    pub has_is_encrypted: bool,
    pub has_security_handler_revision: bool,
    pub has_permissions: bool,
    pub has_eof_section_count: bool,
    pub has_startxref_count: bool,
    pub trailer_id_pair_differs: bool,
    pub has_trailer_id_pair_differs: bool,
    pub has_raw_file_size: bool,
    pub xmp_truncated: bool,
    pub has_xmp_truncated: bool,
    pub has_signature_count: bool,
    pub signature_byte_range_reaches_eof: bool,
    pub has_signature_byte_range_reaches_eof: bool,
}

#[repr(C)]
#[derive(Default)]
pub struct LiteParseDocumentMetaValue {
    pub meta: LiteParseDocumentMeta,
    pub present: bool,
}

/// Page annotation borrowing strings from its result handle.
#[repr(C)]
pub struct LiteParseAnnotation {
    pub subtype: LiteParseByteView,
    pub contents: LiteParseByteView,
    pub created: LiteParseByteView,
    pub modified: LiteParseByteView,
    pub title: LiteParseByteView,
    pub uri: LiteParseByteView,
    pub rect: LiteParseRect,
    /// Number of quadpoint rectangles; fetch them with
    /// `liteparse_result_annotation_quadpoints`.
    pub quadpoint_count: usize,
    pub has_rect: bool,
}

/// AcroForm widget borrowing strings from its result handle.
#[repr(C)]
pub struct LiteParseFormField {
    pub id: LiteParseByteView,
    pub field_type: LiteParseByteView,
    pub name: LiteParseByteView,
    pub alternate_name: LiteParseByteView,
    pub value: LiteParseByteView,
    pub export_value: LiteParseByteView,
    pub page: u32,
    pub annotation_index: i32,
    pub widget_index: i32,
    pub object_number: i32,
    pub field_flags: i32,
    pub control_count: i32,
    pub control_index: i32,
    pub rect: LiteParseRect,
    pub options_len: usize,
    pub selected_options_len: usize,
    pub has_object_number: bool,
    pub has_control_count: bool,
    pub has_control_index: bool,
    pub checked: bool,
    pub has_checked: bool,
    pub has_rect: bool,
}

/// Values for `LiteParseStructureAttribute.kind`.
pub const LITEPARSE_STRUCTURE_ATTR_BOOL: u32 = 0;
pub const LITEPARSE_STRUCTURE_ATTR_NUMBER: u32 = 1;
pub const LITEPARSE_STRUCTURE_ATTR_STRING: u32 = 2;

/// One node of a page's structure tree, pre-flattened in pre-order
/// (parent before children). `parent_index` is `-1` for roots. Attribute and
/// annotation ranges index into the flattened arrays returned by
/// `liteparse_result_structure_attributes` / `_annotations`; marked-content
/// ids point into storage owned by the result handle.
#[repr(C)]
pub struct LiteParseStructureNode {
    pub element_type: LiteParseByteView,
    pub id: LiteParseByteView,
    pub actual_text: LiteParseByteView,
    pub alt_text: LiteParseByteView,
    pub title: LiteParseByteView,
    /// Index of the parent node in the same slice, or -1 for a root.
    pub parent_index: i32,
    /// Nesting depth, 0 for roots.
    pub depth: u32,
    /// Range into the flattened id array from
    /// `liteparse_result_structure_marked_content_ids`.
    pub marked_content_id_offset: usize,
    pub marked_content_ids_len: usize,
    pub attribute_offset: usize,
    pub attribute_count: usize,
    pub annotation_offset: usize,
    pub annotation_count: usize,
}

#[repr(C)]
pub struct LiteParseStructureAttribute {
    pub name: LiteParseByteView,
    /// One of the `LITEPARSE_STRUCTURE_ATTR_*` values.
    pub kind: u32,
    pub number_value: f32,
    pub string_value: LiteParseByteView,
    pub bool_value: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseLayoutCell {
    pub text: LiteParseByteView,
    pub bbox: LiteParseRect,
    /// Merge span; `0` or `1` means a single cell. Packed from `merged_table`.
    pub colspan: u16,
    pub rowspan: u16,
    pub has_bbox: bool,
}

/// Row range into `liteparse_result_block_cells`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseLayoutRow {
    pub cell_offset: usize,
    pub cell_count: usize,
}

/// One classified layout block. Variant-specific fields that do not apply to
/// `kind` carry their absent encoding (`has_*` false, null views).
///
/// Table geometry: `header_cell_offset/count` indexes the page's packed cell
/// array; `first_row/row_count` indexes the packed row array from
/// `liteparse_result_block_rows`. Verbatim source lines (`code`,
/// `grid_fallback`) live in the packed line array from
/// `liteparse_result_block_lines`, indexed by `line_offset/line_count`.
/// `merged_table` stores every row in that row range (header rows first,
/// counted by `header_rows`) and puts colspan/rowspan on each cell.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseLayoutBlock {
    /// One of `heading`, `paragraph`, `list_item`, `code`, `table`,
    /// `merged_table`, `grid_fallback`, `rule`, `figure`.
    pub kind: LiteParseByteView,
    pub text: LiteParseByteView,
    /// Heading level (1-6), or list nesting depth.
    pub level: u8,
    pub marker: LiteParseByteView,
    pub lang: LiteParseByteView,
    pub line_offset: usize,
    pub line_count: usize,
    pub header_cell_offset: usize,
    pub header_cell_count: usize,
    pub first_row: usize,
    pub row_count: usize,
    /// `merged_table`: how many leading rows of the packed row range are header.
    pub header_rows: usize,
    /// Figure image id and encoded format.
    pub id: LiteParseByteView,
    pub format: LiteParseByteView,
    pub bbox: LiteParseRect,
    pub has_level: bool,
    pub bold: bool,
    pub italic: bool,
    pub ordered: bool,
    pub has_ordered: bool,
    pub has_bbox: bool,
    pub has_header_rows: bool,
}

#[repr(C)]
pub struct LiteParseVectorShape {
    pub bbox: LiteParseRect,
    pub stroke_color: LiteParseByteView,
    pub fill_color: LiteParseByteView,
    pub stroke: bool,
    pub fill: bool,
    pub has_curve: bool,
}

#[repr(C)]
pub struct LiteParseVectorLine {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke_width: f32,
    pub stroke_color: LiteParseByteView,
    pub fill_color: LiteParseByteView,
    pub stroke: bool,
    pub has_stroke_width: bool,
    pub fill: bool,
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

impl From<Option<&Rect>> for LiteParseRectValue {
    fn from(rect: Option<&Rect>) -> Self {
        let (rect, present) = optional_rect(rect);
        Self { present, rect }
    }
}

fn optional<T: Default + Copy>(value: Option<T>) -> (T, bool) {
    (value.unwrap_or_default(), value.is_some())
}

fn optional_rect(rect: Option<&Rect>) -> (LiteParseRect, bool) {
    optional(rect.map(LiteParseRect::from))
}

fn fixed_bytes(value: &[u8]) -> LiteParseLayoutBytes {
    LiteParseLayoutBytes {
        ptr: value.as_ptr(),
        len: u64::try_from(value.len()).expect("layout byte length does not fit u64"),
    }
}

fn optional_fixed_str(value: Option<&str>) -> LiteParseLayoutBytes {
    value.map_or_else(LiteParseLayoutBytes::default, |value| {
        fixed_bytes(value.as_bytes())
    })
}

fn packed_len(value: usize) -> u64 {
    u64::try_from(value).expect("layout array length does not fit u64")
}

fn flag_bits(bits: &[(bool, u32)]) -> u32 {
    bits.iter()
        .filter(|(set, _)| *set)
        .fold(0, |flags, (_, bit)| flags | bit)
}

fn layout_flags(
    metadata_enabled: bool,
    has_words: bool,
    has_char_codes: bool,
    has_regions: bool,
) -> u32 {
    LITEPARSE_PROJECTED_LAYOUT_FLAG_SOURCE_PROVENANCE_UNAVAILABLE
        | LITEPARSE_PROJECTED_LAYOUT_FLAG_BLOCK_ASSOCIATIONS_UNAVAILABLE
        | LITEPARSE_PROJECTED_LAYOUT_FLAG_WORDS_SOURCE_COORDINATES
        | flag_bits(&[
            (
                metadata_enabled,
                LITEPARSE_PROJECTED_LAYOUT_FLAG_RICH_METADATA,
            ),
            (has_words, LITEPARSE_PROJECTED_LAYOUT_FLAG_WORDS),
            (has_char_codes, LITEPARSE_PROJECTED_LAYOUT_FLAG_CHAR_CODES),
            (has_regions, LITEPARSE_PROJECTED_LAYOUT_FLAG_REGION_TREE),
        ])
}

fn anchor_value(anchor: &liteparse::types::Anchor) -> u32 {
    match anchor {
        liteparse::types::Anchor::Left => LITEPARSE_PROJECTED_ANCHOR_LEFT,
        liteparse::types::Anchor::Right => LITEPARSE_PROJECTED_ANCHOR_RIGHT,
        liteparse::types::Anchor::Center => LITEPARSE_PROJECTED_ANCHOR_CENTER,
        liteparse::types::Anchor::Floating => LITEPARSE_PROJECTED_ANCHOR_FLOATING,
    }
}

pub(crate) struct ProjectedLayoutPacked {
    pub(crate) snapshot: LiteParseProjectedLayoutSnapshot,
    pub(crate) pages: Vec<LiteParseProjectedLayoutPage>,
    pub(crate) lines: Vec<LiteParseProjectedLine>,
    pub(crate) spans: Vec<LiteParseProjectedSpan>,
    pub(crate) words: Vec<LiteParseProjectedWord>,
    pub(crate) char_codes: Vec<u32>,
    pub(crate) region_paths: Vec<u16>,
    pub(crate) regions: Vec<LiteParseProjectedRegion>,
    pub(crate) region_items: Vec<u64>,
    pub(crate) region_children: Vec<u64>,
}

impl ProjectedLayoutPacked {
    pub(crate) fn pack(pages: &[liteparse::ParsedPage], metadata_enabled: bool) -> Self {
        let all_lines = || pages.iter().flat_map(|page| &page.projected_lines);
        let all_spans = || all_lines().flat_map(|line| &line.spans);
        let mut packed = Self {
            snapshot: LiteParseProjectedLayoutSnapshot::default(),
            pages: Vec::with_capacity(pages.len()),
            lines: Vec::with_capacity(all_lines().count()),
            spans: Vec::with_capacity(all_spans().count()),
            words: Vec::with_capacity(all_spans().map(|span| span.words.len()).sum()),
            char_codes: Vec::new(),
            region_paths: Vec::new(),
            regions: Vec::new(),
            region_items: Vec::new(),
            region_children: Vec::new(),
        };

        for page in pages {
            let line_offset = packed.lines.len();
            let span_offset = packed.spans.len();
            let word_offset = packed.words.len();
            let char_code_offset = packed.char_codes.len();
            let region_path_offset = packed.region_paths.len();
            let region_offset = packed.regions.len();
            let region_item_offset = packed.region_items.len();
            let region_child_offset = packed.region_children.len();
            flatten_region(
                &page.regions,
                LITEPARSE_PROJECTED_REGION_NO_PARENT,
                &mut packed,
            );

            for line in &page.projected_lines {
                let line_span_offset = packed.spans.len();
                for span in &line.spans {
                    let span_word_offset = packed.words.len();
                    packed
                        .words
                        .extend(span.words.iter().map(|word| LiteParseProjectedWord {
                            text: fixed_bytes(word.text.as_bytes()),
                            x: word.x,
                            y: word.y,
                            width: word.width,
                            height: word.height,
                        }));

                    let metadata = span.text_metadata(metadata_enabled);
                    let span_char_code_offset = packed.char_codes.len();
                    if let Some(char_codes) = metadata.char_codes {
                        packed.char_codes.extend_from_slice(char_codes);
                    }

                    let (font_size, has_font_size) = optional(span.font_size);
                    let (confidence, has_confidence) = optional(span.confidence);
                    let (font_flags, has_font_flags) = optional(span.font_flags);
                    let (font_height, has_font_height) = optional(metadata.font_height);
                    let (font_ascent, has_font_ascent) = optional(metadata.font_ascent);
                    let (font_descent, has_font_descent) = optional(metadata.font_descent);
                    let (font_weight, has_font_weight) = optional(metadata.font_weight);
                    let (text_width, has_text_width) = optional(metadata.text_width);
                    let (mcid, has_mcid) = optional(metadata.mcid);

                    let flags = flag_bits(&[
                        (has_font_size, LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_SIZE),
                        (has_confidence, LITEPARSE_PROJECTED_SPAN_FLAG_HAS_CONFIDENCE),
                        (span.strike, LITEPARSE_PROJECTED_SPAN_FLAG_STRIKE),
                        (
                            span.has_unicode_map_error,
                            LITEPARSE_PROJECTED_SPAN_FLAG_UNICODE_MAP_ERROR,
                        ),
                        (has_font_flags, LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_FLAGS),
                        (
                            has_font_height,
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_HEIGHT,
                        ),
                        (
                            has_font_ascent,
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_ASCENT,
                        ),
                        (
                            has_font_descent,
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_DESCENT,
                        ),
                        (
                            has_font_weight,
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_WEIGHT,
                        ),
                        (has_text_width, LITEPARSE_PROJECTED_SPAN_FLAG_HAS_TEXT_WIDTH),
                        (
                            metadata.font_is_buggy == Some(true),
                            LITEPARSE_PROJECTED_SPAN_FLAG_FONT_IS_BUGGY,
                        ),
                        (has_mcid, LITEPARSE_PROJECTED_SPAN_FLAG_HAS_MCID),
                        (
                            metadata.trailing_space_generated == Some(true),
                            LITEPARSE_PROJECTED_SPAN_FLAG_TRAILING_SPACE_GENERATED,
                        ),
                        (
                            packed.char_codes.len() > span_char_code_offset,
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_CHAR_CODES,
                        ),
                        (
                            packed.words.len() > span_word_offset,
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_WORDS,
                        ),
                        (
                            span.font_name.is_some(),
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FONT_NAME,
                        ),
                        (span.link.is_some(), LITEPARSE_PROJECTED_SPAN_FLAG_HAS_LINK),
                        (
                            metadata.fill_color.is_some(),
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_FILL_COLOR,
                        ),
                        (
                            metadata.stroke_color.is_some(),
                            LITEPARSE_PROJECTED_SPAN_FLAG_HAS_STROKE_COLOR,
                        ),
                    ]);

                    packed.spans.push(LiteParseProjectedSpan {
                        text: fixed_bytes(span.text.as_bytes()),
                        font_name: optional_fixed_str(span.font_name.as_deref()),
                        link: optional_fixed_str(span.link.as_deref()),
                        fill_color: optional_fixed_str(metadata.fill_color),
                        stroke_color: optional_fixed_str(metadata.stroke_color),
                        x: span.x,
                        y: span.y,
                        width: span.width,
                        height: span.height,
                        rotation: span.rotation,
                        font_size,
                        confidence,
                        font_flags,
                        font_height,
                        font_ascent,
                        font_descent,
                        font_weight,
                        text_width,
                        mcid,
                        char_code_offset: packed_len(span_char_code_offset),
                        char_code_count: packed_len(
                            packed.char_codes.len() - span_char_code_offset,
                        ),
                        word_offset: packed_len(span_word_offset),
                        word_count: packed_len(packed.words.len() - span_word_offset),
                        flags,
                    });
                }

                let line_region_path_offset = packed.region_paths.len();
                packed.region_paths.extend_from_slice(&line.region_path);
                let (heading_font_size, has_heading_font_size) = optional(line.heading_font_size);
                let (mcid, has_mcid) = optional(line.mcid);
                let flags = flag_bits(&[
                    (
                        has_heading_font_size,
                        LITEPARSE_PROJECTED_LINE_FLAG_HAS_HEADING_FONT_SIZE,
                    ),
                    (
                        line.dominant_font_name.is_some(),
                        LITEPARSE_PROJECTED_LINE_FLAG_HAS_DOMINANT_FONT_NAME,
                    ),
                    (has_mcid, LITEPARSE_PROJECTED_LINE_FLAG_HAS_MCID),
                    (line.all_bold, LITEPARSE_PROJECTED_LINE_FLAG_ALL_BOLD),
                    (line.all_italic, LITEPARSE_PROJECTED_LINE_FLAG_ALL_ITALIC),
                    (line.all_mono, LITEPARSE_PROJECTED_LINE_FLAG_ALL_MONO),
                    (line.all_strike, LITEPARSE_PROJECTED_LINE_FLAG_ALL_STRIKE),
                    (
                        line.font_size_is_estimated,
                        LITEPARSE_PROJECTED_LINE_FLAG_FONT_SIZE_ESTIMATED,
                    ),
                    (line.rtl, LITEPARSE_PROJECTED_LINE_FLAG_RTL),
                    (line.in_figure, LITEPARSE_PROJECTED_LINE_FLAG_IN_FIGURE),
                ]);

                packed.lines.push(LiteParseProjectedLine {
                    text: fixed_bytes(line.text.as_bytes()),
                    dominant_font_name: optional_fixed_str(line.dominant_font_name.as_deref()),
                    bbox: LiteParseRect::from(&line.bbox),
                    indent_x: line.indent_x,
                    dominant_font_size: line.dominant_font_size,
                    heading_font_size,
                    mcid,
                    anchor: anchor_value(&line.anchor),
                    flags,
                    span_offset: packed_len(line_span_offset),
                    span_count: packed_len(packed.spans.len() - line_span_offset),
                    region_path_offset: packed_len(line_region_path_offset),
                    region_path_count: packed_len(line.region_path.len()),
                });
            }

            let flags = layout_flags(
                metadata_enabled,
                packed.words.len() > word_offset,
                packed.char_codes.len() > char_code_offset,
                packed.regions.len() > region_offset,
            );
            packed.pages.push(LiteParseProjectedLayoutPage {
                page_number: u32::try_from(page.page_number).expect("page number does not fit u32"),
                coordinate_space: LITEPARSE_PROJECTED_LAYOUT_COORDINATES,
                flags,
                width: page.page_width,
                height: page.page_height,
                line_offset: packed_len(line_offset),
                line_count: packed_len(packed.lines.len() - line_offset),
                span_offset: packed_len(span_offset),
                span_count: packed_len(packed.spans.len() - span_offset),
                word_offset: packed_len(word_offset),
                word_count: packed_len(packed.words.len() - word_offset),
                char_code_offset: packed_len(char_code_offset),
                char_code_count: packed_len(packed.char_codes.len() - char_code_offset),
                region_path_offset: packed_len(region_path_offset),
                region_path_count: packed_len(packed.region_paths.len() - region_path_offset),
                region_offset: packed_len(region_offset),
                region_count: packed_len(packed.regions.len() - region_offset),
                region_item_offset: packed_len(region_item_offset),
                region_item_count: packed_len(packed.region_items.len() - region_item_offset),
                region_child_offset: packed_len(region_child_offset),
                region_child_count: packed_len(packed.region_children.len() - region_child_offset),
            });
        }

        let flags = layout_flags(
            metadata_enabled,
            !packed.words.is_empty(),
            !packed.char_codes.is_empty(),
            !packed.regions.is_empty(),
        );
        packed.snapshot = LiteParseProjectedLayoutSnapshot {
            version: LITEPARSE_PROJECTED_LAYOUT_SNAPSHOT_VERSION,
            coordinate_space: LITEPARSE_PROJECTED_LAYOUT_COORDINATES,
            flags,
            reserved: 0,
            page_count: packed_len(packed.pages.len()),
            line_count: packed_len(packed.lines.len()),
            span_count: packed_len(packed.spans.len()),
            word_count: packed_len(packed.words.len()),
            char_code_count: packed_len(packed.char_codes.len()),
            region_path_count: packed_len(packed.region_paths.len()),
            region_count: packed_len(packed.regions.len()),
            region_item_count: packed_len(packed.region_items.len()),
            region_child_count: packed_len(packed.region_children.len()),
        };
        packed
    }
}

fn flatten_region(region: &Region, parent_index: u64, packed: &mut ProjectedLayoutPacked) -> u64 {
    let node_index = packed.regions.len() as u64;
    packed.regions.push(LiteParseProjectedRegion {
        bbox: LiteParseRect::from(&region.bbox),
        parent_index,
        ..Default::default()
    });
    let item_offset = packed.region_items.len();
    let (flags, child_indices) = match &region.kind {
        RegionKind::Leaf { item_indices } => {
            packed
                .region_items
                .extend(item_indices.iter().map(|index| *index as u64));
            (LITEPARSE_PROJECTED_REGION_FLAG_LEAF, Vec::new())
        }
        RegionKind::Split { axis, children } => {
            let child_indices: Vec<u64> = children
                .iter()
                .map(|child| flatten_region(child, node_index, packed))
                .collect();
            let axis_flag = match axis {
                CutAxis::Horizontal => LITEPARSE_PROJECTED_REGION_FLAG_HORIZONTAL,
                CutAxis::Vertical => LITEPARSE_PROJECTED_REGION_FLAG_VERTICAL,
            };
            (
                LITEPARSE_PROJECTED_REGION_FLAG_SPLIT | axis_flag,
                child_indices,
            )
        }
    };
    // Appended after the recursion so a node's children stay contiguous even
    // when its descendants are split nodes that append their own children.
    let child_offset = packed.region_children.len();
    packed.region_children.extend_from_slice(&child_indices);
    let node = &mut packed.regions[node_index as usize];
    node.child_offset = packed_len(child_offset);
    node.child_count = packed_len(child_indices.len());
    node.item_offset = packed_len(item_offset);
    node.item_count = packed_len(packed.region_items.len() - item_offset);
    node.flags = flags;
    node_index
}

pub fn projected_layout_abi() -> LiteParseProjectedLayoutAbi {
    use std::mem::{align_of, offset_of, size_of};
    LiteParseProjectedLayoutAbi {
        version: LITEPARSE_PROJECTED_LAYOUT_ABI_VERSION,
        layout_bytes_size: size_of::<LiteParseLayoutBytes>() as u32,
        layout_bytes_align: align_of::<LiteParseLayoutBytes>() as u32,
        projected_page_size: size_of::<LiteParseProjectedLayoutPage>() as u32,
        projected_page_align: align_of::<LiteParseProjectedLayoutPage>() as u32,
        projected_line_size: size_of::<LiteParseProjectedLine>() as u32,
        projected_line_align: align_of::<LiteParseProjectedLine>() as u32,
        projected_span_size: size_of::<LiteParseProjectedSpan>() as u32,
        projected_span_align: align_of::<LiteParseProjectedSpan>() as u32,
        projected_word_size: size_of::<LiteParseProjectedWord>() as u32,
        projected_word_align: align_of::<LiteParseProjectedWord>() as u32,
        projected_region_size: size_of::<LiteParseProjectedRegion>() as u32,
        projected_region_align: align_of::<LiteParseProjectedRegion>() as u32,
        line_span_offset: offset_of!(LiteParseProjectedLine, span_offset) as u32,
        line_region_path_offset: offset_of!(LiteParseProjectedLine, region_path_offset) as u32,
        span_char_code_offset: offset_of!(LiteParseProjectedSpan, char_code_offset) as u32,
        span_word_offset: offset_of!(LiteParseProjectedSpan, word_offset) as u32,
        page_region_offset: offset_of!(LiteParseProjectedLayoutPage, region_offset) as u32,
        region_child_offset: offset_of!(LiteParseProjectedRegion, child_offset) as u32,
        layout_bytes_len_offset: offset_of!(LiteParseLayoutBytes, len) as u32,
    }
}

impl LiteParseTextItem {
    pub(crate) fn borrow(item: &TextItem, metadata_enabled: bool) -> Self {
        let metadata = item.text_metadata(metadata_enabled);
        let (font_size, has_font_size) = optional(item.font_size);
        let (confidence, has_confidence) = optional(item.confidence);
        let (font_flags, has_font_flags) = optional(item.font_flags);
        let (font_height, has_font_height) = optional(metadata.font_height);
        let (font_ascent, has_font_ascent) = optional(metadata.font_ascent);
        let (font_descent, has_font_descent) = optional(metadata.font_descent);
        let (font_weight, has_font_weight) = optional(metadata.font_weight);
        let (text_width, has_text_width) = optional(metadata.text_width);
        let (font_is_buggy, has_font_is_buggy) = optional(metadata.font_is_buggy);
        let (mcid, has_mcid) = optional(metadata.mcid);
        let (trailing_space_generated, has_trailing_space_generated) =
            optional(metadata.trailing_space_generated);
        Self {
            text: bytes_view(item.text.as_bytes()),
            font_name: optional_str_view(item.font_name.as_deref()),
            link: optional_str_view(item.link.as_deref()),
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            rotation: item.rotation,
            font_size,
            has_font_size,
            confidence,
            has_confidence,
            strike: item.strike,
            has_unicode_map_error: item.has_unicode_map_error,
            font_flags,
            has_font_flags,
            font_height,
            has_font_height,
            font_ascent,
            has_font_ascent,
            font_descent,
            has_font_descent,
            font_weight,
            has_font_weight,
            text_width,
            has_text_width,
            font_is_buggy,
            has_font_is_buggy,
            mcid,
            has_mcid,
            fill_color: optional_str_view(metadata.fill_color),
            stroke_color: optional_str_view(metadata.stroke_color),
            char_codes: metadata
                .char_codes
                .map_or(std::ptr::null(), <[u32]>::as_ptr),
            char_codes_len: metadata.char_codes.map_or(0, <[u32]>::len),
            word_offset: 0,
            word_count: 0,
            trailing_space_generated,
            has_trailing_space_generated,
        }
    }
}

impl From<&WordBox> for LiteParseWordBox {
    fn from(word: &WordBox) -> Self {
        Self {
            text: bytes_view(word.text.as_bytes()),
            x: word.x,
            y: word.y,
            width: word.width,
            height: word.height,
        }
    }
}

impl LiteParseStructNode {
    pub(crate) fn borrow(node: &StructNode, mcids: &[i32]) -> Self {
        let (bbox, has_bbox) = optional_rect(node.bbox.as_ref());
        Self {
            role: bytes_view(node.role.as_bytes()),
            alt_text: optional_str_view(node.alt_text.as_deref()),
            mcids: if mcids.is_empty() {
                std::ptr::null()
            } else {
                mcids.as_ptr()
            },
            mcids_len: mcids.len(),
            bbox,
            has_bbox,
        }
    }
}

impl From<&ImageRef> for LiteParseImageRef {
    fn from(image: &ImageRef) -> Self {
        Self {
            id: bytes_view(image.id.as_bytes()),
            format: bytes_view(image.format.as_bytes()),
            bbox: LiteParseRect::from(&image.bbox),
            obj_index: image.obj_index,
            pixel_width: image.pixel_width,
            pixel_height: image.pixel_height,
            rotation: image.rotation,
        }
    }
}

impl From<&ExtractedImage> for LiteParseImage {
    fn from(image: &ExtractedImage) -> Self {
        Self {
            id: bytes_view(image.id.as_bytes()),
            name: bytes_view(image.name.as_bytes()),
            path: optional_str_view(image.path.as_deref()),
            format: bytes_view(image.format.as_bytes()),
            duplicate_of: optional_str_view(image.duplicate_of.as_deref()),
            page: image.page,
            width: image.width,
            height: image.height,
            rotation: image.rotation,
            bbox: LiteParseRect::from(&image.bbox),
            bytes: bytes_view(&image.bytes),
        }
    }
}

impl LiteParseScreenshot {
    pub(crate) fn borrow(screenshot: &ScreenshotResult, effective_dpi: f32) -> Self {
        Self {
            page_number: screenshot.page_num,
            width: screenshot.width,
            height: screenshot.height,
            effective_dpi,
            is_solid_fill: screenshot.is_solid_fill,
            png: bytes_view(&screenshot.image_bytes),
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
            is_line: rect.is_line,
            color: bytes_view(rect.color.as_bytes()),
        }
    }
}

impl From<&OutlineTarget> for LiteParseOutlineEntry {
    fn from(entry: &OutlineTarget) -> Self {
        let (y_pdf, has_y_pdf) = optional(entry.y_pdf);
        Self {
            level: entry.level,
            title: bytes_view(entry.title.as_bytes()),
            page_index: entry.page_index,
            y_pdf,
            has_y_pdf,
        }
    }
}

impl From<&PageError> for LiteParsePageError {
    fn from(error: &PageError) -> Self {
        Self {
            page_number: error.page_number,
            message: bytes_view(error.message.as_bytes()),
        }
    }
}

impl From<&XfaPacket> for LiteParseXfaPacket {
    fn from(packet: &XfaPacket) -> Self {
        Self {
            index: packet.index,
            name: optional_str_view(packet.name.as_deref()),
            content_length: packet.content_length,
            content: optional_str_view(packet.content.as_deref()),
        }
    }
}

impl LiteParseDocumentMeta {
    pub(crate) fn build(meta: &DocumentMetadata, descriptive: Option<&DescriptiveInfo>) -> Self {
        let (file_version, has_file_version) = optional(meta.file_version);
        let (is_encrypted, has_is_encrypted) = optional(meta.is_encrypted);
        let (security_handler_revision, has_security_handler_revision) =
            optional(meta.security_handler_revision);
        let (permissions, has_permissions) = optional(meta.permissions);
        let (eof_section_count, has_eof_section_count) = optional(meta.eof_section_count);
        let (startxref_count, has_startxref_count) = optional(meta.startxref_count);
        let (trailer_id_pair_differs, has_trailer_id_pair_differs) =
            optional(meta.trailer_id_pair_differs);
        let (raw_file_size, has_raw_file_size) = optional(meta.raw_file_size);
        let (xmp_truncated, has_xmp_truncated) = optional(meta.xmp_truncated);
        let (signature_count, has_signature_count) = optional(meta.signature_count);
        let (signature_byte_range_reaches_eof, has_signature_byte_range_reaches_eof) =
            optional(meta.signature_byte_range_reaches_eof);
        let describe = |pick: fn(&DescriptiveInfo) -> &Option<String>| {
            optional_str_view(descriptive.and_then(|info| pick(info).as_deref()))
        };
        Self {
            title: describe(|info| &info.title),
            author: describe(|info| &info.author),
            subject: describe(|info| &info.subject),
            keywords: describe(|info| &info.keywords),
            trapped: describe(|info| &info.trapped),
            creation_date: optional_str_view(meta.creation_date.as_deref()),
            mod_date: optional_str_view(meta.mod_date.as_deref()),
            file_version,
            has_file_version,
            is_encrypted,
            has_is_encrypted,
            security_handler_revision,
            has_security_handler_revision,
            permissions,
            has_permissions,
            eof_section_count,
            has_eof_section_count,
            startxref_count,
            has_startxref_count,
            trailer_id_pair_differs,
            has_trailer_id_pair_differs,
            raw_file_size,
            has_raw_file_size,
            xmp: optional_str_view(meta.xmp.as_deref()),
            xmp_truncated,
            has_xmp_truncated,
            signature_count,
            has_signature_count,
            signature_byte_range_reaches_eof,
            has_signature_byte_range_reaches_eof,
        }
    }
}

impl From<&DocumentAnnotation> for LiteParseAnnotation {
    fn from(annotation: &DocumentAnnotation) -> Self {
        let (rect, has_rect) = optional_rect(annotation.rect.as_ref());
        Self {
            subtype: bytes_view(annotation.subtype.as_bytes()),
            contents: optional_str_view(annotation.contents.as_deref()),
            created: optional_str_view(annotation.created.as_deref()),
            modified: optional_str_view(annotation.modified.as_deref()),
            title: optional_str_view(annotation.title.as_deref()),
            uri: optional_str_view(annotation.uri.as_deref()),
            rect,
            has_rect,
            quadpoint_count: annotation.quadpoint_rects.len(),
        }
    }
}

impl From<&FormField> for LiteParseFormField {
    fn from(field: &FormField) -> Self {
        let (object_number, has_object_number) = optional(field.object_number);
        let (control_count, has_control_count) = optional(field.control_count);
        let (control_index, has_control_index) = optional(field.control_index);
        let (checked, has_checked) = optional(field.checked);
        let (rect, has_rect) = optional_rect(field.rect.as_ref());
        Self {
            id: bytes_view(field.id.as_bytes()),
            field_type: bytes_view(field.field_type.as_bytes()),
            name: optional_str_view(field.name.as_deref()),
            alternate_name: optional_str_view(field.alternate_name.as_deref()),
            value: optional_str_view(field.value.as_deref()),
            export_value: optional_str_view(field.export_value.as_deref()),
            page: field.page,
            annotation_index: field.annotation_index,
            widget_index: field.widget_index,
            object_number,
            has_object_number,
            field_flags: field.field_flags,
            control_count,
            has_control_count,
            control_index,
            has_control_index,
            checked,
            has_checked,
            rect,
            has_rect,
            options_len: field.options.len(),
            selected_options_len: field.selected_options.len(),
        }
    }
}

impl LiteParsePageGeometry {
    /// Build from resolved PDF box edges (bottom-left origin, pre-`user_unit`),
    /// the page `/UserUnit`, and the clockwise quarter turns (`None` when
    /// PDFium could not report a value in `0..=3`). No core types involved.
    pub(crate) fn from_parts(
        box_left: f32,
        box_bottom: f32,
        box_right: f32,
        box_top: f32,
        user_unit: f32,
        rotation_quarter_turns: Option<u8>,
    ) -> Option<Self> {
        let edges = [box_left, box_bottom, box_right, box_top, user_unit];
        if !edges.iter().all(|value| value.is_finite()) || user_unit <= 0.0 {
            return None;
        }
        Some(Self {
            box_left,
            box_bottom,
            box_right,
            box_top,
            user_unit,
            rotation_quarter_turns: u32::from(rotation_quarter_turns.unwrap_or(0)),
            has_rotation: rotation_quarter_turns.is_some(),
        })
    }

    /// Resolve geometry directly from a live PDFium page. Mirrors the core
    /// `extract_single_page` view-box fallback so extract/raw-text/page-object
    /// geometries agree with parse without a core field.
    pub(crate) fn from_pdfium(page: &liteparse_pdfium::Page<'_, '_>) -> Option<Self> {
        let raw_w = page.width();
        let raw_h = page.height();
        let view_box = page.view_box().unwrap_or(liteparse_pdfium::RectF {
            left: 0.0,
            top: raw_h,
            right: raw_w,
            bottom: 0.0,
        });
        Self::from_parts(
            view_box.left,
            view_box.bottom,
            view_box.right,
            view_box.top,
            page.user_unit(),
            u8::try_from(page.rotation())
                .ok()
                .filter(|turns| *turns < 4),
        )
    }
}

impl From<&PageComplexityStats> for LiteParsePageComplexity {
    fn from(stats: &PageComplexityStats) -> Self {
        let (uncovered_vector_area, has_uncovered_vector_area) =
            optional(stats.uncovered_vector_area);
        let layout = stats.layout.as_ref();
        Self {
            page_number: stats.page_number,
            text_length: stats.text_length,
            text_coverage: stats.text_coverage,
            image_block_count: stats.image_block_count,
            image_coverage: stats.image_coverage,
            largest_image_coverage: stats.largest_image_coverage,
            has_substantial_images: stats.has_substantial_images,
            full_page_image: stats.full_page_image,
            is_garbled: stats.is_garbled,
            needs_ocr: stats.needs_ocr,
            page_area: stats.page_area,
            uncovered_vector_area,
            has_uncovered_vector_area,
            reasons_mask: reason_mask(&stats.reasons),
            has_layout: stats.layout.is_some(),
            layout_column_count: layout.map_or(0, |l| l.column_count),
            layout_ruled_table_count: layout.map_or(0, |l| l.ruled_table_count),
            layout_text_table_run_count: layout.map_or(0, |l| l.text_table_run_count),
            layout_figure_count: layout.map_or(0, |l| l.figure_count),
            layout_ruled_table_coverage: layout.map_or(0.0, |l| l.ruled_table_coverage),
            layout_figure_coverage: layout.map_or(0.0, |l| l.figure_coverage),
            layout_is_complex: layout.is_some_and(|l| l.is_complex),
            layout_reasons_mask: layout.map_or(0, |l| layout_reason_mask(&l.reasons)),
        }
    }
}

fn layout_reason_mask(reasons: &[LayoutComplexityReason]) -> u32 {
    reasons
        .iter()
        .map(|reason| match reason {
            LayoutComplexityReason::MultiColumn => LITEPARSE_LAYOUT_REASON_MULTI_COLUMN,
            LayoutComplexityReason::TableLikely => LITEPARSE_LAYOUT_REASON_TABLE_LIKELY,
            LayoutComplexityReason::DenseGraphics => LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS,
        })
        .fold(0, |mask, bit| mask | bit)
}

fn reason_mask(reasons: &[ComplexityReason]) -> u32 {
    reasons
        .iter()
        .map(|reason| match reason {
            ComplexityReason::Scanned => LITEPARSE_REASON_SCANNED,
            ComplexityReason::NoText => LITEPARSE_REASON_NO_TEXT,
            ComplexityReason::SparseText => LITEPARSE_REASON_SPARSE_TEXT,
            ComplexityReason::EmbeddedImages => LITEPARSE_REASON_EMBEDDED_IMAGES,
            ComplexityReason::Garbled => LITEPARSE_REASON_GARBLED,
            ComplexityReason::VectorText => LITEPARSE_REASON_VECTOR_TEXT,
            ComplexityReason::AnnotationText => LITEPARSE_REASON_ANNOTATION_TEXT,
        })
        .fold(0, |mask, bit| mask | bit)
}

pub(crate) fn views<'a, C, V: From<&'a C>>(items: &'a [C]) -> Vec<V> {
    items.iter().map(V::from).collect()
}

pub(crate) struct StructurePacked {
    pub(crate) nodes: Vec<LiteParseStructureNode>,
    pub(crate) attributes: Vec<LiteParseStructureAttribute>,
    pub(crate) annotations: Vec<LiteParseAnnotation>,
    pub(crate) marked_content_ids: Vec<i32>,
}

impl StructurePacked {
    const MAX_DEPTH: u32 = 128;

    pub(crate) fn pack(tree: &StructureTree) -> Self {
        let mut packed = Self {
            nodes: Vec::new(),
            attributes: Vec::new(),
            annotations: Vec::new(),
            marked_content_ids: Vec::new(),
        };
        for root in &tree.roots {
            packed.walk(root, -1, 0);
        }
        packed
    }

    fn walk(&mut self, element: &StructureTreeElement, parent_index: i32, depth: u32) {
        if depth > Self::MAX_DEPTH {
            return;
        }
        let node_index = self.nodes.len();
        let attribute_offset = self.attributes.len();
        self.attributes.extend(
            element
                .attributes
                .iter()
                .map(|(name, value)| structure_attribute(name, value)),
        );
        let annotation_offset = self.annotations.len();
        self.annotations
            .extend(element.annotations.iter().map(LiteParseAnnotation::from));
        let marked_content_id_offset = self.marked_content_ids.len();
        self.marked_content_ids
            .extend_from_slice(&element.marked_content_ids);
        self.nodes.push(LiteParseStructureNode {
            element_type: bytes_view(element.element_type.as_bytes()),
            id: optional_str_view(element.id.as_deref()),
            actual_text: optional_str_view(element.actual_text.as_deref()),
            alt_text: optional_str_view(element.alt_text.as_deref()),
            title: optional_str_view(element.title.as_deref()),
            parent_index,
            depth,
            marked_content_id_offset,
            marked_content_ids_len: element.marked_content_ids.len(),
            attribute_offset,
            attribute_count: element.attributes.len(),
            annotation_offset,
            annotation_count: element.annotations.len(),
        });
        for child in &element.children {
            self.walk(child, node_index as i32, depth + 1);
        }
    }
}

fn structure_attribute(name: &str, value: &StructureAttributeValue) -> LiteParseStructureAttribute {
    let (kind, bool_value, number_value, string_value) = match value {
        StructureAttributeValue::Boolean(value) => (
            LITEPARSE_STRUCTURE_ATTR_BOOL,
            *value,
            0.0,
            LiteParseByteView::default(),
        ),
        StructureAttributeValue::Number(value) => (
            LITEPARSE_STRUCTURE_ATTR_NUMBER,
            false,
            *value,
            LiteParseByteView::default(),
        ),
        StructureAttributeValue::String(value) => (
            LITEPARSE_STRUCTURE_ATTR_STRING,
            false,
            0.0,
            bytes_view(value.as_bytes()),
        ),
    };
    LiteParseStructureAttribute {
        name: bytes_view(name.as_bytes()),
        kind,
        bool_value,
        number_value,
        string_value,
    }
}

pub(crate) struct BlocksPacked {
    pub(crate) blocks: Vec<LiteParseLayoutBlock>,
    pub(crate) cells: Vec<LiteParseLayoutCell>,
    pub(crate) rows: Vec<LiteParseLayoutRow>,
    pub(crate) lines: Vec<LiteParseByteView>,
}

impl BlocksPacked {
    pub(crate) fn pack(blocks: &[LayoutBlock]) -> Self {
        let mut packed = Self {
            blocks: Vec::with_capacity(blocks.len()),
            cells: Vec::new(),
            rows: Vec::new(),
            lines: Vec::new(),
        };
        for block in blocks {
            packed.push(block);
        }
        packed
    }

    fn push(&mut self, block: &LayoutBlock) {
        let header_cell_offset = self.cells.len();
        self.cells
            .extend(block.header.iter().flatten().map(layout_cell));
        let header_cell_count = self.cells.len() - header_cell_offset;

        let first_row = self.rows.len();
        for row in block.rows.iter().flatten() {
            let cell_offset = self.cells.len();
            self.cells.extend(row.iter().map(layout_cell));
            self.rows.push(LiteParseLayoutRow {
                cell_offset,
                cell_count: row.len(),
            });
        }
        let row_count = self.rows.len() - first_row;

        let line_offset = self.lines.len();
        self.lines.extend(
            block
                .lines
                .iter()
                .flatten()
                .map(|line| bytes_view(line.as_bytes())),
        );
        let line_count = self.lines.len() - line_offset;

        let (level, has_level) = optional(block.level);
        let (ordered, has_ordered) = optional(block.ordered);
        let (header_rows, has_header_rows) = optional(block.header_rows);
        let (bbox, has_bbox) = optional_rect(block.bbox.as_ref());
        self.blocks.push(LiteParseLayoutBlock {
            kind: bytes_view(block.kind.as_bytes()),
            text: optional_str_view(block.text.as_deref()),
            level,
            has_level,
            bold: block.bold,
            italic: block.italic,
            ordered,
            has_ordered,
            marker: optional_str_view(block.marker.as_deref()),
            lang: optional_str_view(block.lang.as_deref()),
            line_offset,
            line_count,
            header_cell_offset,
            header_cell_count,
            first_row,
            row_count,
            header_rows,
            id: optional_str_view(block.id.as_deref()),
            format: optional_str_view(block.format.as_deref()),
            bbox,
            has_bbox,
            has_header_rows,
        });
    }
}

fn layout_cell(cell: &LayoutCell) -> LiteParseLayoutCell {
    let (bbox, has_bbox) = optional_rect(cell.bbox.as_ref());
    LiteParseLayoutCell {
        text: bytes_view(cell.text.as_bytes()),
        bbox,
        colspan: cell.colspan.unwrap_or(0),
        rowspan: cell.rowspan.unwrap_or(0),
        has_bbox,
    }
}

pub(crate) struct VectorsPacked {
    pub(crate) shapes: Vec<LiteParseVectorShape>,
    pub(crate) lines: Vec<LiteParseVectorLine>,
}

impl VectorsPacked {
    pub(crate) fn pack(graphics: &VectorGraphics) -> Self {
        Self {
            shapes: graphics
                .shapes
                .iter()
                .map(|shape| LiteParseVectorShape {
                    bbox: LiteParseRect::from(&shape.bbox),
                    stroke: shape.stroke,
                    stroke_color: optional_str_view(shape.stroke_color.as_deref()),
                    fill: shape.fill,
                    fill_color: optional_str_view(shape.fill_color.as_deref()),
                    has_curve: shape.has_curve,
                })
                .collect(),
            lines: graphics
                .lines
                .iter()
                .map(|line| {
                    let (stroke_width, has_stroke_width) = optional(line.stroke_width);
                    LiteParseVectorLine {
                        x1: line.x1,
                        y1: line.y1,
                        x2: line.x2,
                        y2: line.y2,
                        stroke: line.stroke,
                        stroke_width,
                        has_stroke_width,
                        stroke_color: optional_str_view(line.stroke_color.as_deref()),
                        fill: line.fill,
                        fill_color: optional_str_view(line.fill_color.as_deref()),
                    }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    fn leaf(items: Vec<usize>) -> Region {
        Region {
            bbox: rect(),
            kind: RegionKind::Leaf {
                item_indices: items,
            },
        }
    }

    fn split(children: Vec<Region>) -> Region {
        Region {
            bbox: rect(),
            kind: RegionKind::Split {
                axis: CutAxis::Vertical,
                children,
            },
        }
    }

    #[test]
    fn nested_region_children_are_contiguous() {
        // root -> [a -> [a1, a2], b]
        let tree = split(vec![
            split(vec![leaf(vec![0]), leaf(vec![1])]),
            leaf(vec![2]),
        ]);
        let mut packed = ProjectedLayoutPacked::pack(&[], false);
        flatten_region(&tree, LITEPARSE_PROJECTED_REGION_NO_PARENT, &mut packed);

        let children = |node: usize| {
            let region = &packed.regions[node];
            let start = region.child_offset as usize;
            packed.region_children[start..start + region.child_count as usize].to_vec()
        };
        assert_eq!(children(0), [1, 4]);
        assert_eq!(children(1), [2, 3]);
        assert!(children(2).is_empty());
        for (index, region) in packed.regions.iter().enumerate().skip(1) {
            assert!(children(region.parent_index as usize).contains(&(index as u64)));
        }
    }
}
