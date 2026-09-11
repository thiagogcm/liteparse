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
 * Values for `LiteParseContentGraphic.kind`.
 */
#define LITEPARSE_GRAPHIC_STROKE 0

#define LITEPARSE_GRAPHIC_RECT 1

/**
 * Pixel formats passed to a `LiteParseOcrRecognizeFn`.
 */
#define LITEPARSE_OCR_PIXEL_FORMAT_RGB 0

#define LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE 1

/**
 * Values for `LiteParsePageObject.kind`.
 */
#define LITEPARSE_PAGE_OBJECT_TEXT 0

#define LITEPARSE_PAGE_OBJECT_PATH 1

#define LITEPARSE_PAGE_OBJECT_IMAGE 2

#define LITEPARSE_PAGE_OBJECT_SHADING 3

#define LITEPARSE_PAGE_OBJECT_FORM 4

#define LITEPARSE_PAGE_OBJECT_UNKNOWN 5

/**
 * Values for `LiteParsePathSegment.kind`.
 */
#define LITEPARSE_PATH_SEGMENT_UNKNOWN 0

#define LITEPARSE_PATH_SEGMENT_MOVETO 1

#define LITEPARSE_PATH_SEGMENT_LINETO 2

#define LITEPARSE_PATH_SEGMENT_BEZIERTO 3

/**
 * Values for `LiteParsePageObject.bitmap_format`.
 */
#define LITEPARSE_BITMAP_FORMAT_UNKNOWN 0

#define LITEPARSE_BITMAP_FORMAT_GRAY 1

#define LITEPARSE_BITMAP_FORMAT_BGR 2

#define LITEPARSE_BITMAP_FORMAT_BGRX 3

#define LITEPARSE_BITMAP_FORMAT_BGRA 4

#define LITEPARSE_BITMAP_FORMAT_BGRA_PREMUL 5

/**
 * Copy the image stream as stored (`FPDFImageObj_GetImageDataRaw`).
 */
#define LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW (1 << 0)

/**
 * Copy the stream after lossless filters (`FPDFImageObj_GetImageDataDecoded`).
 */
#define LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED (1 << 1)

/**
 * Copy the image's own pixels (`FPDFImageObj_GetBitmap`), not matrix-rendered.
 */
#define LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP (1 << 2)

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
 * Pre-projection pages from `extract_pages_and_images`. Views borrow here.
 */
typedef struct LiteParseExtract LiteParseExtract;

/**
 * Valid only during the callback that receives it.
 */
typedef struct LiteParseOcrSink LiteParseOcrSink;

/**
 * Unfiltered page content objects. Views borrow from this handle.
 */
typedef struct LiteParsePageObjects LiteParsePageObjects;

/**
 * An owned parser. Safe to share between threads; destruction must wait for
 * in-flight operations.
 */
typedef struct LiteParseParser LiteParseParser;

/**
 * A document's heuristic-free text runs. Views borrow from this handle.
 */
typedef struct LiteParseRawText LiteParseRawText;

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
 * One caller-supplied page. Offset/count pairs index the shared arrays on
 * `LiteParseContent`.
 */
typedef struct {
  /**
   * 1-based source page number.
   */
  uint32_t page_number;
  float page_width;
  float page_height;
  size_t item_offset;
  size_t item_count;
  size_t graphic_offset;
  size_t graphic_count;
  size_t struct_offset;
  size_t struct_count;
  size_t image_ref_offset;
  size_t image_ref_count;
  size_t block_offset;
  size_t block_count;
  /**
   * Empty view means the host supplied no `/PageLabels` entry.
   */
  LiteParseByteView page_label;
} LiteParseContentPage;

/**
 * Borrows data from its result handle. Rich metadata requires
 * `LITEPARSE_FLAG_EXTRACT_TEXT_METADATA`. As parse-content input the same
 * layout is filled by the caller; views are copied during the call.
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
  /**
   * Range into `LiteParseContent.words` when this item is content input.
   * Result accessors leave these zero and use `liteparse_result_word_boxes`.
   */
  size_t word_offset;
  size_t word_count;
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
 * A rectangle in top-left-origin 72-DPI viewport space.
 */
typedef struct {
  float x;
  float y;
  float width;
  float height;
} LiteParseRect;

/**
 * A layout graphic in the same viewport space as the page's text items.
 */
