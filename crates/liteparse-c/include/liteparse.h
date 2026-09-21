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
 * Keeps the native default in `u32` config fields.
 */
#define LITEPARSE_UNSET UINT32_MAX

/**
 * `LiteParseConfig.flags` bits.
 */
#define LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX (1 << 0)

/**
 * `LiteParseConfig.bools_set` / `bools_values` bits. Bits are ABI-stable:
 * append new flags without renumbering existing ones.
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
 * `LiteParseDocumentInfo.flags` bit: the source was converted to PDF (an
 * office document or image).
 */
#define LITEPARSE_DOCUMENT_FLAG_CONVERTED (1 << 0)

/**
 * `liteparse_parser_set_ocr_callback` flags.
 */
#define LITEPARSE_OCR_FLAG_PREFERS_GRAYSCALE (1 << 0)

/**
 * `LiteParseOcrImage.pixel_format` values.
 */
#define LITEPARSE_OCR_PIXEL_FORMAT_RGB 0

#define LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE 1

/**
 * `LiteParseOcrWord.flags` bits.
 */
#define LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON (1 << 0)

/**
 * `LiteParsePageObject.kind` values.
 */
#define LITEPARSE_PAGE_OBJECT_TEXT 0

#define LITEPARSE_PAGE_OBJECT_PATH 1

#define LITEPARSE_PAGE_OBJECT_IMAGE 2

#define LITEPARSE_PAGE_OBJECT_SHADING 3

#define LITEPARSE_PAGE_OBJECT_FORM 4

#define LITEPARSE_PAGE_OBJECT_UNKNOWN 5

/**
 * `LiteParsePageObject.flags` bits.
 */
#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_MATRIX (1 << 0)

#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_BOUNDS (1 << 1)

#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_DRAW_MODE (1 << 2)

#define LITEPARSE_PAGE_OBJECT_FLAG_PATH_FILLED (1 << 3)

#define LITEPARSE_PAGE_OBJECT_FLAG_PATH_STROKED (1 << 4)

#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_WIDTH (1 << 5)

#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_FILL_COLOR (1 << 6)

#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_COLOR (1 << 7)

#define LITEPARSE_PAGE_OBJECT_FLAG_HAS_IMAGE_METADATA (1 << 8)

/**
 * `LiteParsePathSegment.kind` values.
 */
#define LITEPARSE_PATH_SEGMENT_UNKNOWN 0

#define LITEPARSE_PATH_SEGMENT_MOVETO 1

#define LITEPARSE_PATH_SEGMENT_LINETO 2

#define LITEPARSE_PATH_SEGMENT_BEZIERTO 3

/**
 * `LiteParsePathSegment.flags` bits.
 */
#define LITEPARSE_PATH_SEGMENT_FLAG_CLOSE (1 << 0)

#define LITEPARSE_PATH_SEGMENT_FLAG_HAS_POINT (1 << 1)

/**
 * `LiteParsePageObject.bitmap_format` values.
 */
#define LITEPARSE_BITMAP_FORMAT_UNKNOWN 0

#define LITEPARSE_BITMAP_FORMAT_GRAY 1

#define LITEPARSE_BITMAP_FORMAT_BGR 2

#define LITEPARSE_BITMAP_FORMAT_BGRX 3

#define LITEPARSE_BITMAP_FORMAT_BGRA 4

#define LITEPARSE_BITMAP_FORMAT_BGRA_PREMUL 5

/**
 * `liteparse_document_page_objects` flag: copy the image stream as stored
 * (`FPDFImageObj_GetImageDataRaw`).
 */
#define LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW (1 << 0)

/**
 * Flag: copy the stream after lossless filters
 * (`FPDFImageObj_GetImageDataDecoded`).
 */
#define LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED (1 << 1)

/**
 * Flag: copy the image's own pixels (`FPDFImageObj_GetBitmap`), not
 * matrix-rendered.
 */
#define LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP (1 << 2)

/**
 * `LiteParsePageObjectPage.flags` bits.
 */
#define LITEPARSE_OBJECT_PAGE_FLAG_HAS_GEOMETRY (1 << 0)

#define LITEPARSE_OBJECT_PAGE_FLAG_HAS_ROTATION (1 << 1)

/**
 * `LiteParseRawTextPage.flags` bits.
 */
#define LITEPARSE_RAW_PAGE_FLAG_HAS_GEOMETRY (1 << 0)

#define LITEPARSE_RAW_PAGE_FLAG_HAS_ROTATION (1 << 1)

/**
 * `LiteParseRawTextItem.flags` bits.
 */
#define LITEPARSE_RAW_ITEM_FLAG_HAS_GLYPH_NAMES (1 << 0)

#define LITEPARSE_RAW_ITEM_FLAG_HAS_GROUNDING_BOUNDS (1 << 1)

#define LITEPARSE_RAW_ITEM_FLAG_HAS_BASELINE_GAP (1 << 2)

#define LITEPARSE_RAW_ITEM_FLAG_HAS_MCID (1 << 3)

#define LITEPARSE_RAW_ITEM_FLAG_HAS_FILL_COLOR (1 << 4)

#define LITEPARSE_RAW_ITEM_FLAG_HAS_STROKE_COLOR (1 << 5)

#define LITEPARSE_RAW_ITEM_FLAG_FONT_IS_BUGGY (1 << 6)

#define LITEPARSE_RAW_ITEM_FLAG_TRAILING_SPACE_GENERATED (1 << 7)

/**
 * `LiteParseResultView.form_type` values (PDFium `FPDF_FORMTYPE_*`).
 */
#define LITEPARSE_FORM_TYPE_NONE 0

#define LITEPARSE_FORM_TYPE_ACRO_FORM 1

#define LITEPARSE_FORM_TYPE_XFA_FULL 2

#define LITEPARSE_FORM_TYPE_XFA_FOREGROUND 3

/**
 * `LiteParsePageComplexity.reasons` bits.
 */
#define LITEPARSE_REASON_SCANNED (1 << 0)

#define LITEPARSE_REASON_NO_TEXT (1 << 1)

#define LITEPARSE_REASON_SPARSE_TEXT (1 << 2)

#define LITEPARSE_REASON_EMBEDDED_IMAGES (1 << 3)

#define LITEPARSE_REASON_GARBLED (1 << 4)

#define LITEPARSE_REASON_VECTOR_TEXT (1 << 5)

#define LITEPARSE_REASON_ANNOTATION_TEXT (1 << 6)

/**
 * `LiteParsePageComplexity.layout_reasons` bits.
 */
#define LITEPARSE_LAYOUT_REASON_MULTI_COLUMN (1 << 0)

#define LITEPARSE_LAYOUT_REASON_TABLE_LIKELY (1 << 1)

#define LITEPARSE_LAYOUT_REASON_DENSE_GRAPHICS (1 << 2)

/**
 * `LiteParsePageComplexity.flags` bits.
 */
#define LITEPARSE_COMPLEXITY_FLAG_HAS_SUBSTANTIAL_IMAGES (1 << 0)

