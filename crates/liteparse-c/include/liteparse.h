#ifndef LITEPARSE_H
#define LITEPARSE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "liteparse_abi.h"

/**
 * Capability flags reported by `liteparse_abi_descriptor`.
 */
#define LITEPARSE_CAPABILITY_TESSERACT 1

/**
 * `liteparse_sizeof` selector for `LiteParseByteView`.
 */
#define LITEPARSE_TYPE_BYTE_VIEW 0

/**
 * `liteparse_sizeof` selector for `LiteParseStr`.
 */
#define LITEPARSE_TYPE_STR 1

/**
 * `liteparse_sizeof` selector for `LiteParseRect`.
 */
#define LITEPARSE_TYPE_RECT 2

/**
 * `liteparse_sizeof` selector for `LiteParsePageGeometry`.
 */
#define LITEPARSE_TYPE_PAGE_GEOMETRY 3

/**
 * `liteparse_sizeof` selector for `LiteParsePageComplexity`.
 */
#define LITEPARSE_TYPE_PAGE_COMPLEXITY 4

/**
 * `liteparse_sizeof` selector for `LiteParseTextItem`.
 */
#define LITEPARSE_TYPE_TEXT_ITEM 5

/**
 * `liteparse_sizeof` selector for `LiteParseWordBox`.
 */
#define LITEPARSE_TYPE_WORD_BOX 6

/**
 * `liteparse_sizeof` selector for `LiteParseGraphic`.
 */
#define LITEPARSE_TYPE_GRAPHIC 7

/**
 * `liteparse_sizeof` selector for `LiteParseStructNode`.
 */
#define LITEPARSE_TYPE_STRUCT_NODE 8

/**
 * `liteparse_sizeof` selector for `LiteParseImageRef`.
 */
#define LITEPARSE_TYPE_IMAGE_REF 9

/**
 * `liteparse_sizeof` selector for `LiteParseImage`.
 */
#define LITEPARSE_TYPE_IMAGE 10

/**
 * `liteparse_sizeof` selector for `LiteParseAnnotation`.
 */
#define LITEPARSE_TYPE_ANNOTATION 11

/**
 * `liteparse_sizeof` selector for `LiteParseFormField`.
 */
#define LITEPARSE_TYPE_FORM_FIELD 12

/**
 * `liteparse_sizeof` selector for `LiteParseStructureNode`.
 */
#define LITEPARSE_TYPE_STRUCTURE_NODE 13

/**
 * `liteparse_sizeof` selector for `LiteParseStructureAttribute`.
 */
#define LITEPARSE_TYPE_STRUCTURE_ATTRIBUTE 14

/**
 * `liteparse_sizeof` selector for `LiteParseLayoutBlock`.
 */
#define LITEPARSE_TYPE_LAYOUT_BLOCK 15

/**
 * `liteparse_sizeof` selector for `LiteParseLayoutCell`.
 */
#define LITEPARSE_TYPE_LAYOUT_CELL 16

/**
 * `liteparse_sizeof` selector for `LiteParseLayoutRow`.
 */
#define LITEPARSE_TYPE_LAYOUT_ROW 17

/**
 * `liteparse_sizeof` selector for `LiteParseVectorShape`.
 */
#define LITEPARSE_TYPE_VECTOR_SHAPE 18

/**
 * `liteparse_sizeof` selector for `LiteParseVectorLine`.
 */
#define LITEPARSE_TYPE_VECTOR_LINE 19

/**
 * `liteparse_sizeof` selector for `LiteParseOutlineEntry`.
 */
#define LITEPARSE_TYPE_OUTLINE_ENTRY 20

/**
 * `liteparse_sizeof` selector for `LiteParsePageError`.
 */
#define LITEPARSE_TYPE_PAGE_ERROR 21

/**
 * `liteparse_sizeof` selector for `LiteParseXfaPacket`.
 */
#define LITEPARSE_TYPE_XFA_PACKET 22

/**
 * `liteparse_sizeof` selector for `LiteParseDocumentMeta`.
 */
#define LITEPARSE_TYPE_DOCUMENT_META 23

