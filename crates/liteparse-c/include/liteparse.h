#ifndef LITEPARSE_H
#define LITEPARSE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/**
 * Values for `LiteParseConfig.output_format`.
 */
#define LITEPARSE_OUTPUT_FORMAT_JSON 0

#define LITEPARSE_OUTPUT_FORMAT_TEXT 1

#define LITEPARSE_OUTPUT_FORMAT_MARKDOWN 2

/**
 * Values for `LiteParseConfig.image_mode`.
 */
#define LITEPARSE_IMAGE_MODE_OFF 0

#define LITEPARSE_IMAGE_MODE_PLACEHOLDER 1

#define LITEPARSE_IMAGE_MODE_EMBED 2

/**
 * Keep the native default in fields where zero is not meaningful.
 */
#define LITEPARSE_UNSET UINT32_MAX

/**
 * Bits are ABI-stable: append new flags without renumbering existing ones.
 */
#define LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR (1ull << 0)

#define LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS (1ull << 1)

#define LITEPARSE_FLAG_EMIT_WORD_BOXES (1ull << 2)

#define LITEPARSE_FLAG_EXTRACT_ANNOTATIONS (1ull << 3)

#define LITEPARSE_FLAG_EXTRACT_BLOCKS (1ull << 4)

#define LITEPARSE_FLAG_EXTRACT_CONTENT_BOUNDS (1ull << 5)

#define LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA (1ull << 6)

#define LITEPARSE_FLAG_EXTRACT_FORM_FIELDS (1ull << 7)

#define LITEPARSE_FLAG_EXTRACT_IMAGES (1ull << 8)

#define LITEPARSE_FLAG_EXTRACT_LINKS (1ull << 9)

#define LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE (1ull << 10)

#define LITEPARSE_FLAG_EXTRACT_TEXT_METADATA (1ull << 11)

#define LITEPARSE_FLAG_EXTRACT_VECTOR_GRAPHICS (1ull << 12)

#define LITEPARSE_FLAG_EXTRACT_XFA_PACKETS (1ull << 13)

#define LITEPARSE_FLAG_INCLUDE_COMPLEXITY (1ull << 14)

#define LITEPARSE_FLAG_KEEP_HEADERS_FOOTERS (1ull << 15)

#define LITEPARSE_FLAG_OCR_ENABLED (1ull << 16)

#define LITEPARSE_FLAG_OCR_FAILURE_FATAL (1ull << 17)

#define LITEPARSE_FLAG_PRESERVE_VERY_SMALL_TEXT (1ull << 18)

#define LITEPARSE_FLAG_QUIET (1ull << 19)

#define LITEPARSE_FLAG_RENDER_FORM_FIELDS (1ull << 20)

#define LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT (1ull << 21)

#define LITEPARSE_FLAG_EXTRACT_SCREENSHOTS (1ull << 22)

/**
 * Pixel formats passed to a `LiteParseOcrRecognizeFn`.
 */
#define LITEPARSE_OCR_PIXEL_FORMAT_RGB 0

#define LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE 1

/**
 * Values for `LiteParseFormTypeValue.value`.
 */
#define LITEPARSE_FORM_TYPE_NONE 0

#define LITEPARSE_FORM_TYPE_ACRO_FORM 1

#define LITEPARSE_FORM_TYPE_XFA_FULL 2

#define LITEPARSE_FORM_TYPE_XFA_FOREGROUND 3

/**
 * Reason bits reported in `LiteParsePageComplexity.reasons_mask`.
 */
#define LITEPARSE_REASON_SCANNED (1 << 0)

#define LITEPARSE_REASON_NO_TEXT (1 << 1)

#define LITEPARSE_REASON_SPARSE_TEXT (1 << 2)

#define LITEPARSE_REASON_EMBEDDED_IMAGES (1 << 3)

#define LITEPARSE_REASON_GARBLED (1 << 4)

#define LITEPARSE_REASON_VECTOR_TEXT (1 << 5)

#define LITEPARSE_REASON_ANNOTATION_TEXT (1 << 6)

/**
 * Reason bits reported in `LiteParsePageComplexity.layout_reasons_mask`.
 */
#define LITEPARSE_LAYOUT_REASON_MULTI_COLUMN (1 << 0)

#define LITEPARSE_LAYOUT_REASON_TABLE_LIKELY (1 << 1)

#define LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS (1 << 2)

/**
 * Values for `LiteParseStructureAttribute.kind`.
 */
#define LITEPARSE_STRUCTURE_ATTR_BOOL 0

#define LITEPARSE_STRUCTURE_ATTR_NUMBER 1

#define LITEPARSE_STRUCTURE_ATTR_STRING 2