typedef struct {
  /**
   * `LITEPARSE_GRAPHIC_STROKE` or `LITEPARSE_GRAPHIC_RECT`.
   */
  uint32_t kind;
  float x1;
  float y1;
  float x2;
  float y2;
  LiteParseRect bbox;
  LiteParseByteView stroke_color;
  LiteParseByteView fill_color;
  float line_width;
  bool has_fill;
  bool has_stroke;
} LiteParseContentGraphic;

/**
 * One classified layout block. Variant-specific fields that do not apply to
 * `kind` carry their absent encoding (`has_*` false, null views).
 *
 * Table geometry: `header_cell_offset/count` indexes the page's packed cell
 * array; `first_row/row_count` indexes the packed row array from
 * `liteparse_result_block_rows`. Verbatim source lines (`code`,
 * `grid_fallback`) live in the packed line array from
 * `liteparse_result_block_lines`, indexed by `line_offset/line_count`.
 * `merged_table` stores every row in that row range (header rows first,
 * counted by `header_rows`) and puts colspan/rowspan on each cell.
 */
typedef struct {
  /**
   * One of `heading`, `paragraph`, `list_item`, `code`, `table`,
   * `merged_table`, `grid_fallback`, `rule`, `figure`.
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
   * `merged_table`: how many leading rows of the packed row range are header.
   */
  size_t header_rows;
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
  bool has_header_rows;
} LiteParseLayoutBlock;

typedef struct {
  LiteParseByteView text;
  LiteParseRect bbox;
  /**
   * Merge span; `0` or `1` means a single cell. Packed from `merged_table`.
   */
  uint16_t colspan;
  uint16_t rowspan;
  bool has_bbox;
} LiteParseLayoutCell;

/**
 * Row range into `liteparse_result_block_cells`.
 */
typedef struct {
  size_t cell_offset;
  size_t cell_count;
} LiteParseLayoutRow;

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
 * One pre-order structure-tree node used for heading and figure detection.
 */
typedef struct {
  LiteParseByteView role;
  LiteParseByteView alt_text;
  const int32_t *mcids;
  size_t mcids_len;
  LiteParseRect bbox;
  bool has_bbox;
} LiteParseStructNode;

/**
 * Per-page image object. Present even when decoded bytes were not requested.
 */
typedef struct {
  LiteParseByteView id;
  LiteParseByteView format;
  LiteParseRect bbox;
  size_t obj_index;
  uint32_t pixel_width;
  uint32_t pixel_height;
  float rotation;
} LiteParseImageRef;

/**
 * Packed caller-supplied pages for `liteparse_parser_parse_content`.
 *
 * Begin with `liteparse_content_default()`. All views and arrays are copied
 * during the call. Leave the block arrays empty to run projection (and, when
 * configured, classification). Any per-page or document-level block range
 * uses the supplied structure for Markdown instead of classifying.
 */
typedef struct {
  /**
   * Must equal `sizeof(LiteParseContent)`.
   */
  size_t size_of_content;
  const LiteParseContentPage *pages;
  size_t pages_len;
  const LiteParseTextItem *items;
  size_t items_len;
  const LiteParseContentGraphic *graphics;
  size_t graphics_len;
  const LiteParseLayoutBlock *blocks;
  size_t blocks_len;
  const LiteParseLayoutCell *cells;
  size_t cells_len;
  const LiteParseLayoutRow *rows;
  size_t rows_len;
  const LiteParseByteView *lines;
  size_t lines_len;
  const LiteParseOutlineEntry *outline;
  size_t outline_len;
  const LiteParseWordBox *words;
  size_t words_len;
  const LiteParseStructNode *struct_nodes;
  size_t struct_nodes_len;
  const LiteParseImageRef *image_refs;
  size_t image_refs_len;
  /**
   * Range into `blocks` for document-wide structure (`all_blocks`).
   */
  size_t document_block_offset;
  size_t document_block_count;
} LiteParseContent;

/**
 * Fixed-width status code returned by fallible API functions.
 */
typedef uint32_t LiteParseStatus;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseResult *handle;
} LiteParseResultNew;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseDocument *handle;
} LiteParseDocumentNew;

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

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseRawText *handle;
} LiteParseRawTextNew;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParseExtract *handle;
} LiteParseExtractNew;

/**
 * The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
 */
typedef struct {
  LiteParseStatus status;
  LiteParsePageObjects *handle;
} LiteParsePageObjectsNew;

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
  LiteParseRect rect;
  bool present;
} LiteParseRectValue;

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