/**
 * `liteparse_sizeof` selector for `LiteParseScreenshot`.
 */
#define LITEPARSE_TYPE_SCREENSHOT 24

/**
 * `liteparse_sizeof` selector for `LiteParseScreenshotRect`.
 */
#define LITEPARSE_TYPE_SCREENSHOT_RECT 25

/**
 * `liteparse_sizeof` selector for `LiteParseProjectedLine`.
 */
#define LITEPARSE_TYPE_PROJECTED_LINE 26

/**
 * `liteparse_sizeof` selector for `LiteParseProjectedRegion`.
 */
#define LITEPARSE_TYPE_PROJECTED_REGION 27

/**
 * `liteparse_sizeof` selector for `LiteParseItemFrame`.
 */
#define LITEPARSE_TYPE_ITEM_FRAME 28

/**
 * `liteparse_sizeof` selector for `LiteParsePage`.
 */
#define LITEPARSE_TYPE_PAGE 29

/**
 * `liteparse_sizeof` selector for `LiteParseContentInput`.
 */
#define LITEPARSE_TYPE_CONTENT_INPUT 30

/**
 * `liteparse_sizeof` selector for `LiteParseResultView`.
 */
#define LITEPARSE_TYPE_RESULT_VIEW 31

/**
 * `liteparse_sizeof` selector for `LiteParseArenas`.
 */
#define LITEPARSE_TYPE_ARENAS 32

/**
 * `liteparse_sizeof` selector for `LiteParseScreenshotsView`.
 */
#define LITEPARSE_TYPE_SCREENSHOTS_VIEW 33

/**
 * `liteparse_sizeof` selector for `LiteParseComplexityView`.
 */
#define LITEPARSE_TYPE_COMPLEXITY_VIEW 34

/**
 * `liteparse_sizeof` selector for `LiteParseDocumentInfo`.
 */
#define LITEPARSE_TYPE_DOCUMENT_INFO 35

/**
 * `liteparse_sizeof` selector for `LiteParseRenderRegion`.
 */
#define LITEPARSE_TYPE_RENDER_REGION 36

/**
 * `liteparse_sizeof` selector for `LiteParseConfig`.
 */
#define LITEPARSE_TYPE_CONFIG 37

/**
 * `liteparse_sizeof` selector for `LiteParseHeader`.
 */
#define LITEPARSE_TYPE_HEADER 38

/**
 * `liteparse_sizeof` selector for `LiteParsePageOrientationCorrection`.
 */
#define LITEPARSE_TYPE_PAGE_ORIENTATION_CORRECTION 39

/**
 * `liteparse_sizeof` selector for `LiteParseOcrRaster`.
 */
#define LITEPARSE_TYPE_OCR_RASTER 40

/**
 * `liteparse_sizeof` selector for `LiteParseOcrWord`.
 */
#define LITEPARSE_TYPE_OCR_WORD 41

/**
 * `liteparse_sizeof` selector for `LiteParsePageObjectPage`.
 */
#define LITEPARSE_TYPE_PAGE_OBJECT_PAGE 42

/**
 * `liteparse_sizeof` selector for `LiteParseMatrix`.
 */
#define LITEPARSE_TYPE_MATRIX 43

/**
 * `liteparse_sizeof` selector for `LiteParsePdfBounds`.
 */
#define LITEPARSE_TYPE_PDF_BOUNDS 44

/**
 * `liteparse_sizeof` selector for `LiteParsePageObject`.
 */
#define LITEPARSE_TYPE_PAGE_OBJECT 45

/**
 * `liteparse_sizeof` selector for `LiteParsePathSegment`.
 */
#define LITEPARSE_TYPE_PATH_SEGMENT 46

/**
 * `liteparse_sizeof` selector for `LiteParsePageObjectsView`.
 */
#define LITEPARSE_TYPE_PAGE_OBJECTS_VIEW 47

/**
 * `liteparse_sizeof` selector for `LiteParseRawTextPage`.
 */
#define LITEPARSE_TYPE_RAW_TEXT_PAGE 48

/**
 * `liteparse_sizeof` selector for `LiteParseRawTextItem`.
 */