#define LITEPARSE_COMPLEXITY_FLAG_FULL_PAGE_IMAGE (1 << 1)

#define LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED (1 << 2)

#define LITEPARSE_COMPLEXITY_FLAG_NEEDS_OCR (1 << 3)

#define LITEPARSE_COMPLEXITY_FLAG_HAS_UNCOVERED_VECTOR_AREA (1 << 4)

#define LITEPARSE_COMPLEXITY_FLAG_HAS_LAYOUT (1 << 5)

#define LITEPARSE_COMPLEXITY_FLAG_LAYOUT_IS_COMPLEX (1 << 6)

/**
 * `LiteParseTextItem.flags` bits.
 */
#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE (1 << 0)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_CONFIDENCE (1 << 1)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_FLAGS (1 << 2)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_HEIGHT (1 << 3)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_ASCENT (1 << 4)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_DESCENT (1 << 5)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_WEIGHT (1 << 6)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_TEXT_WIDTH (1 << 7)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_MCID (1 << 8)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_FILL_COLOR (1 << 9)

#define LITEPARSE_TEXT_ITEM_FLAG_HAS_STROKE_COLOR (1 << 10)

#define LITEPARSE_TEXT_ITEM_FLAG_STRIKE (1 << 11)

#define LITEPARSE_TEXT_ITEM_FLAG_UNICODE_MAP_ERROR (1 << 12)

#define LITEPARSE_TEXT_ITEM_FLAG_FONT_IS_BUGGY (1 << 13)

#define LITEPARSE_TEXT_ITEM_FLAG_TRAILING_SPACE_GENERATED (1 << 14)

/**
 * `LiteParseGraphic.kind` values.
 */
#define LITEPARSE_GRAPHIC_STROKE 0

#define LITEPARSE_GRAPHIC_RECT 1

/**
 * `LiteParseGraphic.flags` bits.
 */
#define LITEPARSE_GRAPHIC_FLAG_HAS_FILL_COLOR (1 << 0)

#define LITEPARSE_GRAPHIC_FLAG_HAS_STROKE_COLOR (1 << 1)

/**
 * `LiteParseStructNode.flags` bits.
 */
#define LITEPARSE_STRUCT_NODE_FLAG_HAS_BBOX (1 << 0)

/**
 * `LiteParseAnnotation.flags` bits.
 */
#define LITEPARSE_ANNOTATION_FLAG_HAS_RECT (1 << 0)

/**
 * `LiteParseFormField.flags` bits.
 */
#define LITEPARSE_FORM_FIELD_FLAG_HAS_OBJECT_NUMBER (1 << 0)

#define LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_COUNT (1 << 1)

#define LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_INDEX (1 << 2)

#define LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED (1 << 3)

#define LITEPARSE_FORM_FIELD_FLAG_CHECKED (1 << 4)

#define LITEPARSE_FORM_FIELD_FLAG_HAS_RECT (1 << 5)

/**
 * `LiteParseStructureNode.parent_index` for a root element.
 */
#define LITEPARSE_NO_PARENT UINT32_MAX

/**
 * `LiteParseStructureAttribute.kind` values.
 */
#define LITEPARSE_STRUCTURE_ATTR_BOOL 0

#define LITEPARSE_STRUCTURE_ATTR_NUMBER 1

#define LITEPARSE_STRUCTURE_ATTR_STRING 2

/**
 * `LiteParseLayoutBlock.kind` values.
 */
#define LITEPARSE_BLOCK_HEADING 0

#define LITEPARSE_BLOCK_PARAGRAPH 1

#define LITEPARSE_BLOCK_LIST_ITEM 2

#define LITEPARSE_BLOCK_CODE 3

#define LITEPARSE_BLOCK_TABLE 4

#define LITEPARSE_BLOCK_MERGED_TABLE 5

#define LITEPARSE_BLOCK_GRID_FALLBACK 6

#define LITEPARSE_BLOCK_RULE 7

#define LITEPARSE_BLOCK_FIGURE 8

/**
 * A kind this binding does not know; rejected on input.
 */
#define LITEPARSE_BLOCK_UNKNOWN 9

/**
 * `LiteParseLayoutBlock.flags` bits.
 */
#define LITEPARSE_BLOCK_FLAG_HAS_LEVEL (1 << 0)

#define LITEPARSE_BLOCK_FLAG_BOLD (1 << 1)

#define LITEPARSE_BLOCK_FLAG_ITALIC (1 << 2)

#define LITEPARSE_BLOCK_FLAG_HAS_ORDERED (1 << 3)

#define LITEPARSE_BLOCK_FLAG_ORDERED (1 << 4)

#define LITEPARSE_BLOCK_FLAG_HAS_BBOX (1 << 5)

#define LITEPARSE_BLOCK_FLAG_HAS_HEADER_ROWS (1 << 6)

#define LITEPARSE_BLOCK_FLAG_HAS_HEADER (1 << 7)

/**
 * `LiteParseLayoutCell.flags` bits.
 */
#define LITEPARSE_CELL_FLAG_HAS_BBOX (1 << 0)

/**
 * `LiteParseVectorShape.flags` / `LiteParseVectorLine.flags` bits.
 */
#define LITEPARSE_VECTOR_FLAG_STROKE (1 << 0)

#define LITEPARSE_VECTOR_FLAG_FILL (1 << 1)

#define LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR (1 << 2)

#define LITEPARSE_VECTOR_FLAG_HAS_FILL_COLOR (1 << 3)

#define LITEPARSE_VECTOR_FLAG_HAS_CURVE (1 << 4)

#define LITEPARSE_VECTOR_FLAG_HAS_STROKE_WIDTH (1 << 5)

/**
 * `LiteParseOutlineEntry.flags` bits.
 */
#define LITEPARSE_OUTLINE_FLAG_HAS_Y_PDF (1 << 0)

/**
 * `LiteParseXfaPacket.flags` bits.
 */
#define LITEPARSE_XFA_FLAG_HAS_CONTENT (1 << 0)

/**
 * `LiteParseDocumentMeta.flags` bits.
 */
#define LITEPARSE_DOC_META_FLAG_HAS_FILE_VERSION (1 << 0)

#define LITEPARSE_DOC_META_FLAG_HAS_IS_ENCRYPTED (1 << 1)

#define LITEPARSE_DOC_META_FLAG_IS_ENCRYPTED (1 << 2)

#define LITEPARSE_DOC_META_FLAG_HAS_SECURITY_HANDLER_REVISION (1 << 3)

#define LITEPARSE_DOC_META_FLAG_HAS_PERMISSIONS (1 << 4)

#define LITEPARSE_DOC_META_FLAG_HAS_EOF_SECTION_COUNT (1 << 5)

#define LITEPARSE_DOC_META_FLAG_HAS_STARTXREF_COUNT (1 << 6)

#define LITEPARSE_DOC_META_FLAG_HAS_TRAILER_ID_PAIR_DIFFERS (1 << 7)

#define LITEPARSE_DOC_META_FLAG_TRAILER_ID_PAIR_DIFFERS (1 << 8)