typedef struct LiteParseComplexity LiteParseComplexity;

/**
 * A document opened once for many operations. Operations may run
 * concurrently on one handle; destruction must wait for them.
 */
typedef struct LiteParseDocument LiteParseDocument;

/**
 * Valid only during the callback that receives it.
 */
typedef struct LiteParseOcrSink LiteParseOcrSink;

/**
 * An owned parser. Safe to share between threads; destruction must wait for
 * in-flight operations.
 */
typedef struct LiteParseParser LiteParseParser;

typedef struct LiteParseResult LiteParseResult;

typedef struct LiteParseScreenshots LiteParseScreenshots;

typedef struct LiteParseSearchMatches LiteParseSearchMatches;

/**
 * Per-page OCR and layout complexity signals.
 */
typedef struct {
  size_t page_number;
  size_t text_length;
  /**
   * Fraction of page area covered by native text.
   */
  float text_coverage;
  size_t image_block_count;
  /**
   * Summed image-bbox coverage, clamped to 1.0.
   */
  float image_coverage;
  /**
   * Coverage of the largest counted image.
   */
  float largest_image_coverage;
  float page_area;
  float uncovered_vector_area;
  /**
   * `LITEPARSE_REASON_*` bits explaining `needs_ocr`.
   */
  uint32_t reasons_mask;
  /**
   * Side-by-side columns; 1 means a single column.
   */
  size_t layout_column_count;
  size_t layout_ruled_table_count;
  /**
   * Borderless table runs found by track alignment. Overlaps
   * `layout_ruled_table_count`; the two must not be summed.
   */
  size_t layout_text_table_run_count;
  size_t layout_figure_count;
  /**
   * Combined validated ruled-table area over page area, clamped to 1.0.
   */
  float layout_ruled_table_coverage;
  /**
   * Combined figure area over page area, clamped to 1.0.
   */
  float layout_figure_coverage;
  /**
   * `LITEPARSE_LAYOUT_REASON_*` bits explaining `layout_is_complex`.
   */
  uint32_t layout_reasons_mask;
  bool has_substantial_images;
  bool full_page_image;
  bool is_garbled;
  bool needs_ocr;
  bool has_uncovered_vector_area;
  bool has_layout;
  bool layout_is_complex;
} LiteParsePageComplexity;

/**
 * Borrowed, non-NUL-terminated bytes valid while the owner lives.
 */
typedef struct {
  const uint8_t *ptr;
  size_t len;
} LiteParseByteView;

/**
 * One HTTP OCR header, copied by `liteparse_parser_new`.
 */
typedef struct {
  LiteParseByteView name;
  LiteParseByteView value;
} LiteParseHeader;

/**
 * Start with `liteparse_config_default`; parser creation copies all views.
 */
typedef struct {
  /**
   * Must equal `sizeof(LiteParseConfig)`.
   */
  size_t size_of_config;
  uint64_t bools_set;
  uint64_t bools_values;
  /**
   * Zero keeps the native default (1000).
   */
  size_t max_pages;
  /**
   * Zero keeps the native default.
   */
  size_t num_workers;
  /**
   * Zero keeps the native default; nonzero values must be finite and > 0.
   */
  float dpi;
  /**
   * `LITEPARSE_UNSET` keeps the native default.
   */
  uint32_t output_format;
  /**
   * `LITEPARSE_UNSET` keeps the native default.
   */
  uint32_t image_mode;
  /**
   * Normalized fractions ordered top, right, bottom, left. When
   * `has_crop_box` is set every value must lie in `[0, 1]` with
   * `top + bottom < 1` and `left + right < 1`.
   */
  float crop_box[4];
  bool has_crop_box;
  LiteParseByteView ocr_language;
  LiteParseByteView ocr_server_url;
  LiteParseByteView tessdata_path;
  LiteParseByteView password;
  LiteParseByteView image_output_dir;
  const LiteParseHeader *ocr_server_headers;
  size_t ocr_server_headers_len;
  const uint64_t *ocr_hedge_delays_ms;
  size_t ocr_hedge_delays_ms_len;
  /**
   * Optional `%02x%02x.msgpack` glyph-database directory. An explicit path
   * overrides `LITEPARSE_FONT_DB_DIR`.
   */
  LiteParseByteView font_db_dir;
} LiteParseConfig;

/**
 * Fixed-width status code returned by fallible API functions.
 */
typedef uint32_t LiteParseStatus;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseDocument *handle;
} LiteParseDocumentNew;

/**
 * One outline entry (bookmark). `page_index` is zero-based and `-1` when the
 * destination is not a page; `y_pdf` is PDF user space.
 */