#define LITEPARSE_TYPE_RAW_TEXT_ITEM 49

/**
 * `liteparse_sizeof` selector for `LiteParseRawTextView`.
 */
#define LITEPARSE_TYPE_RAW_TEXT_VIEW 50

/**
 * `liteparse_sizeof` selector for `LiteParseContentArrays`.
 */
#define LITEPARSE_TYPE_CONTENT_ARRAYS 51

/**
 * `liteparse_sizeof` selector for `LiteParsePageOutput`.
 */
#define LITEPARSE_TYPE_PAGE_OUTPUT 52

/**
 * `liteparse_sizeof` selector for `LiteParseBlobRef`.
 */
#define LITEPARSE_TYPE_BLOB_REF 53

/**
 * `liteparse_sizeof` selector for `LiteParseAbiDescriptor`.
 */
#define LITEPARSE_TYPE_ABI_DESCRIPTOR 54

/**
 * `liteparse_sizeof` selector for `LiteParseOcrPageInput`.
 */
#define LITEPARSE_TYPE_OCR_PAGE_INPUT 55

/**
 * `liteparse_sizeof` selector for `LiteParseOcrRasterView`.
 */
#define LITEPARSE_TYPE_OCR_RASTER_VIEW 56

/**
 * Maximum sum of flat input array bytes accepted from caller-supplied content.
 */
#define LITEPARSE_MAX_CONTENT_INPUT_BYTES ((256 * 1024) * 1024)

/**
 * Maximum size of a single caller-supplied binary payload.
 */
#define LITEPARSE_MAX_SINGLE_BINARY_BYTES ((32 * 1024) * 1024)

/**
 * Maximum sum of caller-supplied image bytes in one content input.
 */
#define LITEPARSE_MAX_CONTENT_BINARY_BYTES ((128 * 1024) * 1024)

/**
 * Maximum bytes of one result's arrays and arenas. Every offset, count, and
 * arena length of a published result therefore fits in `int32_t`.
 */
#define LITEPARSE_MAX_RESULT_BYTES ((1024 * 1024) * 1024)

/**
 * Values for `LiteParseConfig.output_format`.
 */
#define LITEPARSE_OUTPUT_FORMAT_TEXT 0

#define LITEPARSE_OUTPUT_FORMAT_MARKDOWN 1

/**
 * Values for `LiteParseConfig.image_mode`.
 */
#define LITEPARSE_IMAGE_MODE_OFF 0

#define LITEPARSE_IMAGE_MODE_PLACEHOLDER 1

#define LITEPARSE_IMAGE_MODE_EMBED 2

/**
 * Default resolution in DPI for raster rendering and screenshots.
 */
#define LITEPARSE_DEFAULT_DPI 150.0

/**
 * `LiteParseConfig.options` bit: apply `crop_box`. Presence bits count down
 * from bit 63; boolean flags count up from bit 0.
 */
#define LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX (1ull << 63)

/**
 * `LiteParseConfig.options` bits, whose presence denotes true.
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

#define LITEPARSE_FLAG_RENDER_FORM_FIELDS (1ull << 19)

#define LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT (1ull << 20)

#define LITEPARSE_FLAG_EXTRACT_SCREENSHOTS (1ull << 21)

/**
 * `LiteParseDocumentInfo.flags` bit: the source was converted to PDF (an
 * office document or image).
 */
#define LITEPARSE_DOCUMENT_FLAG_CONVERTED (1 << 0)

/**
 * Maximum number of explicit page numbers accepted in one operation.
 */
#define LITEPARSE_MAX_SELECTED_PAGES 100000

/**
 * `LiteParseOcrRaster.pixel_format` values and `liteparse_job_render_ocr`
 * pixel formats.
 */
#define LITEPARSE_OCR_PIXEL_FORMAT_RGB 0

#define LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE 1

/**
 * `LiteParseOcrRaster.flags` bits.
 * The page has real native text; OCR enriches it rather than replacing it.
 */
#define LITEPARSE_OCR_RASTER_FLAG_HAS_NATIVE_TEXT (1 << 0)

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
 * PDFium returned no raw image data (an empty stream or extraction failure).
 */