#define LITEPARSE_DOC_META_FLAG_HAS_RAW_FILE_SIZE (1 << 9)

#define LITEPARSE_DOC_META_FLAG_HAS_XMP_TRUNCATED (1 << 10)

#define LITEPARSE_DOC_META_FLAG_XMP_TRUNCATED (1 << 11)

#define LITEPARSE_DOC_META_FLAG_HAS_SIGNATURE_COUNT (1 << 12)

#define LITEPARSE_DOC_META_FLAG_HAS_SIGNATURE_BYTE_RANGE_REACHES_EOF (1 << 13)

#define LITEPARSE_DOC_META_FLAG_SIGNATURE_BYTE_RANGE_REACHES_EOF (1 << 14)

/**
 * `LiteParseScreenshot.flags` bits.
 */
#define LITEPARSE_SCREENSHOT_FLAG_SOLID_FILL (1 << 0)

/**
 * `LiteParseScreenshotRect.flags` bits.
 */
#define LITEPARSE_SCREENSHOT_RECT_FLAG_IS_LINE (1 << 0)

#define LITEPARSE_SCREENSHOT_RECT_FLAG_HAS_COLOR (1 << 1)

/**
 * `LiteParseProjectedLine.anchor` values.
 */
#define LITEPARSE_ANCHOR_LEFT 0

#define LITEPARSE_ANCHOR_RIGHT 1

#define LITEPARSE_ANCHOR_CENTER 2

#define LITEPARSE_ANCHOR_FLOATING 3

/**
 * `LiteParseProjectedLine.flags` bits.
 */
#define LITEPARSE_LINE_FLAG_HAS_HEADING_FONT_SIZE (1 << 0)

#define LITEPARSE_LINE_FLAG_HAS_MCID (1 << 1)

#define LITEPARSE_LINE_FLAG_ALL_BOLD (1 << 2)

#define LITEPARSE_LINE_FLAG_ALL_ITALIC (1 << 3)

#define LITEPARSE_LINE_FLAG_ALL_MONO (1 << 4)

#define LITEPARSE_LINE_FLAG_ALL_STRIKE (1 << 5)

#define LITEPARSE_LINE_FLAG_FONT_SIZE_ESTIMATED (1 << 6)

#define LITEPARSE_LINE_FLAG_RTL (1 << 7)

#define LITEPARSE_LINE_FLAG_IN_FIGURE (1 << 8)

/**
 * `LiteParseProjectedRegion.flags` bits.
 */
#define LITEPARSE_REGION_FLAG_LEAF (1 << 0)

#define LITEPARSE_REGION_FLAG_SPLIT (1 << 1)

#define LITEPARSE_REGION_FLAG_HORIZONTAL (1 << 2)

#define LITEPARSE_REGION_FLAG_VERTICAL (1 << 3)

/**
 * `LiteParsePage.flags` bits. The `HAS_*` collection bits distinguish
 * "extraction enabled, none found" from "extraction disabled".
 */
#define LITEPARSE_PAGE_FLAG_HAS_GEOMETRY (1 << 0)

#define LITEPARSE_PAGE_FLAG_HAS_ROTATION (1 << 1)

#define LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS (1 << 2)

#define LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY (1 << 3)

#define LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS (1 << 4)

#define LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS (1 << 5)

#define LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE (1 << 6)

#define LITEPARSE_PAGE_FLAG_HAS_BLOCKS (1 << 7)

#define LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS (1 << 8)

/**
 * `LiteParseResultView.flags` bits.
 */
#define LITEPARSE_RESULT_FLAG_HAS_DOC_META (1 << 0)

#define LITEPARSE_RESULT_FLAG_HAS_FORM_TYPE (1 << 1)

#define LITEPARSE_RESULT_FLAG_HAS_XFA_PACKETS (1 << 2)

/**
 * Text metadata extraction was requested in the parser configuration.
 */
#define LITEPARSE_RESULT_FLAG_TEXT_METADATA (1 << 3)

/**
 * Produced by `liteparse_document_extract`: no projection, text, or Markdown.
 */
#define LITEPARSE_RESULT_FLAG_EXTRACT_ONLY (1 << 4)

/**
 * Extraction flattened at least one page to recover form-widget text; see
 * `flattened_page_numbers`.
 */
#define LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS (1 << 5)

/**
 * `liteparse_result_search` flags.
 */
#define LITEPARSE_SEARCH_FLAG_CASE_SENSITIVE (1 << 0)

/**
 * Per-page complexity signals. Views borrow from the handle.
 */
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
 * Unfiltered page content objects. Views borrow from the handle.
 */
typedef struct LiteParsePageObjects LiteParsePageObjects;

/**
 * An owned parser. Safe to share between threads; destruction must wait for
 * in-flight operations.
 */
typedef struct LiteParseParser LiteParseParser;

/**
 * Heuristic-free text runs. Views borrow from the handle.
 */
typedef struct LiteParseRawText LiteParseRawText;

/**
 * A parse or extract result. Views borrow from it until it is freed.
 */
typedef struct LiteParseResult LiteParseResult;

/**
 * Rendered pages. Views borrow from the handle until it is freed.
 */
typedef struct LiteParseScreenshots LiteParseScreenshots;

/**
 * Phrase matches copied out of a result; they outlive it.
 */
typedef struct LiteParseSearchMatches LiteParseSearchMatches;

/**
 * Per-page OCR and layout complexity signals.
 */
typedef struct {
  uint32_t page_number;
  /**
   * `LITEPARSE_COMPLEXITY_FLAG_*` bits.
   */
  uint32_t flags;
  uint32_t text_length;
  uint32_t image_block_count;
  /**
   * Fraction of page area covered by native text.
   */
  float text_coverage;
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
   * `LITEPARSE_REASON_*` bits explaining `NEEDS_OCR`.
   */
  uint32_t reasons;
  /**
   * Side-by-side columns; 1 means a single column.
   */
  uint32_t layout_column_count;
  uint32_t layout_ruled_table_count;
  /**
   * Borderless table runs found by track alignment. Overlaps
   * `layout_ruled_table_count`; the two must not be summed.
   */
  uint32_t layout_text_table_run_count;
  uint32_t layout_figure_count;
  float layout_ruled_table_coverage;
  float layout_figure_coverage;
  /**
   * `LITEPARSE_LAYOUT_REASON_*` bits explaining `LAYOUT_IS_COMPLEX`.
   */
  uint32_t layout_reasons;
} LiteParsePageComplexity;

typedef struct {
  const LiteParsePageComplexity *pages;
  size_t pages_len;
} LiteParseComplexityView;

/**
 * Fixed-width status code returned by fallible API functions.
 */
typedef uint32_t LiteParseStatus;

/**
 * Borrowed, non-NUL-terminated bytes valid while their owner lives. Absent
 * and empty are both a null pointer with zero length.
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
 * One per-page orientation correction: `page` is 1-based, `angle` is the
 * clockwise degrees (0/90/180/270) the content appears rotated. Out-of-range
 * pages are ignored like core.
 */