typedef struct {
  uint8_t level;
  LiteParseByteView title;
  int32_t page_index;
  float y_pdf;
  bool has_y_pdf;
} LiteParseOutlineEntry;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseResult *handle;
} LiteParseResultNew;

/**
 * Status and handle returned by screenshot renders. The handle is null
 * unless the status is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseScreenshots *handle;
} LiteParseScreenshotsNew;

/**
 * Page region in top-left-origin viewport points. Must fit within the page.
 */
typedef struct {
  float x;
  float y;
  float width;
  float height;
} LiteParseRenderRegion;

/**
 * Status and handle returned by complexity analysis. The handle is null
 * unless the status is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseComplexity *handle;
} LiteParseComplexityNew;

typedef struct {
  size_t text_offset;
  size_t text_length;
  /**
   * Box edges in raster pixels: left, top, right, bottom.
   */
  float x1;
  float y1;
  float x2;
  float y2;
  float confidence;
  /**
   * Four x/y corners in reading order when `has_polygon` is set.
   */
  float polygon[8];
  bool has_polygon;
} LiteParseOcrWordIn;

/**
 * Status and handle returned by `liteparse_parser_new`. The handle is null
 * unless the status is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseParser *handle;
} LiteParseParserNew;

/**
 * Return nonzero to fail recognition. Calls may be concurrent.
 */
typedef uint32_t (*LiteParseOcrRecognizeFn)(void *user_data,
                                            const uint8_t *pixels,
                                            size_t pixels_len,
                                            uint32_t width,
                                            uint32_t height,
                                            uint32_t pixel_format,
                                            const char *language,
                                            float dpi,
                                            LiteParseOcrSink *sink);

/**
 * Page viewport size in 72-DPI points.
 */
typedef struct {
  float width;
  float height;
} LiteParsePageSize;

/**
 * Visible PDF box in bottom-left-origin page space, before `user_unit`.
 */
typedef struct {
  float box_left;
  float box_bottom;
  float box_right;
  float box_top;
  /**
   * Page `/UserUnit`, normally 1.0.
   */
  float user_unit;
  /**
   * Clockwise quarter turns, `0..=3`. Zero is an ordinary rotation, so
   * absence is `has_rotation`, never this field.
   */
  uint32_t rotation_quarter_turns;
  bool has_rotation;
} LiteParsePageGeometry;

/**
 * Optional page geometry; present values are finite with positive `user_unit`.
 */
typedef struct {
  LiteParsePageGeometry geometry;
  bool present;
} LiteParsePageGeometryValue;

typedef struct {
  /**
   * One of the `LITEPARSE_FORM_TYPE_*` values.
   */
  int32_t value;
  bool present;
} LiteParseFormTypeValue;

/**
 * Optional scalars use `has_*`; absent strings are null views.
 */
typedef struct {
  /**
   * Authored `/Info` values read at document open.
   */
  LiteParseByteView title;
  LiteParseByteView author;
  LiteParseByteView subject;
  LiteParseByteView keywords;
  LiteParseByteView trapped;
  LiteParseByteView creation_date;
  LiteParseByteView mod_date;
  int32_t file_version;
  int32_t security_handler_revision;
  uint64_t permissions;
  uint32_t eof_section_count;
  uint32_t startxref_count;
  uint64_t raw_file_size;
  LiteParseByteView xmp;
  uint32_t signature_count;
  bool has_file_version;
  bool is_encrypted;
  bool has_is_encrypted;
  bool has_security_handler_revision;
  bool has_permissions;
  bool has_eof_section_count;
  bool has_startxref_count;
  bool trailer_id_pair_differs;
  bool has_trailer_id_pair_differs;
  bool has_raw_file_size;
  bool xmp_truncated;
  bool has_xmp_truncated;
  bool has_signature_count;
  bool signature_byte_range_reaches_eof;
  bool has_signature_byte_range_reaches_eof;
} LiteParseDocumentMeta;

typedef struct {
  LiteParseDocumentMeta meta;
  bool present;
} LiteParseDocumentMetaValue;

/**
 * A rectangle in top-left-origin 72-DPI viewport space.
 */
typedef struct {
  float x;
  float y;
  float width;
  float height;
} LiteParseRect;

typedef struct {
  LiteParseRect rect;
  bool present;
} LiteParseRectValue;

typedef struct {
  LiteParsePageComplexity stats;
  bool present;
} LiteParsePageComplexityValue;

/**
 * Borrows data from its result handle. Rich metadata requires
 * `LITEPARSE_FLAG_EXTRACT_TEXT_METADATA`.
 */
