use liteparse::ScreenshotResult;
use liteparse::layout::{LayoutBlock, LayoutCell};
use liteparse::ocr_merge::{ComplexityReason, LayoutComplexityReason, PageComplexityStats};
use liteparse::types::{
    DocumentAnnotation, DocumentMetadata, ExtractedImage, FormField, OutlineTarget, PageError,
    PageGeometry, Rect, ScreenshotRect, StructureAttributeValue, StructureTree,
    StructureTreeElement, TextItem, VectorGraphics, WordBox, XfaPacket,
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
#[derive(Default)]
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
#[derive(Default)]
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
/// `LITEPARSE_FLAG_EXTRACT_TEXT_METADATA`.
#[repr(C)]
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
    /// PDF object number, usable to join structure-tree references.
    pub object_number: i32,
    pub has_rect: bool,
    pub has_object_number: bool,
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
pub struct LiteParseLayoutCell {
    pub text: LiteParseByteView,
    pub bbox: LiteParseRect,
    pub has_bbox: bool,
}

/// Row range into `liteparse_result_block_cells`.
#[repr(C)]
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
#[repr(C)]
pub struct LiteParseLayoutBlock {
    /// One of `heading`, `paragraph`, `list_item`, `code`, `table`,
    /// `grid_fallback`, `rule`, `figure`.
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
        let (object_number, has_object_number) = optional(annotation.object_number);
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
            object_number,
            has_object_number,
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
    pub(crate) fn from_core(geometry: &PageGeometry) -> Option<Self> {
        let edges = [
            geometry.box_left,
            geometry.box_bottom,
            geometry.box_right,
            geometry.box_top,
            geometry.user_unit,
        ];
        if !edges.iter().all(|value| value.is_finite()) || geometry.user_unit <= 0.0 {
            return None;
        }
        Some(Self {
            box_left: geometry.box_left,
            box_bottom: geometry.box_bottom,
            box_right: geometry.box_right,
            box_top: geometry.box_top,
            user_unit: geometry.user_unit,
            rotation_quarter_turns: u32::from(geometry.rotation_quarter_turns.unwrap_or(0)),
            has_rotation: geometry.rotation_quarter_turns.is_some(),
        })
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
            id: optional_str_view(block.id.as_deref()),
            format: optional_str_view(block.format.as_deref()),
            bbox,
            has_bbox,
        });
    }
}

fn layout_cell(cell: &LayoutCell) -> LiteParseLayoutCell {
    let (bbox, has_bbox) = optional_rect(cell.bbox.as_ref());
    LiteParseLayoutCell {
        text: bytes_view(cell.text.as_bytes()),
        bbox,
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