typedef struct {
  uint32_t page_number;
  LiteParseByteView message;
} LiteParsePageError;

typedef struct {
  /**
   * One of the `LITEPARSE_FORM_TYPE_*` values.
   */
  int32_t value;
  bool present;
} LiteParseFormTypeValue;

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
 * One extracted page. `page_label` borrows from the handle.
 * `object_offset/count` indexes `liteparse_page_objects_objects` for
 * top-level content objects.
 */
typedef struct {
  uint32_t page_number;
  LiteParseByteView page_label;
  float page_width;
  float page_height;
  LiteParsePageGeometry geometry;
  size_t object_offset;
  size_t object_count;
  bool has_geometry;
} LiteParsePageObjectPage;

/**
 * Affine transform as pdfium reports it (`a b c d e f`).
 */
typedef struct {
  float a;
  float b;
  float c;
  float d;
  float e;
  float f;
} LiteParseMatrix;

/**
 * A y-up rectangle in the space pdfium reports for `FPDFPageObj_GetBounds`
 * (`top > bottom`; object matrix applied, ancestor form matrices not).
 */
typedef struct {
  float left;
  float bottom;
  float right;
  float top;
} LiteParsePdfBounds;

/**
 * One content object. Path segments and image filter names are offset/count
 * ranges into the handle's shared arrays. Image payload views borrow here.
 */
typedef struct {
  /**
   * `LITEPARSE_PAGE_OBJECT_*`.
   */
  uint32_t kind;
  LiteParseMatrix matrix;
  LiteParsePdfBounds bounds;
  /**
   * Direct Form XObject children in `liteparse_page_objects_objects`.
   */
  size_t child_offset;
  size_t child_count;
  size_t segment_offset;
  size_t segment_count;
  float stroke_width;
  /**
   * Packed ARGB when the colour space is reportable as RGB.
   */
  uint32_t fill_color;
  uint32_t stroke_color;
  uint32_t image_width;
  uint32_t image_height;
  float image_horizontal_dpi;
  float image_vertical_dpi;
  uint32_t image_bits_per_pixel;
  /**
   * An `FPDF_COLORSPACE_*` value.
   */
  int32_t image_colorspace;
  /**
   * `-1` when the image is not in marked content.
   */
  int32_t image_marked_content_id;
  size_t filter_offset;
  size_t filter_count;
  LiteParseByteView image_raw;
  LiteParseByteView image_decoded;
  LiteParseByteView image_bitmap;
  int32_t bitmap_width;
  int32_t bitmap_height;
  int32_t bitmap_stride;
  /**
   * `LITEPARSE_BITMAP_FORMAT_*`.
   */
  uint32_t bitmap_format;
  bool has_matrix;
  bool has_bounds;
  bool path_filled;
  bool path_stroked;
  bool has_draw_mode;
  bool has_stroke_width;
  bool has_fill_color;
  bool has_stroke_color;
  bool has_image_metadata;
} LiteParsePageObject;

/**
 * One path segment in the object's own coordinate space.
 */
typedef struct {
  /**
   * `LITEPARSE_PATH_SEGMENT_*`. Meaningful when `has_kind` is true.
   */
  uint32_t kind;
  float x;
  float y;
  bool close;
  bool has_kind;
  bool has_point;
} LiteParsePathSegment;

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
 * One extracted page. `page_label` and `geometry` borrow from the handle.
 */
typedef struct {
  uint32_t page_number;
  LiteParseByteView page_label;
  float page_width;
  float page_height;
  LiteParsePageGeometry geometry;
  bool has_geometry;
} LiteParseRawTextPage;

/**
 * One heuristic-free text run. Glyph arrays borrow from the handle.
 */
typedef struct {
  LiteParseByteView text;
  LiteParseByteView font_name;
  const uint32_t *char_codes;
  size_t char_codes_len;
  /**
   * Present only for Type3 fonts; parallel to `char_codes`.
   */
  const LiteParseByteView *glyph_names;
  size_t glyph_names_len;
  bool has_glyph_names;
  /**
   * Counter-clockwise radians in `[0, 2π)` with page rotation folded in.
   */
  float angle_radians;
  float text_width;
  float x;
  float y;
  float width;
  float height;
  /**
   * Tight viewport-space bounds of non-generated, non-space glyphs.
   */
  LiteParseRectValue grounding_bounds;
  /**
   * Advance gap across a lone generated space, in page points.
   */
  double baseline_gap;
  bool has_baseline_gap;
  int32_t mcid;
  bool has_mcid;
  float font_size;
  int32_t font_weight;
  float font_height;
  float font_ascent;
  float font_descent;
  /**
   * Packed ARGB when the colour space is reportable as RGB.
   */
  uint32_t fill_color;
  uint32_t stroke_color;
  bool has_fill_color;
  bool has_stroke_color;
  bool font_is_buggy;
  bool trailing_space_generated;
} LiteParseRawTextItem;