typedef struct {
  LiteParseByteView text;
  LiteParseByteView font_name;
  LiteParseByteView link;
  float x;
  float y;
  float width;
  float height;
  float rotation;
  float font_size;
  float confidence;
  int32_t font_flags;
  float font_height;
  float font_ascent;
  float font_descent;
  int32_t font_weight;
  float text_width;
  int32_t mcid;
  /**
   * ARGB hex strings such as "ff000000".
   */
  LiteParseByteView fill_color;
  LiteParseByteView stroke_color;
  /**
   * Borrowed raw content-stream character codes.
   */
  const uint32_t *char_codes;
  size_t char_codes_len;
  bool has_font_size;
  bool has_confidence;
  bool strike;
  bool has_unicode_map_error;
  bool has_font_flags;
  bool has_font_height;
  bool has_font_ascent;
  bool has_font_descent;
  bool has_font_weight;
  bool has_text_width;
  bool font_is_buggy;
  bool has_font_is_buggy;
  bool has_mcid;
  bool trailing_space_generated;
  bool has_trailing_space_generated;
} LiteParseTextItem;

/**
 * Word box in top-left-origin 72-DPI page space.
 */
typedef struct {
  LiteParseByteView text;
  float x;
  float y;
  float width;
  float height;
} LiteParseWordBox;

/**
 * Extracted image borrowing data from its result handle.
 */
typedef struct {
  LiteParseByteView id;
  LiteParseByteView name;
  LiteParseByteView path;
  LiteParseByteView format;
  LiteParseByteView duplicate_of;
  uint32_t page;
  uint32_t width;
  uint32_t height;
  float rotation;
  LiteParseRect bbox;
  LiteParseByteView bytes;
} LiteParseImage;

/**
 * Screenshot borrowing PNG data from its owning handle.
 */
typedef struct {
  uint32_t page_number;
  uint32_t width;
  uint32_t height;
  LiteParseByteView png;
  /**
   * Resolution the page was actually rendered at: the requested DPI unless
   * the renderer lowered it to keep the long edge under 30,000 pixels. For
   * a region render it still describes the page, so viewport geometry
   * scales by it either way.
   */
  float effective_dpi;
  bool is_solid_fill;
} LiteParseScreenshot;

/**
 * A solid rectangle or line in top-left-origin 72-DPI viewport space. For a
 * region render the coordinates are relative to the region's origin.
 */
typedef struct {
  float x;
  float y;
  float width;
  float height;
  LiteParseByteView color;
  bool is_line;
} LiteParseScreenshotRect;

typedef struct {
  uint32_t page_number;
  LiteParseByteView message;
} LiteParsePageError;

/**
 * XFA packet borrowing data from its result handle.
 */
typedef struct {
  uint32_t index;
  LiteParseByteView name;
  uint32_t content_length;
  /**
   * Packet content, lossily decoded UTF-8; null view when unreadable.
   */
  LiteParseByteView content;
} LiteParseXfaPacket;

/**
 * Page annotation borrowing strings from its result handle.
 */
typedef struct {
  LiteParseByteView subtype;
  LiteParseByteView contents;
  LiteParseByteView created;
  LiteParseByteView modified;
  LiteParseByteView title;
  LiteParseByteView uri;
  LiteParseRect rect;
  /**
   * Number of quadpoint rectangles; fetch them with
   * `liteparse_result_annotation_quadpoints`.
   */
  size_t quadpoint_count;
  /**
   * PDF object number, usable to join structure-tree references.
   */
  int32_t object_number;
  bool has_rect;
  bool has_object_number;
} LiteParseAnnotation;

/**
 * AcroForm widget borrowing strings from its result handle.
 */
typedef struct {
  LiteParseByteView id;
  LiteParseByteView field_type;
  LiteParseByteView name;
  LiteParseByteView alternate_name;
  LiteParseByteView value;
  LiteParseByteView export_value;
  uint32_t page;
  int32_t annotation_index;
  int32_t widget_index;
  int32_t object_number;
  int32_t field_flags;
  int32_t control_count;
  int32_t control_index;
  LiteParseRect rect;
  size_t options_len;
  size_t selected_options_len;
  bool has_object_number;
  bool has_control_count;
  bool has_control_index;
  bool checked;
  bool has_checked;
  bool has_rect;
} LiteParseFormField;

/**
 * One node of a page's structure tree, pre-flattened in pre-order
 * (parent before children). `parent_index` is `-1` for roots. Attribute and
 * annotation ranges index into the flattened arrays returned by
 * `liteparse_result_structure_attributes` / `_annotations`; marked-content
 * ids point into storage owned by the result handle.
 */