#define LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_RAW_UNAVAILABLE (1 << 9)

/**
 * PDFium returned no decoded image data (an empty stream or extraction failure).
 */
#define LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_DECODED_UNAVAILABLE (1 << 10)

/**
 * PDFium did not return a requested image bitmap.
 */
#define LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_BITMAP_UNAVAILABLE (1 << 11)

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
 * Maximum number of Form XObject levels, counting a top-level form as one.
 */
#define LITEPARSE_MAX_OBJECT_NESTING_DEPTH 32

/**
 * Maximum number of page objects retained in one operation.
 */
#define LITEPARSE_MAX_OBJECTS 100000

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
 * Maximum pixels in one planned screenshot bitmap: the region alone for a
 * region render without rectangle detection, the whole page otherwise.
 */
#define LITEPARSE_MAX_RASTER_PIXELS_PER_PAGE ((64 * 1024) * 1024)

/**
 * Maximum sum of planned raster bytes in one operation: the RGBA bitmaps
 * of a screenshot call (region-sized for a plain region render), or the
 * rasters of one OCR round.
 */
#define LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION ((256 * 1024) * 1024)

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
 * Per-page complexity signals. Views borrow from the handle.
 */
typedef struct LiteParseComplexity LiteParseComplexity;

/**
 * A document opened once for many operations. Operations may run
 * concurrently on one handle; destruction must wait for them.
 */
typedef struct LiteParseDocument LiteParseDocument;

/**
 * A parse in progress whose OCR the host performs between calls. Holds no
 * PDFium resource and may outlive its document.
 */
typedef struct LiteParseJob LiteParseJob;

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
 * Runtime layout and compiled-feature descriptor. `layout_fingerprint`
 * equals `LITEPARSE_ABI_FINGERPRINT` from the header the library was built
 * with.
 */
typedef struct {
  uint32_t pointer_width;
  /**
   * 0 for little endian, 1 for big endian.
   */
  uint32_t endianness;
  uint32_t capabilities;
  uint64_t layout_fingerprint;
  uint32_t max_selected_pages;
  uint64_t max_content_input_bytes;
  uint64_t max_single_binary_bytes;
  uint64_t max_content_binary_bytes;
  uint64_t max_result_bytes;
  uint64_t max_raster_pixels_per_page;
  uint64_t max_raster_bytes_per_operation;
  uint32_t max_objects;
  uint32_t max_object_nesting_depth;
} LiteParseAbiDescriptor;

/**
 * A UTF-8 string stored in the owning view's `pool`: bytes
 * `pool[offset .. offset + len]`, followed by a NUL byte that `len` does not
 * count. Absent and empty strings are both `len == 0`; the pool starts with
 * a NUL so `{0, 0}` also reads as an empty C string. Records hold no
 * pointers, so a view's arrays and pool can be copied out of the handle and
 * read from anywhere.
 */
typedef struct {
  uint32_t offset;
  uint32_t len;
} LiteParseStr;

typedef struct {
  LiteParseStr message;
  uint32_t page_number;
} LiteParsePageError;

/**
 * The string arena, binary arena, and page failures that lead every result
 * view. Strings index `pool` and binary references index `binary`.
 */
typedef struct {
  const uint8_t *pool;
  size_t pool_len;
  const uint8_t *binary;
  size_t binary_len;
  /**
   * Page failures recorded under tolerant processing.
   */
  const LiteParsePageError *page_errors;
  size_t page_errors_len;
} LiteParseArenas;

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
  LiteParseArenas arenas;
  const LiteParsePageComplexity *pages;
  size_t pages_len;
} LiteParseComplexityView;

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
  uint32_t struct_size;
  /**
   * Actual boolean values (`LITEPARSE_FLAG_*`) and crop-box presence.
   */
  uint64_t options;
  /**
   * Zero parses no pages.
   */
  uint32_t max_pages;
  uint32_t num_workers;
  uint32_t output_format;
  uint32_t image_mode;
  /**
   * Must be finite and greater than zero.
   */
  float dpi;
  /**
   * Normalized fractions ordered top, right, bottom, left, applied when
   * `LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX` is set in `options`. Every value must lie in
   * `[0, 1]` with `top + bottom < 1` and `left + right < 1`.
   */
  float crop_box[4];
  LiteParseByteView ocr_language;
  LiteParseByteView ocr_server_url;
  LiteParseByteView tessdata_path;
  LiteParseByteView password;
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
 * One pre-projection page.
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
   * The PDF `/PageLabels` entry, or empty.
   */
  LiteParseStr label;
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
} LiteParsePage;