/**
 * Page viewport size in 72-DPI points.
 */
typedef struct {
  float width;
  float height;
} LiteParsePageSize;

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

typedef struct {
  LiteParsePageComplexity stats;
  bool present;
} LiteParsePageComplexityValue;

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

LiteParseContent liteparse_content_default(void);

/**
 * Parse caller-supplied pages. Copies every view during the call.
 *
 * Empty block ranges run grid projection (and the configured classifier).
 * Any per-page or document-level block range treats those blocks as
 * authoritative Markdown. `max_pages` truncates the supplied page list on
 * both paths and drops document-level blocks when it does. `crop_box` and
 * `skip_diagonal_text` are applied before projection, matching parse.
 *
 * # Safety
 *
 * `parser` must be live. `content` and every non-null array/view it names
 * must be readable for the call.
 */
LiteParseResultNew liteparse_parser_parse_content(const LiteParseParser *parser,
                                                  const LiteParseContent *content);

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
 * Extract heuristic-free text runs for selected pages. Null with zero
 * length selects all. `max_pages` caps either. No projection, OCR, or
 * Markdown — every pdfium glyph lands in exactly one item.
 *
 * # Safety
 *
 * `document` must be live and `pages` readable, or null with zero length.
 */
LiteParseRawTextNew liteparse_document_raw_text(const LiteParseDocument *document,
                                                const uint32_t *pages,
                                                size_t pages_len);

/**
 * Extract pre-projection pages for selected pages. Null with zero length
 * selects all. `max_pages` caps either. No grid projection, OCR, or
 * Markdown. Link stamping and word boxes follow the same rules as parse
 * (links only when Markdown is requested; word boxes when requested or
 * Markdown). Feed `liteparse_extract_as_content` into
 * `liteparse_parser_parse_content` to project and classify. That path
 * applies crop_box and skip_diagonal_text.
 *
 * # Safety
 *
 * `document` must be live and `pages` readable, or null with zero length.
 */
LiteParseExtractNew liteparse_document_extract(const LiteParseDocument *document,
                                               const uint32_t *pages,
                                               size_t pages_len);

/**
 * Snapshot unfiltered page content objects. Null with zero length selects
 * all. `max_pages` caps either. Geometry is left in the space pdfium
 * reports (object matrix applied, ancestor form matrices not). Form
 * children are packed as contiguous ranges so the host composes matrices
 * and chooses which forms to descend. Image payloads are omitted unless
 * `flags` requests them with `LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_*`.
 * Unknown flag bits are `LITEPARSE_STATUS_INVALID_ARGUMENT`.
 *
 * # Safety
 *
 * `document` must be live and `pages` readable, or null with zero length.
 */
LiteParsePageObjectsNew liteparse_document_page_objects(const LiteParseDocument *document,
                                                        const uint32_t *pages,
                                                        size_t pages_len,
                                                        uint32_t flags);

/**
 * Destroy an extract handle. Null is allowed.
 */
void liteparse_extract_free(LiteParseExtract *extract);

/**
 * Packed pages for `liteparse_parser_parse_content`.
 *
 * Pointers borrow from `extract` and stay valid until it is freed. The
 * snapshot includes items, graphics, word boxes, struct-tree nodes, and
 * image refs. Blocks, outline, annotations, forms, and the logical
 * structure tree are empty here; use the extract accessors for those.
 * Keep the handle alive for the duration of `liteparse_parser_parse_content`.
 *
 * `extract` must be live, or null (returns an empty default content).
 */
LiteParseContent liteparse_extract_as_content(const LiteParseExtract *extract);

/**
 * Return the number of successfully extracted pages.
 *
 * # Safety
 *
 * `extract` must be live.
 */
size_t liteparse_extract_page_count(const LiteParseExtract *extract);