typedef struct {
  uint32_t page;
  uint32_t angle;
} LiteParsePageOrientationCorrection;

/**
 * Start with `liteparse_config_init`; parser creation copies all views.
 */
typedef struct {
  /**
   * Must equal `sizeof(LiteParseConfig)`.
   */
  size_t size_of_config;
  /**
   * Core booleans: a bit in `bools_set` selects the field, the same bit in
   * `bools_values` gives its value. Unset bits keep the native default.
   */
  uint64_t bools_set;
  uint64_t bools_values;
  /**
   * `LITEPARSE_CONFIG_FLAG_*` bits.
   */
  uint32_t flags;
  /**
   * `LITEPARSE_UNSET` keeps the native default (1000); zero parses no pages.
   */
  uint32_t max_pages;
  /**
   * `LITEPARSE_UNSET` keeps the native default.
   */
  uint32_t num_workers;
  /**
   * `LITEPARSE_UNSET` keeps the native default.
   */
  uint32_t output_format;
  /**
   * `LITEPARSE_UNSET` keeps the native default.
   */
  uint32_t image_mode;
  /**
   * Zero keeps the native default; nonzero values must be finite and > 0.
   */
  float dpi;
  /**
   * Normalized fractions ordered top, right, bottom, left, applied when
   * `LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX` is set. Every value must lie in
   * `[0, 1]` with `top + bottom < 1` and `left + right < 1`.
   */
  float crop_box[4];
  LiteParseByteView ocr_language;
  LiteParseByteView ocr_server_url;
  LiteParseByteView tessdata_path;
  LiteParseByteView password;
  LiteParseByteView image_output_dir;
  /**
   * Optional `%02x%02x.msgpack` glyph-database directory. An explicit path
   * overrides `LITEPARSE_FONT_DB_DIR`.
   */
  LiteParseByteView font_db_dir;
  const LiteParseHeader *ocr_server_headers;
  size_t ocr_server_headers_len;
  const uint64_t *ocr_hedge_delays_ms;
  size_t ocr_hedge_delays_ms_len;
  const LiteParsePageOrientationCorrection *orientation_corrections;
  size_t orientation_corrections_len;
} LiteParseConfig;

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
   * Clockwise quarter turns, `0..=3`; meaningful only with
   * `LITEPARSE_PAGE_FLAG_HAS_ROTATION`.
   */
  uint32_t rotation_quarter_turns;
} LiteParsePageGeometry;

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
 * One page. Ranges index the flat arrays of the owning view; result-only
 * ranges (`figure_*`, `item_frame_*`, `projected_line_*`, `region_*`) are
 * zero on extract and content input. `text`/`markdown` are output only.
 */
typedef struct {
  /**
   * 1-based source page number.
   */
  uint32_t page_number;
  /**
   * `LITEPARSE_PAGE_FLAG_*` bits.
   */
  uint32_t flags;
  /**
   * Viewport size in 72-DPI points.
   */
  float width;
  float height;
  /**
   * The PDF `/PageLabels` entry, or a null view.
   */
  LiteParseByteView label;
  LiteParseByteView text;
  LiteParseByteView markdown;
  LiteParsePageGeometry geometry;
  LiteParseRect content_bounds;
  LiteParsePageComplexity complexity;
  uint32_t item_offset;
  uint32_t item_count;
  uint32_t graphic_offset;
  uint32_t graphic_count;
  uint32_t struct_node_offset;
  uint32_t struct_node_count;
  uint32_t image_ref_offset;
  uint32_t image_ref_count;
  uint32_t annotation_offset;
  uint32_t annotation_count;
  uint32_t form_field_offset;
  uint32_t form_field_count;
  uint32_t structure_node_offset;
  uint32_t structure_node_count;
  uint32_t block_offset;
  uint32_t block_count;
  uint32_t vector_shape_offset;
  uint32_t vector_shape_count;
  uint32_t vector_line_offset;
  uint32_t vector_line_count;
  uint32_t figure_offset;
  uint32_t figure_count;
  uint32_t item_frame_offset;
  uint32_t item_frame_count;
  uint32_t projected_line_offset;
  uint32_t projected_line_count;
  uint32_t region_offset;
  uint32_t region_count;
} LiteParsePage;

/**
 * One text item. Used for page items, projected spans, search matches, and
 * caller-supplied content. Ranges index the owning view's `words` and
 * `char_codes` arrays.
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
  float font_height;
  float font_ascent;
  float font_descent;
  float text_width;
  int32_t font_flags;
  int32_t font_weight;
  int32_t mcid;
  /**
   * Packed ARGB.
   */
  uint32_t fill_color;
  uint32_t stroke_color;
  uint32_t char_code_offset;
  uint32_t char_code_count;
  uint32_t word_offset;
  uint32_t word_count;
  /**
   * `LITEPARSE_TEXT_ITEM_FLAG_*` bits.
   */
  uint32_t flags;
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
 * A layout graphic primitive in viewport space. Strokes use `x1..y2` and
 * `line_width`; rects use `bbox`.
 */
typedef struct {
  /**
   * `LITEPARSE_GRAPHIC_*`.
   */
  uint32_t kind;
  /**
   * `LITEPARSE_GRAPHIC_FLAG_*` bits.
   */
  uint32_t flags;
  float x1;
  float y1;
  float x2;
  float y2;
  float line_width;
  LiteParseRect bbox;
  uint32_t fill_color;
  uint32_t stroke_color;
} LiteParseGraphic;

/**
 * One pre-order structure-tree node used for heading and figure detection.
 * `mcid_offset/count` index the view's `mcids` array.
 */