typedef struct {
  LiteParseByteView element_type;
  LiteParseByteView id;
  LiteParseByteView actual_text;
  LiteParseByteView alt_text;
  LiteParseByteView title;
  /**
   * Index of the parent node in the same slice, or -1 for a root.
   */
  int32_t parent_index;
  /**
   * Nesting depth, 0 for roots.
   */
  uint32_t depth;
  /**
   * Range into the flattened id array from
   * `liteparse_result_structure_marked_content_ids`.
   */
  size_t marked_content_id_offset;
  size_t marked_content_ids_len;
  size_t attribute_offset;
  size_t attribute_count;
  size_t annotation_offset;
  size_t annotation_count;
} LiteParseStructureNode;

typedef struct {
  LiteParseByteView name;
  /**
   * One of the `LITEPARSE_STRUCTURE_ATTR_*` values.
   */
  uint32_t kind;
  float number_value;
  LiteParseByteView string_value;
  bool bool_value;
} LiteParseStructureAttribute;

/**
 * One classified layout block. Variant-specific fields that do not apply to
 * `kind` carry their absent encoding (`has_*` false, null views).
 *
 * Table geometry: `header_cell_offset/count` indexes the page's packed cell
 * array; `first_row/row_count` indexes the packed row array from
 * `liteparse_result_block_rows`. Verbatim source lines (`code`,
 * `grid_fallback`) live in the packed line array from
 * `liteparse_result_block_lines`, indexed by `line_offset/line_count`.
 */
typedef struct {
  /**
   * One of `heading`, `paragraph`, `list_item`, `code`, `table`,
   * `grid_fallback`, `rule`, `figure`.
   */
  LiteParseByteView kind;
  LiteParseByteView text;
  /**
   * Heading level (1-6), or list nesting depth.
   */
  uint8_t level;
  LiteParseByteView marker;
  LiteParseByteView lang;
  size_t line_offset;
  size_t line_count;
  size_t header_cell_offset;
  size_t header_cell_count;
  size_t first_row;
  size_t row_count;
  /**
   * Figure image id and encoded format.
   */
  LiteParseByteView id;
  LiteParseByteView format;
  LiteParseRect bbox;
  bool has_level;
  bool bold;
  bool italic;
  bool ordered;
  bool has_ordered;
  bool has_bbox;
} LiteParseLayoutBlock;

typedef struct {
  LiteParseByteView text;
  LiteParseRect bbox;
  bool has_bbox;
} LiteParseLayoutCell;

/**
 * Row range into `liteparse_result_block_cells`.
 */
typedef struct {
  size_t cell_offset;
  size_t cell_count;
} LiteParseLayoutRow;

typedef struct {
  LiteParseRect bbox;
  LiteParseByteView stroke_color;
  LiteParseByteView fill_color;
  bool stroke;
  bool fill;
  bool has_curve;
} LiteParseVectorShape;

typedef struct {
  float x1;
  float y1;
  float x2;
  float y2;
  float stroke_width;
  LiteParseByteView stroke_color;
  LiteParseByteView fill_color;
  bool stroke;
  bool has_stroke_width;
  bool fill;
} LiteParseVectorLine;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseSearchMatches *handle;
} LiteParseSearchMatchesNew;

/**
 * The operation succeeded.
 */
#define LITEPARSE_STATUS_OK 0

/**
 * A pointer, length, or input string was invalid.
 */
#define LITEPARSE_STATUS_INVALID_ARGUMENT 1

/**
 * A configuration value was invalid.
 */
#define LITEPARSE_STATUS_INVALID_CONFIG 2

/**
 * The document could not be opened, rendered, or parsed.
 */
#define LITEPARSE_STATUS_PARSE_ERROR 3

/**
 * A result could not be serialized to JSON.
 */
#define LITEPARSE_STATUS_SERIALIZATION_ERROR 4

/**
 * The asynchronous runtime could not be initialized.
 */
#define LITEPARSE_STATUS_RUNTIME_ERROR 5

/**
 * The document is encrypted and the configured password did not open it.
 */
#define LITEPARSE_STATUS_PASSWORD_REQUIRED 6

/**
 * A non-PDF source could not be converted: an unsupported extension, or a
 * missing external converter (LibreOffice).
 */
#define LITEPARSE_STATUS_CONVERSION_ERROR 7

/**
 * OCR was required and could not be performed.
 */
#define LITEPARSE_STATUS_OCR_ERROR 8

/**
 * The source could not be read from the filesystem.
 */
#define LITEPARSE_STATUS_IO_ERROR 9

/**
 * A Rust panic was caught before it crossed the C ABI boundary. Free any
 * returned handle and do not reuse it.
 */
#define LITEPARSE_STATUS_PANIC 255

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Destroy a complexity handle. Null is allowed.
 */
void liteparse_complexity_free(LiteParseComplexity *complexity);