/**
 * One text item. Used for page items, projected spans, and
 * caller-supplied content. Ranges index the owning view's `words` and
 * `char_codes` arrays.
 */
typedef struct {
  LiteParseStr text;
  LiteParseStr font_name;
  LiteParseStr link;
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
  LiteParseStr text;
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
  LiteParseStr role;
  LiteParseStr alt_text;
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
  LiteParseStr id;
  LiteParseStr format;
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
 * Offset and byte length into the owning view's binary arena.
 */
typedef struct {
  uint32_t offset;
  uint32_t len;
} LiteParseBlobRef;

/**
 * Extracted image with a binary-arena reference.
 */
typedef struct {
  LiteParseStr id;
  LiteParseStr name;
  LiteParseStr path;
  LiteParseStr format;
  LiteParseStr duplicate_of;
  LiteParseBlobRef bytes;
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
  LiteParseStr subtype;
  LiteParseStr contents;
  LiteParseStr created;
  LiteParseStr modified;
  LiteParseStr title;
  LiteParseStr uri;
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
  LiteParseStr id;
  LiteParseStr field_type;
  LiteParseStr name;
  LiteParseStr alternate_name;
  LiteParseStr value;
  LiteParseStr export_value;
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
  LiteParseStr element_type;
  LiteParseStr id;
  LiteParseStr actual_text;
  LiteParseStr alt_text;
  LiteParseStr title;
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
  LiteParseStr name;
  LiteParseStr string;
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
  LiteParseStr text;
  LiteParseStr marker;
  LiteParseStr lang;
  /**
   * Figure image id and encoded format.
   */
  LiteParseStr id;
  LiteParseStr format;
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
  LiteParseStr text;
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
  LiteParseStr title;
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

/**
 * Packed page content shared by caller input and result views.
 *
 * Pages carry offset/count ranges into the flat arrays. `arenas.pool` is
 * the UTF-8 string pool every `LiteParseStr` indexes; `arenas.binary` is the
 * arena every `LiteParseBlobRef` indexes. `strings` holds form-field options and block
 * source lines; `annotations` holds page and structure-node annotations;
 * `mcids` holds struct-node and structure-tree marked-content ids; `words`
 * and `char_codes` are shared by every text item.
 * `document_block_offset/count` selects document-wide blocks.
 *
 * Library-owned pools start with a NUL byte and store a NUL after every
 * string, outside its recorded length; caller pools need neither. A result's
 * `content` is valid `LiteParseContentInput.content`.
 */
typedef struct {
  LiteParseArenas arenas;
  /**
   * Source document page count. Every page number, outline target, and
   * page error must lie within it.
   */
  uint32_t total_pages;
  uint32_t document_block_offset;
  uint32_t document_block_count;
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
  const LiteParseStr *strings;
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
} LiteParseContentArrays;

/**
 * Caller-owned content for `liteparse_parser_parse_content`.
 */
typedef struct {
  /**
   * Must equal `sizeof(LiteParseContentInput)`.
   */
  uint32_t struct_size;
  LiteParseContentArrays content;
} LiteParseContentInput;

/**
 * Fixed-width status code returned by fallible API functions.
 */
typedef uint32_t LiteParseStatus;

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
   * String pool behind the outline titles.
   */
  const uint8_t *pool;
  size_t pool_len;
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
 * One page rendered for OCR. `pixels` indexes the view's binary arena:
 * tightly packed rows of 3 bytes per pixel for RGB or 1 for grayscale.
 * `image_rect_offset/count` index the view's `image_rects`, embedded
 * figures in viewport points.
 */
typedef struct {
  LiteParseBlobRef pixels;
  uint32_t page_number;
  uint32_t width;
  uint32_t height;
  /**
   * `LITEPARSE_OCR_PIXEL_FORMAT_*`.
   */
  uint32_t pixel_format;
  /**
   * Effective render DPI; raster pixels map to points at `72 / dpi`.
   */
  float dpi;
  /**
   * `LITEPARSE_OCR_RASTER_FLAG_*` bits.
   */
  uint32_t flags;
  uint32_t image_rect_offset;
  uint32_t image_rect_count;
} LiteParseOcrRaster;

/**
 * One OCR round: rasters with their pixels in `arenas.binary`, embedded
 * image rectangles, and the round's render failures in
 * `arenas.page_errors`. Borrowed from the job until its next call.
 */
typedef struct {
  LiteParseArenas arenas;
  const LiteParseOcrRaster *rasters;
  size_t rasters_len;
  const LiteParseRect *image_rects;
  size_t image_rects_len;
} LiteParseOcrRasterView;

/**
 * Recognition outcome for one rendered page. `word_offset/count` index
 * the merge's `words`. A non-empty `error` records a recognition failure
 * and requires an empty word range.
 */
typedef struct {
  uint32_t page_number;
  uint32_t word_offset;
  uint32_t word_count;
  LiteParseStr error;
} LiteParseOcrPageInput;

/**
 * One recognized word in raster pixels. `text` indexes the merge's pool.
 * Box edges are finite with positive width and height; confidence is
 * finite and in [0, 1]. With `HAS_POLYGON`, `polygon` holds four finite
 * x/y corners in reading order, forming a nondegenerate,
 * non-self-intersecting quadrilateral within the raster; otherwise it is
 * ignored.
 */
typedef struct {
  LiteParseStr text;
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
  LiteParseStr label;
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
 * empty when not requested, empty, or unavailable; the `*_UNAVAILABLE`
 * flags mark requested payloads PDFium did not return.
 */
typedef struct {
  LiteParseBlobRef image_raw;
  LiteParseBlobRef image_decoded;
  LiteParseBlobRef image_bitmap;
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
  LiteParseArenas arenas;
  const LiteParsePageObjectPage *pages;
  size_t pages_len;
  const LiteParsePageObject *objects;
  size_t objects_len;
  const LiteParsePathSegment *segments;
  size_t segments_len;
  const LiteParseStr *filters;
  size_t filters_len;
} LiteParsePageObjectsView;

/**
 * One extracted page. `item_offset/count` index the view's `items`.
 */
typedef struct {
  LiteParseStr label;
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
  LiteParseStr text;
  LiteParseStr font_name;
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
  LiteParseArenas arenas;
  const LiteParseRawTextPage *pages;
  size_t pages_len;
  const LiteParseRawTextItem *items;
  size_t items_len;
  const uint32_t *char_codes;
  size_t char_codes_len;
  const LiteParseStr *glyph_names;
  size_t glyph_names_len;
} LiteParseRawTextView;

/**
 * Projection output for the page at the same index in `content.pages`.
 */
typedef struct {
  LiteParseStr text;
  LiteParseStr markdown;
  uint32_t figure_offset;
  uint32_t figure_count;
  uint32_t item_frame_offset;
  uint32_t item_frame_count;
  uint32_t projected_line_offset;
  uint32_t projected_line_count;
  uint32_t region_offset;
  uint32_t region_count;
} LiteParsePageOutput;

/**
 * Document metadata. Absent strings are empty; scalars use flags.
 */
typedef struct {
  /**
   * Authored `/Info` values read at document open.
   */
  LiteParseStr title;
  LiteParseStr author;
  LiteParseStr subject;
  LiteParseStr keywords;
  LiteParseStr trapped;
  LiteParseStr creation_date;
  LiteParseStr mod_date;
  LiteParseStr xmp;
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
  LiteParseBlobRef png;
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
  LiteParseStr name;
  LiteParseStr content;
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
  LiteParseStr text;
  LiteParseStr dominant_font_name;
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
 * Everything a result exposes. `content` is the page-content model accepted
 * by `liteparse_parser_parse_content`; the remaining arrays are result only.
 * Projected spans share `content.words` and `content.char_codes`; every
 * `LiteParseStr` in the view indexes `content.arenas.pool` and every
 * `LiteParseBlobRef` indexes `content.arenas.binary`.
 */
typedef struct {
  LiteParseContentArrays content;
  /**
   * Parallel to `content.pages` for parse results; empty for extract-only
   * results.
   */
  const LiteParsePageOutput *page_outputs;
  size_t page_outputs_len;
  /**
   * Full-document plain text or Markdown, per the output format.
   */
  LiteParseStr text;
  LiteParseStr creator;
  LiteParseStr producer;
  LiteParseDocumentMeta doc_meta;
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
 * Rendered pages and the solid rectangles detected on them.
 */
typedef struct {
  LiteParseArenas arenas;
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
 * The asynchronous runtime could not be initialized.
 */
#define LITEPARSE_STATUS_RUNTIME_ERROR 4

/**
 * The document is encrypted and the configured password did not open it.
 */
#define LITEPARSE_STATUS_PASSWORD_REQUIRED 5

/**
 * A non-PDF source could not be converted: an unsupported extension, or a
 * missing external converter (LibreOffice).
 */
#define LITEPARSE_STATUS_CONVERSION_ERROR 6

/**
 * OCR was required and could not be performed.
 */
#define LITEPARSE_STATUS_OCR_ERROR 7

/**
 * The source could not be read from the filesystem.
 */
#define LITEPARSE_STATUS_IO_ERROR 8

/**
 * A required runtime dependency is unavailable.
 */
#define LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE 9

/**
 * An operation exceeded an enforced resource limit.
 */
#define LITEPARSE_STATUS_RESOURCE_LIMIT 10

/**
 * A Rust panic was caught before it crossed the C ABI boundary. Free any
 * handle involved and do not reuse it.
 */
#define LITEPARSE_STATUS_PANIC 255

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Write the library's ABI descriptor. Null is a no-op.
 */
void liteparse_abi_descriptor(LiteParseAbiDescriptor *out);

/**
 * `sizeof` the struct a `LITEPARSE_TYPE_*` selector names, or zero for an
 * unknown selector. Lets a binding that declares layouts by hand assert
 * them against the library it loaded.
 */
size_t liteparse_sizeof(uint32_t type_id);

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
void liteparse_content_init(LiteParseContentInput *content);

/**
 * Parse caller-supplied pages without opening a document. Every view and
 * array is copied during the call.
 *
 * With no block ranges the pages run grid projection (and the configured
 * classifier). Any per-page or document-level block range makes those
 * blocks the Markdown structure, so content taken from a parse result with
 * extracted blocks keeps those blocks; `images` and per-page complexity are
 * forwarded on that path. `max_pages` truncates the page list and drops
 * document-level blocks when it does. `crop_box` and `skip_diagonal_text`
 * apply before projection, matching `liteparse_document_parse`.
 * Source page count, outline, page geometry, images, complexity, and
 * page errors are retained in the projected result.
 *
 * `parser` must be live; `content` and every non-null array or view it
 * names must be readable for the call; `out` must be writable.
 */
LiteParseStatus liteparse_parser_parse_content(const LiteParseParser *parser,
                                               const LiteParseContentInput *content,
                                               LiteParseResult **out);

/**
 * Open a path, converting non-PDF input once for the document's lifetime.
 * `path` is `path_len` bytes of UTF-8, not NUL-terminated.
 *
 * `parser` must be live, `path` readable for `path_len` bytes, and `out`
 * writable.
 */
LiteParseStatus liteparse_document_open_path(const LiteParseParser *parser,
                                             const uint8_t *path,
                                             size_t path_len,
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
 * Markdown; word boxes when requested or under Markdown). The view's
 * `content` is valid `LiteParseContentInput.content` for
 * `liteparse_parser_parse_content` to project and classify.
 *
 * See `liteparse_document_parse`.
 */
LiteParseStatus liteparse_document_extract(const LiteParseDocument *document,
                                           const uint32_t *pages,
                                           size_t pages_len,
                                           LiteParseResult **out);

/**
 * Render selected pages to PNG. Zero DPI uses the configured value. A
 * non-null `region` renders only that part of each page, at the size and
 * scale of the matching crop of a whole-page render; glyph anti-aliasing
 * can differ slightly from the crop. Rectangle detection reads the whole
 * page, so with it the page is rendered in full and detected rectangles
 * are clipped and made region-relative.
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
 * Start a parse whose OCR the host performs. Runs extraction and configured
 * pre-OCR work for the same page selection, then releases PDFium. Projection
 * and classification happen in `liteparse_job_finish`, after OCR. The job
 * never runs the configured OCR engine, and it may outlive `document`.
 *
 * `document` must be live, `pages` readable or null with zero length, and
 * `out` writable.
 */
LiteParseStatus liteparse_document_begin(const LiteParseDocument *document,
                                         const uint32_t *pages,
                                         size_t pages_len,
                                         LiteParseJob **out);

/**
 * Render the next OCR round: at most `max_rasters` pages (zero uses the
 * configured `num_workers`), fewer when the round would exceed
 * `LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION`, in
 * `LITEPARSE_OCR_PIXEL_FORMAT_*` pixels. Rounds move forward through the
 * job's pages. The core OCR predicates choose the pages whatever the
 * configured OCR engine; a non-empty `pages` list names job pages and
 * renders only those not yet passed. Repeat the same list on every call
 * until an empty round, which means OCR is complete. The view is valid
 * until the next call on the job, and rasters left unmerged when the next
 * round renders receive no OCR. On failure, including `LITEPARSE_STATUS_RESOURCE_LIMIT`, the job
 * is unchanged.
 *
 * `job` must be live and exclusively owned by the caller for the call,
 * `pages` readable or null with zero length, and `out` writable.
 */
LiteParseStatus liteparse_job_render_ocr(LiteParseJob *job,
                                         const uint32_t *pages,
                                         size_t pages_len,
                                         uint32_t max_rasters,
                                         uint32_t pixel_format,
                                         const LiteParseOcrRasterView **out);

/**
 * Merge recognition for the rendered round: one `LiteParseOcrPageInput`
 * per recognized page, with `LiteParseOcrWord` records in raster pixels
 * whose text indexes `pool`. A rendered page without an input recognized
 * nothing. Every input is validated before the job changes. Recognition
 * errors become page errors; with `LITEPARSE_FLAG_OCR_FAILURE_FATAL` the
 * core merge fails with `LITEPARSE_STATUS_OCR_ERROR` when every page in
 * the round failed and one had sparse native text, after which the job
 * accepts only `liteparse_job_free`.
 *
 * `job` must be live and exclusively owned by the caller for the call;
 * each array must be readable for its length, or null with zero length.
 */
LiteParseStatus liteparse_job_merge_ocr(LiteParseJob *job,
                                        const LiteParseOcrPageInput *page_inputs,
                                        size_t page_inputs_len,
                                        const LiteParseOcrWord *words,
                                        size_t words_len,
                                        const uint8_t *pool,
                                        size_t pool_len);

/**
 * Consume the job: filter, project, classify, and render its pages into
 * the result `liteparse_document_parse` produces. A rendered round left
 * unmerged receives no OCR. `*job` is freed and set to null whether or not
 * the call succeeds.
 *
 * `job` must point to a live job handle or null; `out` must be writable.
 */
LiteParseStatus liteparse_job_finish(LiteParseJob **job, LiteParseResult **out);

/**
 * Destroy a job. Null is allowed.
 */
void liteparse_job_free(LiteParseJob *job);

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
 * Borrow this thread's most recent failure message into `out`. The view
 * stays valid until the next failed call on the same thread; read it on
 * the thread that made the failing call, before any other call that may
 * fail. Null `out` is a no-op.
 */
void liteparse_last_error(LiteParseByteView *out);

/**
 * Borrow the static binding version string into `out`. Null is a no-op.
 */
void liteparse_version(LiteParseByteView *out);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LITEPARSE_H */