/**
 * Borrow packed pages. Offset/count pairs index the shared item and graphic
 * arrays.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseContentPage *liteparse_extract_pages(const LiteParseExtract *extract,
                                                    size_t *out_len);

/**
 * Borrow the flattened pre-projection text items.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseTextItem *liteparse_extract_items(const LiteParseExtract *extract, size_t *out_len);

/**
 * Borrow the flattened structure-tree nodes used for heading detection.
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseStructNode *liteparse_extract_struct_nodes(const LiteParseExtract *extract,
                                                          size_t *out_len);

/**
 * Borrow the flattened layout graphics.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseContentGraphic *liteparse_extract_graphics(const LiteParseExtract *extract,
                                                          size_t *out_len);

/**
 * Borrow one page's `/PageLabels` label, or an empty view when absent.
 *
 * # Safety
 *
 * `extract` must be live.
 */
LiteParseByteView liteparse_extract_page_label(const LiteParseExtract *extract, size_t page_index);

/**
 * Return the resolved PDF box, user unit, and rotation for one page.
 *
 * # Safety
 *
 * `extract` must be live.
 */
LiteParsePageGeometryValue liteparse_extract_page_geometry(const LiteParseExtract *extract,
                                                           size_t page_index);

/**
 * Return one page's union content bounds.
 *
 * # Safety
 *
 * `extract` must be live.
 */
LiteParseRectValue liteparse_extract_page_content_bounds(const LiteParseExtract *extract,
                                                         size_t page_index);

/**
 * Borrow one text item's word boxes. `item_index` is per-page.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseWordBox *liteparse_extract_word_boxes(const LiteParseExtract *extract,
                                                     size_t page_index,
                                                     size_t item_index,
                                                     size_t *out_len);

/**
 * Borrow decoded images. Empty unless `LITEPARSE_FLAG_EXTRACT_IMAGES`.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseImage *liteparse_extract_images(const LiteParseExtract *extract, size_t *out_len);

/**
 * Return the count of image extraction failures.
 *
 * # Safety
 *
 * `extract` must be live.
 */
uint32_t liteparse_extract_image_error_count(const LiteParseExtract *extract);

/**
 * Borrow one page's image objects, including bounds when bytes were skipped.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseImageRef *liteparse_extract_image_refs(const LiteParseExtract *extract,
                                                      size_t page_index,
                                                      size_t *out_len);

/**
 * Borrow tolerated page errors.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParsePageError *liteparse_extract_page_errors(const LiteParseExtract *extract,
                                                        size_t *out_len);

/**
 * Whether extraction flattened at least one selected page to recover
 * form-widget text. Flattening happens on a temporary PDFium document,
 * not the document handle.
 *
 * Hosts that must reproduce the flattened content stream (for example an
 * OCR raster) should flatten exactly the pages from
 * `liteparse_extract_flattened_page_numbers`. Flattening any other page
 * hides that page's non-widget annotations. C screenshot does not flatten;
 * it renders the reopened, unflattened input.
 *
 * `extract` must be live.
 */
bool liteparse_extract_flattened_form_widgets(const LiteParseExtract *extract);

/**
 * Borrow the 1-based page numbers that were flattened during extraction.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const uint32_t *liteparse_extract_flattened_page_numbers(const LiteParseExtract *extract,
                                                         size_t *out_len);

/**
 * Return the document form type when form-field extraction was enabled.
 *
 * # Safety
 *
 * `extract` must be live.
 */
LiteParseFormTypeValue liteparse_extract_form_type(const LiteParseExtract *extract);