/**
 * Borrow the analyzed pages.
 *
 * # Safety
 *
 * `complexity` must be live and `out_len` writable.
 */
const LiteParsePageComplexity *liteparse_complexity_slice(const LiteParseComplexity *complexity,
                                                          size_t *out_len);

/**
 * Borrow the cached JSON report, or an empty view on failure.
 *
 * # Safety
 *
 * `complexity` must be live.
 */
LiteParseByteView liteparse_complexity_json(const LiteParseComplexity *complexity);

LiteParseConfig liteparse_config_default(void);

/**
 * Open a path, converting non-PDF input once for the document's lifetime.
 *
 * # Safety
 *
 * `parser` must be live and `path` readable UTF-8.
 */
LiteParseDocumentNew liteparse_document_open_path(const LiteParseParser *parser,
                                                  LiteParseByteView path);

/**
 * Open and copy in-memory input. Prefer paths for large documents.
 *
 * # Safety
 *
 * `parser` must be live; `data` must be readable, or null with zero length.
 */
LiteParseDocumentNew liteparse_document_open_bytes(const LiteParseParser *parser,
                                                   const uint8_t *data,
                                                   size_t data_len);

/**
 * Destroy a document handle. Null is allowed.
 */
void liteparse_document_free(LiteParseDocument *document);

/**
 * Return the source page count recorded at open.
 *
 * # Safety
 *
 * `document` must be live.
 */
uint32_t liteparse_document_total_pages(const LiteParseDocument *document);

/**
 * Return whether the source was converted to PDF.
 *
 * # Safety
 *
 * `document` must be live.
 */
bool liteparse_document_is_converted(const LiteParseDocument *document);

/**
 * Borrow the document outline.
 *
 * # Safety
 *
 * `document` must be live and `out_len` writable.
 */
const LiteParseOutlineEntry *liteparse_document_outline(const LiteParseDocument *document,
                                                        size_t *out_len);

/**
 * Parse sorted, unique, 1-based pages. Null with zero length selects all.
 *
 * # Safety
 *
 * `document` must be live and `pages` readable, or null with zero length.
 */
LiteParseResultNew liteparse_document_parse(const LiteParseDocument *document,
                                            const uint32_t *pages,
                                            size_t pages_len);

/**
 * Render selected pages to PNG. Zero DPI uses the configured value; region
 * rectangles are clipped and made region-relative.
 *
 * # Safety
 *
 * `document` must be live; non-null inputs must be readable.
 */
LiteParseScreenshotsNew liteparse_document_screenshot(const LiteParseDocument *document,
                                                      const uint32_t *pages,
                                                      size_t pages_len,
                                                      float dpi_override,
                                                      const LiteParseRenderRegion *region);

/**
 * Compute complexity for selected pages; null with zero length selects all.
 *
 * # Safety
 *
 * `document` must be live and `pages` readable, or null with zero length.
 */
LiteParseComplexityNew liteparse_document_complexity(const LiteParseDocument *document,
                                                     const uint32_t *pages,
                                                     size_t pages_len);

/**
 * Append one OCR word. `polygon_corners` points to eight floats when present.
 *
 * # Safety
 *
 * `sink` must belong to the current callback and `text` must be readable UTF-8.
 */
LiteParseStatus liteparse_ocr_sink_add(LiteParseOcrSink *sink,
                                       LiteParseByteView text,
                                       float x1,
                                       float y1,
                                       float x2,
                                       float y2,
                                       float confidence,
                                       const float *polygon_corners);

/**
 * Append OCR words atomically; invalid input appends nothing.
 *
 * # Safety
 *
 * `sink` must belong to the current callback; input arrays must be readable.
 */
LiteParseStatus liteparse_ocr_sink_add_batch(LiteParseOcrSink *sink,
                                             const uint8_t *blob,
                                             size_t blob_len,
                                             const LiteParseOcrWordIn *words,
                                             size_t count);

/**
 * Set the callback's failure message.
 *
 * # Safety
 *
 * `sink` must belong to the current callback and `message` must be readable.
 */
LiteParseStatus liteparse_ocr_sink_set_error(LiteParseOcrSink *sink, LiteParseByteView message);

/**
 * Return the static, NUL-terminated binding version.
 */
const char *liteparse_version(void);

/**
 * Create a parser and copy its configuration.
 *
 * # Safety
 *
 * `config` and its views must be readable for the call.
 */
LiteParseParserNew liteparse_parser_new(const LiteParseConfig *config);

/**
 * Register or clear an OCR callback. Open documents retain their callback.
 *
 * # Safety
 *
 * The callback and `user_data` must remain valid and thread-safe while the
 * parser or any document opened from it lives. `name` must be readable.
 */