typedef struct {
  LiteParseByteView role;
  LiteParseByteView alt_text;
  LiteParseRect bbox;
  uint32_t mcid_offset;
  uint32_t mcid_count;
  /**
   * `LITEPARSE_STRUCT_NODE_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseStructNode;

/**
 * Per-page raster image object, present even when bytes were not extracted.
 */
typedef struct {
  LiteParseByteView id;
  LiteParseByteView format;
  LiteParseRect bbox;
  uint32_t obj_index;
  uint32_t pixel_width;
  uint32_t pixel_height;
  uint32_t bits_per_pixel;
  /**
   * An `FPDF_COLORSPACE_*` value.
   */
  int32_t colorspace;
  float rotation;
} LiteParseImageRef;

/**
 * Extracted image with encoded bytes.
 */
typedef struct {
  LiteParseByteView id;
  LiteParseByteView name;
  LiteParseByteView path;
  LiteParseByteView format;
  LiteParseByteView duplicate_of;
  LiteParseByteView bytes;
  LiteParseRect bbox;
  uint32_t page;
  uint32_t width;
  uint32_t height;
  float rotation;
} LiteParseImage;

/**
 * Page or structure-node annotation. `quadpoint_offset/count` index the
 * view's `quadpoints` array.
 */
typedef struct {
  LiteParseByteView subtype;
  LiteParseByteView contents;
  LiteParseByteView created;
  LiteParseByteView modified;
  LiteParseByteView title;
  LiteParseByteView uri;
  LiteParseRect rect;
  uint32_t quadpoint_offset;
  uint32_t quadpoint_count;
  /**
   * `LITEPARSE_ANNOTATION_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseAnnotation;

/**
 * AcroForm widget. Option ranges index the view's `strings` array.
 */
typedef struct {
  LiteParseByteView id;
  LiteParseByteView field_type;
  LiteParseByteView name;
  LiteParseByteView alternate_name;
  LiteParseByteView value;
  LiteParseByteView export_value;
  LiteParseRect rect;
  uint32_t page;
  int32_t annotation_index;
  int32_t widget_index;
  int32_t object_number;
  int32_t field_flags;
  int32_t control_count;
  int32_t control_index;
  uint32_t option_offset;
  uint32_t option_count;
  uint32_t selected_option_offset;
  uint32_t selected_option_count;
  /**
   * `LITEPARSE_FORM_FIELD_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseFormField;

/**
 * One tagged-PDF structure element, flattened in pre-order (parent before
 * children). `parent_index` is an absolute index into the view's
 * `structure_nodes` array or `LITEPARSE_NO_PARENT`. Ranges index the view's
 * `mcids`, `structure_attributes`, and `annotations` arrays.
 */
typedef struct {
  LiteParseByteView element_type;
  LiteParseByteView id;
  LiteParseByteView actual_text;
  LiteParseByteView alt_text;
  LiteParseByteView title;
  uint32_t parent_index;
  /**
   * Nesting depth, 0 for roots.
   */
  uint32_t depth;
  uint32_t mcid_offset;
  uint32_t mcid_count;
  uint32_t attribute_offset;
  uint32_t attribute_count;
  uint32_t annotation_offset;
  uint32_t annotation_count;
} LiteParseStructureNode;

/**
 * One `/A` attribute. Booleans are `kind == BOOL` with `number` 0 or 1.
 */
typedef struct {
  LiteParseByteView name;
  LiteParseByteView string;
  /**
   * `LITEPARSE_STRUCTURE_ATTR_*`.
   */
  uint32_t kind;
  float number;
} LiteParseStructureAttribute;

/**
 * One classified layout block. Fields that do not apply to `kind` carry
 * their absent encoding. Tables: `header_cell_offset/count` and each row's
 * `cell_offset/count` index `cells`; `row_offset/count` index `rows`.
 * `merged_table` stores header rows first (`header_rows` of them) and puts
 * colspan/rowspan on each cell. `code`/`grid_fallback` lines are
 * `line_offset/count` into `strings`.
 */
typedef struct {
  LiteParseByteView text;
  LiteParseByteView marker;
  LiteParseByteView lang;
  /**
   * Figure image id and encoded format.
   */
  LiteParseByteView id;
  LiteParseByteView format;
  LiteParseRect bbox;
  /**
   * `LITEPARSE_BLOCK_*`.
   */
  uint32_t kind;
  /**
   * `LITEPARSE_BLOCK_FLAG_*` bits.
   */
  uint32_t flags;
  /**
   * Heading level (1-6), or list nesting depth.
   */
  uint32_t level;
  uint32_t line_offset;
  uint32_t line_count;
  uint32_t header_cell_offset;
  uint32_t header_cell_count;
  uint32_t row_offset;
  uint32_t row_count;
  uint32_t header_rows;
} LiteParseLayoutBlock;

typedef struct {
  LiteParseByteView text;
  LiteParseRect bbox;
  /**
   * Merge span; `0` or `1` means a single cell.
   */
  uint32_t colspan;
  uint32_t rowspan;
  /**
   * `LITEPARSE_CELL_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseLayoutCell;

/**
 * Row range into `cells`.
 */
typedef struct {
  uint32_t cell_offset;
  uint32_t cell_count;
} LiteParseLayoutRow;

typedef struct {
  LiteParseRect bbox;
  uint32_t stroke_color;
  uint32_t fill_color;
  /**
   * `LITEPARSE_VECTOR_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseVectorShape;

typedef struct {
  float x1;
  float y1;
  float x2;
  float y2;
  float stroke_width;
  uint32_t stroke_color;
  uint32_t fill_color;
  /**
   * `LITEPARSE_VECTOR_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseVectorLine;

/**
 * One outline entry (bookmark). `page_index` is zero-based and `-1` when the
 * destination is not a page; `y_pdf` is PDF user space.
 */
typedef struct {
  LiteParseByteView title;
  int32_t page_index;
  float y_pdf;
  /**
   * Hierarchy depth, 1-based.
   */
  uint32_t level;
  /**
   * `LITEPARSE_OUTLINE_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseOutlineEntry;

typedef struct {
  LiteParseByteView message;
  uint32_t page_number;
} LiteParsePageError;

/**
 * Packed page content. Read from a result view, or filled by the caller for
 * `liteparse_parser_parse_content`.
 *
 * Pages carry offset/count ranges into the flat arrays. `strings` holds
 * form-field options and block source lines; `annotations` holds page and
 * structure-node annotations; `mcids` holds struct-node and structure-tree
 * marked-content ids; `words` and `char_codes` are shared by every text
 * item. `document_block_offset/count` selects document-wide blocks on input
 * only; results never report them.
 */
typedef struct {
  /**
   * Must equal `sizeof(LiteParseContent)` on input.
   */
  size_t size_of_content;
  const LiteParsePage *pages;
  size_t pages_len;
  const LiteParseTextItem *items;
  size_t items_len;
  const LiteParseWordBox *words;
  size_t words_len;
  const uint32_t *char_codes;
  size_t char_codes_len;
  const LiteParseGraphic *graphics;
  size_t graphics_len;
  const LiteParseStructNode *struct_nodes;
  size_t struct_nodes_len;
  const int32_t *mcids;
  size_t mcids_len;
  const LiteParseImageRef *image_refs;
  size_t image_refs_len;
  const LiteParseImage *images;
  size_t images_len;
  const LiteParseAnnotation *annotations;
  size_t annotations_len;
  const LiteParseRect *quadpoints;
  size_t quadpoints_len;
  const LiteParseFormField *form_fields;
  size_t form_fields_len;
  const LiteParseByteView *strings;
  size_t strings_len;
  const LiteParseStructureNode *structure_nodes;
  size_t structure_nodes_len;
  const LiteParseStructureAttribute *structure_attributes;
  size_t structure_attributes_len;
  const LiteParseLayoutBlock *blocks;
  size_t blocks_len;
  const LiteParseLayoutCell *cells;
  size_t cells_len;
  const LiteParseLayoutRow *rows;
  size_t rows_len;
  const LiteParseVectorShape *vector_shapes;
  size_t vector_shapes_len;
  const LiteParseVectorLine *vector_lines;
  size_t vector_lines_len;
  const LiteParseOutlineEntry *outline;
  size_t outline_len;
  const LiteParsePageError *page_errors;
  size_t page_errors_len;
  uint32_t document_block_offset;
  uint32_t document_block_count;
} LiteParseContent;

/**
 * Facts recorded when a document is opened. Borrowed until the document is
 * freed.
 */
typedef struct {
  uint32_t total_pages;
  /**
   * `LITEPARSE_DOCUMENT_FLAG_*` bits.
   */
  uint32_t flags;
  /**
   * Bookmarks, walked once at open.
   */
  const LiteParseOutlineEntry *outline;
  size_t outline_len;
} LiteParseDocumentInfo;

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
 * One recognized word. Box edges are raster pixels; `polygon` holds four
 * x/y corners in reading order when `HAS_POLYGON` is set.
 */
typedef struct {
  LiteParseByteView text;
  float x1;
  float y1;
  float x2;
  float y2;
  float confidence;
  float polygon[8];
  /**
   * `LITEPARSE_OCR_WORD_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseOcrWord;

/**
 * One extracted page. `object_offset/count` index the view's `objects`
 * for top-level content objects.
 */
typedef struct {
  LiteParseByteView label;
  LiteParsePageGeometry geometry;
  uint32_t page_number;
  /**
   * `LITEPARSE_OBJECT_PAGE_FLAG_*` bits.
   */
  uint32_t flags;
  float width;
  float height;
  uint32_t object_offset;
  uint32_t object_count;
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
 * One content object. Ranges index the view's `objects` (direct Form
 * XObject children), `segments`, and `filters` arrays. Image payloads are
 * null unless requested.
 */
typedef struct {
  LiteParseByteView image_raw;
  LiteParseByteView image_decoded;
  LiteParseByteView image_bitmap;
  LiteParseMatrix matrix;
  LiteParsePdfBounds bounds;
  /**
   * `LITEPARSE_PAGE_OBJECT_*`.
   */
  uint32_t kind;
  /**
   * `LITEPARSE_PAGE_OBJECT_FLAG_*` bits.
   */
  uint32_t flags;
  uint32_t child_offset;
  uint32_t child_count;
  uint32_t segment_offset;
  uint32_t segment_count;
  uint32_t filter_offset;
  uint32_t filter_count;
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
  int32_t bitmap_width;
  int32_t bitmap_height;
  int32_t bitmap_stride;
  /**
   * `LITEPARSE_BITMAP_FORMAT_*`.
   */
  uint32_t bitmap_format;
} LiteParsePageObject;

/**
 * One path segment in the object's own coordinate space.
 */
typedef struct {
  /**
   * `LITEPARSE_PATH_SEGMENT_*`.
   */
  uint32_t kind;
  /**
   * `LITEPARSE_PATH_SEGMENT_FLAG_*` bits.
   */
  uint32_t flags;
  float x;
  float y;
} LiteParsePathSegment;

typedef struct {
  const LiteParsePageObjectPage *pages;
  size_t pages_len;
  const LiteParsePageObject *objects;
  size_t objects_len;
  const LiteParsePathSegment *segments;
  size_t segments_len;
  const LiteParseByteView *filters;
  size_t filters_len;
} LiteParsePageObjectsView;

/**
 * The raster handed to an OCR callback. Borrowed for the callback's
 * duration.
 */
typedef struct {
  /**
   * Tightly packed rows, 3 bytes per pixel for RGB or 1 for grayscale.
   */
  LiteParseByteView pixels;
  uint32_t width;
  uint32_t height;
  /**
   * `LITEPARSE_OCR_PIXEL_FORMAT_*`.
   */
  uint32_t pixel_format;
  float dpi;
  /**
   * The configured OCR language.
   */
  LiteParseByteView language;
} LiteParseOcrImage;

/**
 * Return nonzero to fail recognition. Calls may be concurrent.
 */
typedef uint32_t (*LiteParseOcrRecognizeFn)(void *user_data,
                                            const LiteParseOcrImage *image,
                                            LiteParseOcrSink *sink);

/**
 * One extracted page. `item_offset/count` index the view's `items`.
 */
typedef struct {
  LiteParseByteView label;
  LiteParsePageGeometry geometry;
  uint32_t page_number;
  /**
   * `LITEPARSE_RAW_PAGE_FLAG_*` bits.
   */
  uint32_t flags;
  float width;
  float height;
  uint32_t item_offset;
  uint32_t item_count;
} LiteParseRawTextPage;

/**
 * One heuristic-free text run. `char_code_offset/count` index the view's
 * `char_codes`; `glyph_name_offset/count` (Type3 fonts only, parallel to
 * the char codes) index `glyph_names`.
 */
typedef struct {
  LiteParseByteView text;
  LiteParseByteView font_name;
  /**
   * Advance gap across a lone generated space, in page points.
   */
  double baseline_gap;
  /**
   * Tight viewport-space bounds of non-generated, non-space glyphs.
   */
  LiteParseRect grounding_bounds;
  /**
   * Counter-clockwise radians in `[0, 2π)` with page rotation folded in.
   */
  float angle_radians;
  float text_width;
  float x;
  float y;
  float width;
  float height;
  float font_size;
  float font_height;
  float font_ascent;
  float font_descent;
  int32_t font_weight;
  int32_t mcid;
  /**
   * Packed ARGB when the colour space is reportable as RGB.
   */
  uint32_t fill_color;
  uint32_t stroke_color;
  uint32_t char_code_offset;
  uint32_t char_code_count;
  uint32_t glyph_name_offset;
  uint32_t glyph_name_count;
  /**
   * `LITEPARSE_RAW_ITEM_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseRawTextItem;

typedef struct {
  const LiteParseRawTextPage *pages;
  size_t pages_len;
  const LiteParseRawTextItem *items;
  size_t items_len;
  const uint32_t *char_codes;
  size_t char_codes_len;
  const LiteParseByteView *glyph_names;
  size_t glyph_names_len;
} LiteParseRawTextView;

/**
 * Document metadata. Absent strings are null views; scalars use flags.
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
  LiteParseByteView xmp;
  uint64_t permissions;
  uint64_t raw_file_size;
  int32_t file_version;
  int32_t security_handler_revision;
  uint32_t eof_section_count;
  uint32_t startxref_count;
  uint32_t signature_count;
  /**
   * `LITEPARSE_DOC_META_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseDocumentMeta;

/**
 * Rendered page PNG. `rect_offset/count` index the view's
 * `screenshot_rects` array.
 */
typedef struct {
  LiteParseByteView png;
  uint32_t page_number;
  uint32_t width;
  uint32_t height;
  /**
   * Resolution the page was actually rendered at: the requested DPI unless
   * the renderer lowered it to keep the long edge under 30,000 pixels. For
   * a region render it still describes the page.
   */
  float effective_dpi;
  uint32_t rect_offset;
  uint32_t rect_count;
  /**
   * `LITEPARSE_SCREENSHOT_FLAG_*` bits.
   */
  uint32_t flags;
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
  /**
   * Packed ARGB.
   */
  uint32_t color;
  /**
   * `LITEPARSE_SCREENSHOT_RECT_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseScreenshotRect;

/**
 * XFA packet; `content` is lossily decoded UTF-8.
 */
typedef struct {
  LiteParseByteView name;
  LiteParseByteView content;
  uint32_t index;
  uint32_t content_length;
  /**
   * `LITEPARSE_XFA_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseXfaPacket;

/**
 * Projected and original geometry of one text item on pages where rotation
 * handling displaced content.
 */
typedef struct {
  LiteParseRect projected;
  LiteParseRect original;
} LiteParseItemFrame;

/**
 * One projected text line. `span_offset/count` index the view's
 * `projected_spans` array; `region_path_offset/count` index `region_paths`
 * (child ordinals from the page's region root).
 */
typedef struct {
  LiteParseByteView text;
  LiteParseByteView dominant_font_name;
  LiteParseRect bbox;
  float indent_x;
  float dominant_font_size;
  float heading_font_size;
  int32_t mcid;
  /**
   * `LITEPARSE_ANCHOR_*`.
   */
  uint32_t anchor;
  /**
   * `LITEPARSE_LINE_FLAG_*` bits.
   */
  uint32_t flags;
  uint32_t span_offset;
  uint32_t span_count;
  uint32_t region_path_offset;
  uint32_t region_path_count;
} LiteParseProjectedLine;

/**
 * One XY-cut region, flattened in pre-order. `parent_index` and the entries
 * of `region_children` are absolute indices into `regions`. Leaves
 * correspond to the `region_paths` on projected lines; the core keeps no
 * leaf-to-item mapping that the result can expose.
 */
typedef struct {
  LiteParseRect bbox;
  uint32_t parent_index;
  uint32_t child_offset;
  uint32_t child_count;
  /**
   * `LITEPARSE_REGION_FLAG_*` bits.
   */
  uint32_t flags;
} LiteParseProjectedRegion;

/**
 * Everything a result exposes. `content` is the page-content model shared
 * with `liteparse_parser_parse_content`; the remaining arrays are result
 * only. Projected spans share `content.words` and `content.char_codes`.
 */
typedef struct {
  LiteParseContent content;
  /**
   * Full-document plain text or Markdown, per the output format.
   */
  LiteParseByteView text;
  LiteParseByteView creator;
  LiteParseByteView producer;
  LiteParseDocumentMeta doc_meta;
  uint32_t total_pages;
  uint32_t image_error_count;
  /**
   * `LITEPARSE_FORM_TYPE_*`, meaningful with `HAS_FORM_TYPE`.
   */
  int32_t form_type;
  /**
   * `LITEPARSE_RESULT_FLAG_*` bits.
   */
  uint32_t flags;
  const LiteParseScreenshot *screenshots;
  size_t screenshots_len;
  const LiteParseScreenshotRect *screenshot_rects;
  size_t screenshot_rects_len;
  const LiteParseXfaPacket *xfa_packets;
  size_t xfa_packets_len;
  const LiteParseRect *figures;
  size_t figures_len;
  const LiteParseItemFrame *item_frames;
  size_t item_frames_len;
  const LiteParseProjectedLine *projected_lines;
  size_t projected_lines_len;
  const LiteParseTextItem *projected_spans;
  size_t projected_spans_len;
  const uint16_t *region_paths;
  size_t region_paths_len;
  const LiteParseProjectedRegion *regions;
  size_t regions_len;
  const uint32_t *region_children;
  size_t region_children_len;
  const uint32_t *flattened_page_numbers;
  size_t flattened_page_numbers_len;
} LiteParseResultView;

/**
 * Text items copied out of a result by `liteparse_result_search`.
 */
typedef struct {
  const LiteParseTextItem *items;
  size_t items_len;
  const LiteParseWordBox *words;
  size_t words_len;
  const uint32_t *char_codes;
  size_t char_codes_len;
} LiteParseSearchView;

/**
 * Rendered pages and the solid rectangles detected on them.
 */
typedef struct {
  const LiteParseScreenshot *screenshots;
  size_t screenshots_len;
  const LiteParseScreenshotRect *rects;
  size_t rects_len;
} LiteParseScreenshotsView;

/**
 * The operation succeeded.
 */
#define LITEPARSE_STATUS_OK 0

/**
 * A pointer, length, range, or input string was invalid.
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
 * handle involved and do not reuse it.
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
 * Borrow the analyzed pages; null for a null handle.
 *
 * `complexity` must be null or live.
 */
const LiteParseComplexityView *liteparse_complexity_view(const LiteParseComplexity *complexity);

/**
 * Borrow the cached JSON report.
 *
 * `complexity` must be live and `out` writable.
 */
LiteParseStatus liteparse_complexity_to_json(const LiteParseComplexity *complexity,
                                             LiteParseByteView *out);

/**
 * Fill `config` with defaults. Null is a no-op.
 *
 * `config` must be null or writable.
 */
void liteparse_config_init(LiteParseConfig *config);

/**
 * Fill `content` with an empty, correctly sized value. Null is a no-op.
 *
 * `content` must be null or writable.
 */
void liteparse_content_init(LiteParseContent *content);

/**
 * Parse caller-supplied pages without opening a document. Every view and
 * array is copied during the call.
 *
 * With no block ranges the pages run grid projection (and the configured
 * classifier). Any per-page or document-level block range makes those
 * blocks the Markdown structure; `images` and per-page complexity are
 * forwarded on that path. `max_pages` truncates the page list and drops
 * document-level blocks when it does. `crop_box` and `skip_diagonal_text`
 * apply before projection, matching `liteparse_document_parse`. Output-only
 * fields (`page_errors`, page `text`/`markdown`, geometry, and the
 * result-only ranges) are ignored on input.
 *
 * `parser` must be live; `content` and every non-null array or view it
 * names must be readable for the call; `out` must be writable.
 */
LiteParseStatus liteparse_parser_parse_content(const LiteParseParser *parser,
                                               const LiteParseContent *content,
                                               LiteParseResult **out);

/**
 * Open a path, converting non-PDF input once for the document's lifetime.
 *
 * `parser` must be live, `path` readable UTF-8, and `out` writable.
 */
LiteParseStatus liteparse_document_open_path(const LiteParseParser *parser,
                                             LiteParseByteView path,
                                             LiteParseDocument **out);

/**
 * Open and copy in-memory input. Prefer paths for large documents: bytes
 * are copied at open and again per parse.
 *
 * `parser` must be live; `data` must be readable, or null with zero
 * length; `out` must be writable.
 */
LiteParseStatus liteparse_document_open_bytes(const LiteParseParser *parser,
                                              const uint8_t *data,
                                              size_t data_len,
                                              LiteParseDocument **out);

/**
 * Destroy a document handle. Null is allowed.
 */
void liteparse_document_free(LiteParseDocument *document);

/**
 * Borrow the facts recorded at open; null for a null handle.
 *
 * `document` must be null or live.
 */
const LiteParseDocumentInfo *liteparse_document_info(const LiteParseDocument *document);

/**
 * Parse 1-based pages; null with zero length selects all. Selections are
 * validated against the page count, de-duplicated, and processed in
 * ascending order. `max_pages` caps either form.
 *
 * `document` must be live, `pages` readable or null with zero length, and
 * `out` writable.
 */
LiteParseStatus liteparse_document_parse(const LiteParseDocument *document,
                                         const uint32_t *pages,
                                         size_t pages_len,
                                         LiteParseResult **out);

/**
 * Extract pre-projection pages: heuristic text items, graphics, and the
 * configured extras, with no projection, OCR, text, or Markdown. Link
 * stamping and word boxes follow the same rules as parse (links only under
 * Markdown; word boxes when requested or under Markdown). Feed the view's
 * `content` to `liteparse_parser_parse_content` to project and classify.
 *
 * See `liteparse_document_parse`.
 */
LiteParseStatus liteparse_document_extract(const LiteParseDocument *document,
                                           const uint32_t *pages,
                                           size_t pages_len,
                                           LiteParseResult **out);

/**
 * Render selected pages to PNG. Zero DPI uses the configured value. A
 * non-null `region` crops each page; detected rectangles are then clipped
 * and made region-relative. The whole page is rasterized before cropping.
 *
 * `document` must be live; non-null inputs must be readable; `out` writable.
 */
LiteParseStatus liteparse_document_screenshot(const LiteParseDocument *document,
                                              const uint32_t *pages,
                                              size_t pages_len,
                                              float dpi_override,
                                              const LiteParseRenderRegion *region,
                                              LiteParseScreenshots **out);

/**
 * Compute pre-OCR complexity signals for selected pages.
 *
 * See `liteparse_document_parse`.
 */
LiteParseStatus liteparse_document_complexity(const LiteParseDocument *document,
                                              const uint32_t *pages,
                                              size_t pages_len,
                                              LiteParseComplexity **out);

/**
 * Extract heuristic-free PDFium text runs: no gap merge, projection, OCR,
 * or Markdown; every glyph lands in exactly one item.
 *
 * See `liteparse_document_parse`.
 */
LiteParseStatus liteparse_document_raw_text(const LiteParseDocument *document,
                                            const uint32_t *pages,
                                            size_t pages_len,
                                            LiteParseRawText **out);

/**
 * Snapshot unfiltered page content objects: kinds, matrices, PDFium y-up
 * bounds, form children, path segments, and image metadata. No size
 * filters, viewport transforms, or form-matrix composition. `flags` is a
 * mask of `LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_*`; unknown bits are
 * `LITEPARSE_STATUS_INVALID_ARGUMENT`.
 *
 * See `liteparse_document_parse`.
 */
LiteParseStatus liteparse_document_page_objects(const LiteParseDocument *document,
                                                const uint32_t *pages,
                                                size_t pages_len,
                                                uint32_t flags,
                                                LiteParsePageObjects **out);

/**
 * Append recognized words atomically; invalid input appends nothing.
 *
 * `sink` must belong to the current callback; `words` must be readable for
 * `count` entries, or null with zero count.
 */
LiteParseStatus liteparse_ocr_sink_add(LiteParseOcrSink *sink,
                                       const LiteParseOcrWord *words,
                                       size_t count);

/**
 * Set the callback's failure message.
 *
 * `sink` must belong to the current callback and `message` must be readable.
 */
LiteParseStatus liteparse_ocr_sink_set_error(LiteParseOcrSink *sink, LiteParseByteView message);

/**
 * Destroy a page-objects handle. Null is allowed.
 */
void liteparse_page_objects_free(LiteParsePageObjects *page_objects);

/**
 * Borrow the content objects; null for a null handle.
 *
 * `page_objects` must be null or live.
 */
const LiteParsePageObjectsView *liteparse_page_objects_view(const LiteParsePageObjects *page_objects);

/**
 * Create a parser, copying its configuration.
 *
 * `config` and its views must be readable for the call; `out` writable.
 */
LiteParseStatus liteparse_parser_new(const LiteParseConfig *config, LiteParseParser **out);

/**
 * Register (or clear, with a null `recognize`) an in-process OCR engine.
 * `flags` is a mask of `LITEPARSE_OCR_FLAG_*`. Documents opened before a
 * change keep the engine they were opened with.
 *
 * The callback and `user_data` must remain valid and thread-safe while the
 * parser or any document opened from it lives. `name` must be readable.
 */
LiteParseStatus liteparse_parser_set_ocr_callback(const LiteParseParser *parser,
                                                  LiteParseOcrRecognizeFn recognize,
                                                  void *user_data,
                                                  LiteParseByteView name,
                                                  uint32_t flags);

/**
 * Destroy a parser handle. Null is a no-op.
 */
void liteparse_parser_free(LiteParseParser *parser);

/**
 * Destroy a raw-text handle. Null is allowed.
 */
void liteparse_raw_text_free(LiteParseRawText *raw_text);

/**
 * Borrow the extracted runs; null for a null handle.
 *
 * `raw_text` must be null or live.
 */
const LiteParseRawTextView *liteparse_raw_text_view(const LiteParseRawText *raw_text);

/**
 * Destroy a result handle. Null is allowed.
 */
void liteparse_result_free(LiteParseResult *result);

/**
 * Borrow the result view; null for a null handle. Valid until
 * `liteparse_result_free`.
 *
 * `result` must be null or live.
 */
const LiteParseResultView *liteparse_result_view(const LiteParseResult *result);

/**
 * Borrow the cached pretty JSON form of a parse result. Extract results
 * have none.
 *
 * `result` must be live and `out` writable.
 */
LiteParseStatus liteparse_result_to_json(const LiteParseResult *result, LiteParseByteView *out);

/**
 * Find phrase matches on one page as merged text items. `page_index` is a
 * 0-based index into `content.pages`, not a source page number. `flags` is
 * a mask of `LITEPARSE_SEARCH_FLAG_*`. Matches outlive the result.
 *
 * `result` must be live, `phrase` readable UTF-8, and `out` writable.
 */
LiteParseStatus liteparse_result_search(const LiteParseResult *result,
                                        size_t page_index,
                                        LiteParseByteView phrase,
                                        uint32_t flags,
                                        LiteParseSearchMatches **out);

/**
 * Borrow the matches; null for a null handle.
 *
 * `matches` must be null or live.
 */
const LiteParseSearchView *liteparse_search_matches_view(const LiteParseSearchMatches *matches);

/**
 * Destroy a search-match handle. Null is allowed.
 */
void liteparse_search_matches_free(LiteParseSearchMatches *matches);

/**
 * Destroy a screenshots handle. Null is allowed.
 */
void liteparse_screenshots_free(LiteParseScreenshots *screenshots);

/**
 * Borrow the rendered pages; null for a null handle.
 *
 * `screenshots` must be null or live.
 */
const LiteParseScreenshotsView *liteparse_screenshots_view(const LiteParseScreenshots *screenshots);

/**
 * Borrow this thread's most recent failure message. The view stays valid
 * until the next failed call on the same thread.
 */
LiteParseByteView liteparse_last_error(void);

/**
 * Borrow the static binding version string.
 */
LiteParseByteView liteparse_version(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LITEPARSE_H */