/**
 * Borrow one page's annotations.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseAnnotation *liteparse_extract_annotations(const LiteParseExtract *extract,
                                                         size_t page_index,
                                                         size_t *out_len);

/**
 * Borrow one annotation's quadpoint rectangles.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseRect *liteparse_extract_annotation_quadpoints(const LiteParseExtract *extract,
                                                             size_t page_index,
                                                             size_t annotation_index,
                                                             size_t *out_len);

/**
 * Borrow one page's AcroForm widgets.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseFormField *liteparse_extract_form_fields(const LiteParseExtract *extract,
                                                        size_t page_index,
                                                        size_t *out_len);

/**
 * Borrow one widget's option strings.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseByteView *liteparse_extract_form_field_options(const LiteParseExtract *extract,
                                                              size_t page_index,
                                                              size_t field_index,
                                                              size_t *out_len);

/**
 * Borrow one widget's selected option strings.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseByteView *liteparse_extract_form_field_selected_options(const LiteParseExtract *extract,
                                                                       size_t page_index,
                                                                       size_t field_index,
                                                                       size_t *out_len);

/**
 * Borrow one page's pre-order structure-tree nodes.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseStructureNode *liteparse_extract_structure_nodes(const LiteParseExtract *extract,
                                                                size_t page_index,
                                                                size_t *out_len);

/**
 * Borrow one page's flattened structure attributes.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseStructureAttribute *liteparse_extract_structure_attributes(const LiteParseExtract *extract,
                                                                          size_t page_index,
                                                                          size_t *out_len);

/**
 * Borrow one page's flattened structure-node annotations.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseAnnotation *liteparse_extract_structure_annotations(const LiteParseExtract *extract,
                                                                   size_t page_index,
                                                                   size_t *out_len);

/**
 * Borrow one page's flattened structure-node marked-content ids.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const int32_t *liteparse_extract_structure_marked_content_ids(const LiteParseExtract *extract,
                                                              size_t page_index,
                                                              size_t *out_len);

/**
 * Borrow one page's vector path objects.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseVectorShape *liteparse_extract_vector_shapes(const LiteParseExtract *extract,
                                                            size_t page_index,
                                                            size_t *out_len);

/**
 * Borrow one page's merged vector segments.
 *
 * # Safety
 *
 * `extract` must be live and `out_len` writable.
 */
const LiteParseVectorLine *liteparse_extract_vector_lines(const LiteParseExtract *extract,
                                                          size_t page_index,
                                                          size_t *out_len);

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
 * Destroy a page-objects handle. Null is allowed.
 */
void liteparse_page_objects_free(LiteParsePageObjects *page_objects);

/**
 * Return the number of extracted pages.
 *
 * # Safety
 *
 * `page_objects` must be live.
 */
size_t liteparse_page_objects_page_count(const LiteParsePageObjects *page_objects);

/**
 * Borrow the extracted pages.
 *
 * # Safety
 *
 * `page_objects` must be live and `out_len` writable.
 */
const LiteParsePageObjectPage *liteparse_page_objects_pages(const LiteParsePageObjects *page_objects,
                                                            size_t *out_len);

/**
 * Borrow the flattened content objects. Page and form ranges index this array.
 *
 * # Safety
 *
 * `page_objects` must be live and `out_len` writable.
 */
const LiteParsePageObject *liteparse_page_objects_objects(const LiteParsePageObjects *page_objects,
                                                          size_t *out_len);

/**
 * Borrow the flattened path segments. Object `segment_offset/count` indexes here.
 *
 * # Safety
 *
 * `page_objects` must be live and `out_len` writable.
 */
const LiteParsePathSegment *liteparse_page_objects_segments(const LiteParsePageObjects *page_objects,
                                                            size_t *out_len);

/**
 * Borrow image filter names. Object `filter_offset/count` indexes here.
 *
 * # Safety
 *
 * `page_objects` must be live and `out_len` writable.
 */
const LiteParseByteView *liteparse_page_objects_image_filters(const LiteParsePageObjects *page_objects,
                                                              size_t *out_len);

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
 * Destroy a raw-text handle. Null is allowed.
 */
void liteparse_raw_text_free(LiteParseRawText *raw_text);

/**
 * Return the number of extracted pages.
 *
 * # Safety
 *
 * `raw_text` must be live.
 */
size_t liteparse_raw_text_page_count(const LiteParseRawText *raw_text);

/**
 * Borrow the extracted pages.
 *
 * # Safety
 *
 * `raw_text` must be live and `out_len` writable.
 */
const LiteParseRawTextPage *liteparse_raw_text_pages(const LiteParseRawText *raw_text,
                                                     size_t *out_len);

/**
 * Borrow one page's raw text items.
 *
 * # Safety
 *
 * `raw_text` must be live and `out_len` writable.
 */
const LiteParseRawTextItem *liteparse_raw_text_items(const LiteParseRawText *raw_text,
                                                     size_t page_index,
                                                     size_t *out_len);

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
 * Borrow a page's `/PageLabels` label, or an empty view when the PDF has none.
 *
 * # Safety
 *
 * `result` must be live. The view is valid until `liteparse_result_free`.
 */
LiteParseByteView liteparse_result_page_label(const LiteParseResult *result, size_t page_index);

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
 * Borrow one page's image objects, including bounds when bytes were skipped.
 *
 * `result` must be live and `out_len` writable.
 */
const LiteParseImageRef *liteparse_result_image_refs(const LiteParseResult *result,
                                                     size_t page_index,
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