LiteParseStatus liteparse_parser_set_ocr_callback(const LiteParseParser *parser,
                                                  LiteParseOcrRecognizeFn recognize,
                                                  void *user_data,
                                                  LiteParseByteView name,
                                                  bool prefers_grayscale);

/**
 * Destroy a parser handle. Null is a no-op.
 */
void liteparse_parser_free(LiteParseParser *parser);

/**
 * Destroy a result handle. Null is allowed.
 */
void liteparse_result_free(LiteParseResult *result);

/**
 * Borrow the cached pretty JSON result.
 *
 * # Safety
 *
 * `result` must be live and `out` writable.
 */
LiteParseStatus liteparse_result_to_json(const LiteParseResult *result, LiteParseByteView *out);

/**
 * Return the source document page count.
 */
uint32_t liteparse_result_total_pages(const LiteParseResult *result);

/**
 * Return the number of parsed pages.
 */
size_t liteparse_result_page_count(const LiteParseResult *result);

/**
 * Return a page's 1-based source page number.
 */
uint32_t liteparse_result_page_number(const LiteParseResult *result, size_t page_index);

/**
 * Borrow full-document plain text or Markdown, according to the output format.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseByteView liteparse_result_text(const LiteParseResult *result);

/**
 * Borrow one page's plain UTF-8 text.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseByteView liteparse_result_page_text(const LiteParseResult *result, size_t page_index);

/**
 * Borrow one page's Markdown; empty unless Markdown output was requested.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseByteView liteparse_result_page_markdown(const LiteParseResult *result, size_t page_index);

/**
 * Borrow the document's optional `/Info` Creator value. Empty when absent.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseByteView liteparse_result_creator(const LiteParseResult *result);

/**
 * Borrow the document's optional `/Info` Producer value. Empty when absent.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseByteView liteparse_result_producer(const LiteParseResult *result);

/**
 * Return one page's viewport dimensions in 72-DPI points.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParsePageSize liteparse_result_page_size(const LiteParseResult *result, size_t page_index);

/**
 * Return the resolved PDF box, user unit, and rotation for one page.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParsePageGeometryValue liteparse_result_page_geometry(const LiteParseResult *result,
                                                          size_t page_index);

/**
 * Return the count of image extraction failures.
 *
 * # Safety
 *
 * `result` must be live.
 */
uint32_t liteparse_result_image_error_count(const LiteParseResult *result);

/**
 * Return the optional document form type.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseFormTypeValue liteparse_result_form_type(const LiteParseResult *result);

/**
 * Return document metadata when extraction was enabled.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseDocumentMetaValue liteparse_result_doc_meta(const LiteParseResult *result);

/**
 * Return one page's union content bounds by value.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParseRectValue liteparse_result_page_content_bounds(const LiteParseResult *result,
                                                        size_t page_index);

/**
 * Return complexity when it was included during parsing.
 *
 * # Safety
 *
 * `result` must be live.
 */
LiteParsePageComplexityValue liteparse_result_page_complexity(const LiteParseResult *result,
                                                              size_t page_index);

/**
 * Borrow one page's text items.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseTextItem *liteparse_result_text_items(const LiteParseResult *result,
                                                     size_t page_index,
                                                     size_t *out_len);

/**
 * Borrow one text item's word boxes.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseWordBox *liteparse_result_word_boxes(const LiteParseResult *result,
                                                    size_t page_index,
                                                    size_t item_index,
                                                    size_t *out_len);

/**
 * Borrow all extracted images.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseImage *liteparse_result_images(const LiteParseResult *result, size_t *out_len);

/**
 * Borrow screenshots produced during parsing.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseScreenshot *liteparse_result_screenshots(const LiteParseResult *result,
                                                        size_t *out_len);

/**
 * Borrow one screenshot's detected rectangles.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseScreenshotRect *liteparse_result_screenshot_rects(const LiteParseResult *result,
                                                                 size_t index,
                                                                 size_t *out_len);

/**
 * Borrow the document outline.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseOutlineEntry *liteparse_result_outline(const LiteParseResult *result,
                                                      size_t *out_len);

/**
 * Borrow all tolerated page errors.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParsePageError *liteparse_result_page_errors(const LiteParseResult *result,
                                                       size_t *out_len);

/**
 * Borrow extracted XFA packets.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseXfaPacket *liteparse_result_xfa_packets(const LiteParseResult *result,
                                                       size_t *out_len);

/**
 * Borrow one page's annotations.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseAnnotation *liteparse_result_annotations(const LiteParseResult *result,
                                                        size_t page_index,
                                                        size_t *out_len);

/**
 * Borrow one annotation's quadpoint rectangles.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseRect *liteparse_result_annotation_quadpoints(const LiteParseResult *result,
                                                            size_t page_index,
                                                            size_t annotation_index,
                                                            size_t *out_len);

/**
 * Borrow one page's AcroForm widgets.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseFormField *liteparse_result_form_fields(const LiteParseResult *result,
                                                       size_t page_index,
                                                       size_t *out_len);

/**
 * Borrow one widget's option strings.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseByteView *liteparse_result_form_field_options(const LiteParseResult *result,
                                                             size_t page_index,
                                                             size_t field_index,
                                                             size_t *out_len);

/**
 * Borrow one widget's selected option strings.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseByteView *liteparse_result_form_field_selected_options(const LiteParseResult *result,
                                                                      size_t page_index,
                                                                      size_t field_index,
                                                                      size_t *out_len);

/**
 * Borrow one page's pre-order structure-tree nodes.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseStructureNode *liteparse_result_structure_nodes(const LiteParseResult *result,
                                                               size_t page_index,
                                                               size_t *out_len);

/**
 * Borrow one page's flattened structure attributes.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseStructureAttribute *liteparse_result_structure_attributes(const LiteParseResult *result,
                                                                         size_t page_index,
                                                                         size_t *out_len);

/**
 * Borrow one page's flattened structure-node annotations.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseAnnotation *liteparse_result_structure_annotations(const LiteParseResult *result,
                                                                  size_t page_index,
                                                                  size_t *out_len);

/**
 * Borrow one page's flattened structure-node marked-content ids.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const int32_t *liteparse_result_structure_marked_content_ids(const LiteParseResult *result,
                                                             size_t page_index,
                                                             size_t *out_len);

/**
 * Borrow one page's classified layout blocks.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseLayoutBlock *liteparse_result_blocks(const LiteParseResult *result,
                                                    size_t page_index,
                                                    size_t *out_len);

/**
 * Borrow one page's packed layout table cells. Block header ranges and row
 * offsets index into this slice.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseLayoutCell *liteparse_result_block_cells(const LiteParseResult *result,
                                                        size_t page_index,
                                                        size_t *out_len);

/**
 * Borrow one page's packed layout table rows.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseLayoutRow *liteparse_result_block_rows(const LiteParseResult *result,
                                                      size_t page_index,
                                                      size_t *out_len);

/**
 * Borrow one page's verbatim layout source lines (`code`, `grid_fallback`).
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseByteView *liteparse_result_block_lines(const LiteParseResult *result,
                                                      size_t page_index,
                                                      size_t *out_len);

/**
 * Borrow one page's vector path objects.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseVectorShape *liteparse_result_vector_shapes(const LiteParseResult *result,
                                                           size_t page_index,
                                                           size_t *out_len);

/**
 * Borrow one page's merged vector segments.
 *
 * # Safety
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseVectorLine *liteparse_result_vector_lines(const LiteParseResult *result,
                                                         size_t page_index,
                                                         size_t *out_len);

/**
 * Search one page. Matches outlive the result handle.
 *
 * # Safety
 *
 * `result` must be live and `phrase` readable UTF-8.
 */
LiteParseSearchMatchesNew liteparse_result_search(const LiteParseResult *result,
                                                  size_t page_index,
                                                  LiteParseByteView phrase,
                                                  bool case_sensitive);

/**
 * Borrow all phrase matches.
 *
 * # Safety
 *
 * `matches` must be live and `out_len` writable.
 */
const LiteParseTextItem *liteparse_search_matches_slice(const LiteParseSearchMatches *matches,
                                                        size_t *out_len);

/**
 * Destroy a search-match handle. Null is allowed.
 */
void liteparse_search_matches_free(LiteParseSearchMatches *matches);

/**
 * Destroy a screenshots handle. Null is allowed.
 */
void liteparse_screenshots_free(LiteParseScreenshots *screenshots);

/**
 * Borrow all rendered pages.
 *
 * # Safety
 *
 * `screenshots` must be live and `out_len` writable.
 */
const LiteParseScreenshot *liteparse_screenshots_slice(const LiteParseScreenshots *screenshots,
                                                       size_t *out_len);

/**
 * Borrow one page's detected rectangles.
 *
 * # Safety
 *
 * `screenshots` must be live and `out_len` writable.
 */
const LiteParseScreenshotRect *liteparse_screenshots_rects(const LiteParseScreenshots *screenshots,
                                                           size_t index,
                                                           size_t *out_len);

/**
 * Borrow this thread's most recent failure message. The view stays valid
 * until the next failed call on the same thread.
 */
LiteParseByteView liteparse_last_error(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LITEPARSE_H */
