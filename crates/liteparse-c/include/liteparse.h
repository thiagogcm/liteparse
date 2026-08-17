#ifndef LITEPARSE_H
#define LITEPARSE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/**
 * Values accepted by `liteparse_options_set_output_format`.
 */
#define LITEPARSE_OUTPUT_FORMAT_JSON 0

#define LITEPARSE_OUTPUT_FORMAT_TEXT 1

#define LITEPARSE_OUTPUT_FORMAT_MARKDOWN 2

/**
 * Values accepted by `liteparse_options_set_image_mode`.
 */
#define LITEPARSE_IMAGE_MODE_OFF 0

#define LITEPARSE_IMAGE_MODE_PLACEHOLDER 1

#define LITEPARSE_IMAGE_MODE_EMBED 2

/**
 * Pixel formats handed to a `LiteParseOcrRecognizeFn`. RGB is tightly packed
 * 3 bytes per pixel; grayscale is 1 byte per pixel.
 */
#define LITEPARSE_OCR_PIXEL_FORMAT_RGB 0

#define LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE 1

/**
 * Opaque per-page complexity report handle. Create it with
 * `liteparse_is_complex_path` or `liteparse_is_complex_bytes` and destroy it
 * with `liteparse_complexity_free`.
 */
typedef struct LiteParseComplexity LiteParseComplexity;

/**
 * Opaque OCR result sink passed to a `LiteParseOcrRecognizeFn`; valid only
 * during that invocation and never retained.
 */
typedef struct LiteParseOcrSink LiteParseOcrSink;

/**
 * Opaque options handle. Create it with `liteparse_options_new` and destroy
 * it with `liteparse_options_free`.
 */
typedef struct LiteParseOptions LiteParseOptions;

/**
 * Opaque parser handle. Create it with `liteparse_parser_new_with_options`
 * and destroy it with `liteparse_parser_free`.
 */
typedef struct LiteParseParser LiteParseParser;

/**
 * Opaque typed parse-result handle. Create it with one of the
 * `liteparse_parse_*_result` functions and destroy it with
 * `liteparse_result_free`.
 */
typedef struct LiteParseResult LiteParseResult;

/**
 * Opaque page-screenshot collection handle. Create it with
 * `liteparse_screenshot_path` or `liteparse_screenshot_bytes` and destroy it
 * with `liteparse_screenshots_free`.
 */
typedef struct LiteParseScreenshots LiteParseScreenshots;

/**
 * Opaque phrase-search match collection handle. Create it with
 * `liteparse_result_search` and destroy it with
 * `liteparse_search_matches_free`.
 */
typedef struct LiteParseSearchMatches LiteParseSearchMatches;

/**
 * Opaque bounded-memory batch-parsing session handle. Create it with
 * `liteparse_session_open_path` or `liteparse_session_open_bytes` and destroy
 * it with `liteparse_session_free`.
 */
typedef struct LiteParseSession LiteParseSession;

/**
 * Fixed-width status code returned by every fallible C API function.
 */
typedef uint32_t LiteParseStatus;

/**
 * A custom in-process OCR engine, called once per rendered page raster.
 *
 * `pixels` holds `pixels_len` bytes of tightly packed image data in the
 * given `pixel_format` (`width * height` bytes for grayscale, `* 3` for
 * RGB); `language` and `dpi` come from the parser configuration. Push
 * word-level results into `sink` with `liteparse_ocr_sink_add` and return
 * zero. A non-zero return fails the page's OCR with the message set via
 * `liteparse_ocr_sink_set_error`.
 *
 * The callback must not unwind, must not retain any argument past the call,
 * and must be thread-safe: parses can invoke it from several threads at
 * once. Null is a valid value and clears a registration.
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
 * Borrowed bytes returned by typed result getters. The pointer remains valid
 * until the owning `LiteParseResult` is freed. Bytes are UTF-8 where the
 * getter says so, but are not NUL-terminated.
 */
typedef struct {
  const uint8_t *ptr;
  size_t len;
} LiteParseByteView;

/**
 * A text item view. All string fields are borrowed from the result handle.
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
  bool has_font_size;
  float confidence;
  bool has_confidence;
} LiteParseTextItem;

/**
 * One word's bounding box within a text item, in the same top-left-origin
 * 72-DPI viewport space as the parent item. `text` is borrowed from the
 * owning `LiteParseResult` and excludes inter-word spaces.
 */
typedef struct {
  LiteParseByteView text;
  float x;
  float y;
  float width;
  float height;
} LiteParseWordBox;

/**
 * An extracted embedded image view. Image bytes and string fields are
 * borrowed from the result handle.
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
  float bbox_x;
  float bbox_y;
  float bbox_width;
  float bbox_height;
  LiteParseByteView bytes;
} LiteParseImage;

/**
 * One rendered page screenshot. PNG bytes are borrowed from the owning
 * `LiteParseScreenshots` handle.
 */
typedef struct {
  /**
   * 1-based source page number.
   */
  uint32_t page_number;
  /**
   * Rendered bitmap width in pixels.
   */
  uint32_t width;
  /**
   * Rendered bitmap height in pixels.
   */
  uint32_t height;
  /**
   * True when every pixel has the same color (blank page after render).
   */
  bool is_solid_fill;
  /**
   * PNG-encoded image bytes.
   */
  LiteParseByteView png;
  /**
   * Number of rects readable via `liteparse_screenshots_get_rect`.
   * Non-zero only when `detect_screenshot_rects` was enabled.
   */
  size_t rect_count;
} LiteParseScreenshot;

/**
 * One solid rectangle (or thick line) detected in a rendered page bitmap.
 * Coordinates are in top-left-origin 72-DPI viewport space. The color is a
 * borrowed UTF-8 ARGB hex string (e.g. "ff1a2b3c").
 */
typedef struct {
  float x;
  float y;
  float width;
  float height;
  /**
   * True when only one dimension reaches the minimum rectangle size
   * (a solid line rather than a filled area).
   */
  bool is_line;
  LiteParseByteView color;
} LiteParseScreenshotRect;

/**
 * Flat per-page complexity signals. The flag reasons and layout signals are
 * available through `liteparse_complexity_to_json`.
 */
typedef struct {
  /**
   * 1-based source page number.
   */
  size_t page_number;
  /**
   * Usable native text length in bytes.
   */
  size_t text_length;
  /**
   * Fraction of the page area covered by native text.
   */
  float text_coverage;
  /**
   * Number of raster image objects counted on the page.
   */
  size_t image_block_count;
  /**
   * Summed image-bbox area over the page area, clamped to 1.0.
   */
  float image_coverage;
  /**
   * Area of the single largest counted image over the page area.
   */
  float largest_image_coverage;
  bool has_substantial_images;
  /**
   * A single raster covering >= 90% of the page is present (scan signal).
   */
  bool full_page_image;
  bool is_garbled;
  /**
   * Whether the page needs more than the cheap text-only path.
   */
  bool needs_ocr;
  /**
   * Page area in pt^2.
   */
  float page_area;
  /**
   * Filled vector-outline area not covered by native text, in pt^2.
   * Meaningful only when `has_uncovered_vector_area` is true.
   */
  float uncovered_vector_area;
  bool has_uncovered_vector_area;
} LiteParsePageComplexity;

/**
 * The operation completed successfully.
 */
#define LITEPARSE_STATUS_OK 0

/**
 * A required pointer was null or an input string was not valid UTF-8.
 */
#define LITEPARSE_STATUS_INVALID_ARGUMENT 1

/**
 * An options value was invalid or requested an unsupported C feature.
 */
#define LITEPARSE_STATUS_INVALID_CONFIG 2

/**
 * LiteParse could not open, convert, or parse the document.
 */
#define LITEPARSE_STATUS_PARSE_ERROR 3

/**
 * The parse result could not be serialized to JSON.
 */
#define LITEPARSE_STATUS_SERIALIZATION_ERROR 4

/**
 * The binding could not initialize its asynchronous runtime.
 */
#define LITEPARSE_STATUS_RUNTIME_ERROR 5

/**
 * An unwinding Rust panic was caught before it crossed the C ABI boundary.
 * After a parser operation returns this status, free the handle without
 * reusing it. Process-aborting panics and allocation failures are not caught.
 */
#define LITEPARSE_STATUS_PANIC 255

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Create options initialized with LiteParse defaults.
 */
LiteParseStatus liteparse_options_new(LiteParseOptions **out_options, char **out_error);

/**
 * Destroy an options handle. A null pointer is accepted as a no-op.
 */
void liteparse_options_free(LiteParseOptions *options);

LiteParseStatus liteparse_options_set_continue_on_page_error(LiteParseOptions *options,
                                                             bool value,
                                                             char **out_error);

LiteParseStatus liteparse_options_set_detect_screenshot_rects(LiteParseOptions *options,
                                                              bool value,
                                                              char **out_error);

LiteParseStatus liteparse_options_set_emit_word_boxes(LiteParseOptions *options,
                                                      bool value,
                                                      char **out_error);

LiteParseStatus liteparse_options_set_extract_annotations(LiteParseOptions *options,
                                                          bool value,
                                                          char **out_error);

LiteParseStatus liteparse_options_set_extract_blocks(LiteParseOptions *options,
                                                     bool value,
                                                     char **out_error);

LiteParseStatus liteparse_options_set_extract_content_bounds(LiteParseOptions *options,
                                                             bool value,
                                                             char **out_error);

LiteParseStatus liteparse_options_set_extract_document_metadata(LiteParseOptions *options,
                                                                bool value,
                                                                char **out_error);

LiteParseStatus liteparse_options_set_extract_form_fields(LiteParseOptions *options,
                                                          bool value,
                                                          char **out_error);

LiteParseStatus liteparse_options_set_extract_images(LiteParseOptions *options,
                                                     bool value,
                                                     char **out_error);

LiteParseStatus liteparse_options_set_extract_links(LiteParseOptions *options,
                                                    bool value,
                                                    char **out_error);

LiteParseStatus liteparse_options_set_extract_structure_tree(LiteParseOptions *options,
                                                             bool value,
                                                             char **out_error);

LiteParseStatus liteparse_options_set_extract_text_metadata(LiteParseOptions *options,
                                                            bool value,
                                                            char **out_error);

LiteParseStatus liteparse_options_set_extract_vector_graphics(LiteParseOptions *options,
                                                              bool value,
                                                              char **out_error);

LiteParseStatus liteparse_options_set_extract_xfa_packets(LiteParseOptions *options,
                                                          bool value,
                                                          char **out_error);

LiteParseStatus liteparse_options_set_include_complexity(LiteParseOptions *options,
                                                         bool value,
                                                         char **out_error);

LiteParseStatus liteparse_options_set_keep_headers_footers(LiteParseOptions *options,
                                                           bool value,
                                                           char **out_error);

LiteParseStatus liteparse_options_set_ocr_enabled(LiteParseOptions *options,
                                                  bool value,
                                                  char **out_error);

LiteParseStatus liteparse_options_set_ocr_failure_fatal(LiteParseOptions *options,
                                                        bool value,
                                                        char **out_error);

LiteParseStatus liteparse_options_set_preserve_very_small_text(LiteParseOptions *options,
                                                               bool value,
                                                               char **out_error);

LiteParseStatus liteparse_options_set_quiet(LiteParseOptions *options,
                                            bool value,
                                            char **out_error);

LiteParseStatus liteparse_options_set_render_form_fields(LiteParseOptions *options,
                                                         bool value,
                                                         char **out_error);

LiteParseStatus liteparse_options_set_skip_diagonal_text(LiteParseOptions *options,
                                                         bool value,
                                                         char **out_error);

LiteParseStatus liteparse_options_set_ocr_language(LiteParseOptions *options,
                                                   const char *value,
                                                   char **out_error);

LiteParseStatus liteparse_options_set_ocr_server_url(LiteParseOptions *options,
                                                     const char *value,
                                                     char **out_error);

LiteParseStatus liteparse_options_set_tessdata_path(LiteParseOptions *options,
                                                    const char *value,
                                                    char **out_error);

LiteParseStatus liteparse_options_set_target_pages(LiteParseOptions *options,
                                                   const char *value,
                                                   char **out_error);

LiteParseStatus liteparse_options_set_password(LiteParseOptions *options,
                                               const char *value,
                                               char **out_error);

LiteParseStatus liteparse_options_set_image_output_dir(LiteParseOptions *options,
                                                       const char *value,
                                                       char **out_error);

LiteParseStatus liteparse_options_set_max_pages(LiteParseOptions *options,
                                                size_t value,
                                                char **out_error);

LiteParseStatus liteparse_options_set_num_workers(LiteParseOptions *options,
                                                  size_t value,
                                                  char **out_error);

LiteParseStatus liteparse_options_set_dpi(LiteParseOptions *options, float value, char **out_error);

LiteParseStatus liteparse_options_set_output_format(LiteParseOptions *options,
                                                    uint32_t value,
                                                    char **out_error);

LiteParseStatus liteparse_options_set_image_mode(LiteParseOptions *options,
                                                 uint32_t value,
                                                 char **out_error);

LiteParseStatus liteparse_options_set_crop_box(LiteParseOptions *options,
                                               float top,
                                               float right,
                                               float bottom,
                                               float left,
                                               char **out_error);

LiteParseStatus liteparse_options_clear_crop_box(LiteParseOptions *options, char **out_error);

LiteParseStatus liteparse_options_add_ocr_server_header(LiteParseOptions *options,
                                                        const char *name,
                                                        const char *value,
                                                        char **out_error);

LiteParseStatus liteparse_options_clear_ocr_server_headers(LiteParseOptions *options,
                                                           char **out_error);

LiteParseStatus liteparse_options_add_ocr_hedge_delay_ms(LiteParseOptions *options,
                                                         uint64_t delay_ms,
                                                         char **out_error);

LiteParseStatus liteparse_options_clear_ocr_hedge_delays(LiteParseOptions *options,
                                                         char **out_error);

/**
 * Register a custom in-process OCR engine on an options handle.
 *
 * Parsers created from these options call `recognize` instead of the
 * built-in Tesseract or HTTP engines; OCR still requires `ocr_enabled`.
 * `user_data` is passed through verbatim, `name` labels the engine in logs
 * (null selects "c-callback"), and `prefers_grayscale` picks the raster
 * format. A null `recognize` clears the registration.
 *
 * # Safety
 *
 * `name`, when non-null, must be NUL-terminated. The callback and
 * `user_data` must stay valid, and thread-safe, for the lifetime of every
 * parser created from these options.
 */
LiteParseStatus liteparse_options_set_ocr_callback(LiteParseOptions *options,
                                                   LiteParseOcrRecognizeFn recognize,
                                                   void *user_data,
                                                   const char *name,
                                                   bool prefers_grayscale,
                                                   char **out_error);

/**
 * Append one word-level OCR result from inside a `LiteParseOcrRecognizeFn`.
 *
 * The box is `[x1, y1, x2, y2]` (left, top, right, bottom) in pixel
 * coordinates of the callback's raster; `confidence` is 0.0-1.0. `polygon`
 * is null, or the 8 x/y floats of a rotated detection quad (top-left,
 * top-right, bottom-right, bottom-left in reading order) so rotated text
 * orientation can be recovered.
 *
 * # Safety
 *
 * `sink` must be the current invocation's live sink, `text` NUL-terminated
 * UTF-8, and `polygon` null or 8 readable floats.
 */
LiteParseStatus liteparse_ocr_sink_add(LiteParseOcrSink *sink,
                                       const char *text,
                                       float x1,
                                       float y1,
                                       float x2,
                                       float y2,
                                       float confidence,
                                       const float *polygon);

/**
 * Set the failure message reported when the current callback invocation
 * returns non-zero; ignored on success. Later calls replace earlier ones.
 *
 * # Safety
 *
 * `sink` must be the current invocation's live sink and `message` a
 * NUL-terminated string.
 */
LiteParseStatus liteparse_ocr_sink_set_error(LiteParseOcrSink *sink, const char *message);

/**
 * Return the LiteParse C binding version as a borrowed, NUL-terminated UTF-8
 * string. The returned pointer is static and must not be freed.
 */
const char *liteparse_version(void);

/**
 * Create a parser from a typed options handle. The options remain owned by
 * the caller and may be reused to create additional parsers.
 */
LiteParseStatus liteparse_parser_new_with_options(const LiteParseOptions *options,
                                                  LiteParseParser **out_parser,
                                                  char **out_error);

/**
 * Destroy a parser handle. A null pointer is accepted as a no-op. Must not
 * race with an active call on the handle.
 */
void liteparse_parser_free(LiteParseParser *parser);

/**
 * Parse a document from a filesystem path and return structured JSON: the
 * CLI JSON schema plus top-level `text`, `creator`, `producer`, and
 * `doc_meta` fields.
 *
 * On success `*out_json` receives an owned NUL-terminated UTF-8 string; on
 * failure it stays null and `*out_error` (when non-null) receives an owned
 * error string. Free either with `liteparse_string_free`.
 *
 * # Safety
 *
 * `parser` must be a live handle, `path` NUL-terminated, and each output
 * slot valid for one non-overlapping pointer write (`out_error` may be
 * null).
 */
LiteParseStatus liteparse_parse_path(const LiteParseParser *parser,
                                     const char *path,
                                     char **out_json,
                                     char **out_error);

/**
 * Parse a document from an in-memory byte buffer and return structured
 * JSON. The input is copied, so the caller may free its buffer on return;
 * ownership matches `liteparse_parse_path`.
 *
 * # Safety
 *
 * As `liteparse_parse_path`, except `data` must be readable for `data_len`
 * bytes (null only with `data_len` zero).
 */
LiteParseStatus liteparse_parse_bytes(const LiteParseParser *parser,
                                      const uint8_t *data,
                                      size_t data_len,
                                      char **out_json,
                                      char **out_error);

/**
 * Parse a document into a typed result handle. The handle exposes page and
 * text-item getters and remains independent of the JSON convenience API.
 */
LiteParseStatus liteparse_parse_path_result(const LiteParseParser *parser,
                                            const char *path,
                                            LiteParseResult **out_result,
                                            char **out_error);

/**
 * Parse an in-memory document into a typed result handle. The input bytes are
 * copied before parsing and may be released when this function returns.
 */
LiteParseStatus liteparse_parse_bytes_result(const LiteParseParser *parser,
                                             const uint8_t *data,
                                             size_t data_len,
                                             LiteParseResult **out_result,
                                             char **out_error);

/**
 * Destroy a typed result handle. A null pointer is accepted as a no-op.
 */
void liteparse_result_free(LiteParseResult *result);

/**
 * Serialize a typed result to the same pretty JSON representation returned by
 * `liteparse_parse_path` and `liteparse_parse_bytes`.
 */
LiteParseStatus liteparse_result_to_json(const LiteParseResult *result,
                                         char **out_json,
                                         char **out_error);

/**
 * Return the source document page count, or zero for a null/invalid handle.
 */
uint32_t liteparse_result_get_total_pages(const LiteParseResult *result);

/**
 * Return the number of parsed pages, or zero for a null/invalid handle.
 */
size_t liteparse_result_get_page_count(const LiteParseResult *result);

/**
 * Return a page's 1-based source page number, or zero for an invalid index.
 */
uint32_t liteparse_result_get_page_number(const LiteParseResult *result, size_t page_index);

/**
 * Return the full-document UTF-8 text as a borrowed byte view.
 */
bool liteparse_result_get_text(const LiteParseResult *result, LiteParseByteView *out);

/**
 * Return one parsed page's UTF-8 text as a borrowed byte view.
 */
bool liteparse_result_get_page_text(const LiteParseResult *result,
                                    size_t page_index,
                                    LiteParseByteView *out);

/**
 * Return the document's optional `/Info` Creator value.
 */
bool liteparse_result_get_creator(const LiteParseResult *result, LiteParseByteView *out);

/**
 * Return the document's optional `/Info` Producer value.
 */
bool liteparse_result_get_producer(const LiteParseResult *result, LiteParseByteView *out);

/**
 * Return one page's viewport dimensions in 72-DPI points.
 */
bool liteparse_result_get_page_size(const LiteParseResult *result,
                                    size_t page_index,
                                    float *out_width,
                                    float *out_height);

/**
 * Return the number of text items on one parsed page.
 */
size_t liteparse_result_get_text_item_count(const LiteParseResult *result, size_t page_index);

/**
 * Return one text item and its borrowed UTF-8 fields.
 */
bool liteparse_result_get_text_item(const LiteParseResult *result,
                                    size_t page_index,
                                    size_t item_index,
                                    LiteParseTextItem *out);

/**
 * Return the number of word boxes on one text item. Always zero unless
 * `emit_word_boxes` was enabled in the options builder.
 */
size_t liteparse_result_get_word_box_count(const LiteParseResult *result,
                                           size_t page_index,
                                           size_t item_index);

/**
 * Return one word box of one text item. Word boxes are absent from the JSON
 * output; this getter is their only access path. Populated only when
 * `emit_word_boxes` was enabled and the item produced a word split.
 */
bool liteparse_result_get_word_box(const LiteParseResult *result,
                                   size_t page_index,
                                   size_t item_index,
                                   size_t word_index,
                                   LiteParseWordBox *out);

/**
 * Return the number of extracted embedded images.
 */
size_t liteparse_result_get_image_count(const LiteParseResult *result);

/**
 * Return one extracted embedded image and its borrowed metadata/bytes.
 */
bool liteparse_result_get_image(const LiteParseResult *result,
                                size_t image_index,
                                LiteParseImage *out);

/**
 * Search one parsed page's text items for phrase matches.
 *
 * Consecutive items are concatenated, so a spanning phrase yields one
 * merged item with a combined bounding box. An empty phrase matches
 * nothing; an out-of-range `page_index` is an invalid argument. Matches are
 * copies, independent of `result`.
 *
 * # Safety
 *
 * `result` must be a live handle, `phrase` NUL-terminated, and each output
 * slot valid for one non-overlapping pointer write.
 */
LiteParseStatus liteparse_result_search(const LiteParseResult *result,
                                        size_t page_index,
                                        const char *phrase,
                                        bool case_sensitive,
                                        LiteParseSearchMatches **out_matches,
                                        char **out_error);

/**
 * Destroy a search-match handle. A null pointer is accepted as a no-op.
 */
void liteparse_search_matches_free(LiteParseSearchMatches *matches);

/**
 * Return the number of phrase matches, or zero for a null/invalid handle.
 */
size_t liteparse_search_matches_get_count(const LiteParseSearchMatches *matches);

/**
 * Return one phrase match as a text-item view. String fields are borrowed
 * from the owning `LiteParseSearchMatches` handle, not the parse result.
 */
bool liteparse_search_matches_get(const LiteParseSearchMatches *matches,
                                  size_t index,
                                  LiteParseTextItem *out);

/**
 * Render document pages to PNG screenshots.
 *
 * `page_numbers` selects 1-based source pages; null with length zero
 * renders every page. Non-PDF inputs are converted first. Rect detection is
 * controlled by `liteparse_options_set_detect_screenshot_rects`.
 *
 * # Safety
 *
 * As `liteparse_parse_path`, plus `page_numbers` must be readable for
 * `page_numbers_len` values (null only with length zero).
 */
LiteParseStatus liteparse_screenshot_path(const LiteParseParser *parser,
                                          const char *path,
                                          const uint32_t *page_numbers,
                                          size_t page_numbers_len,
                                          LiteParseScreenshots **out_screenshots,
                                          char **out_error);

/**
 * Render pages of an in-memory document to PNG screenshots. The input bytes
 * are copied before rendering. See `liteparse_screenshot_path` for the page
 * selection and safety contract; `data` follows `liteparse_parse_bytes`.
 */
LiteParseStatus liteparse_screenshot_bytes(const LiteParseParser *parser,
                                           const uint8_t *data,
                                           size_t data_len,
                                           const uint32_t *page_numbers,
                                           size_t page_numbers_len,
                                           LiteParseScreenshots **out_screenshots,
                                           char **out_error);

/**
 * Destroy a screenshots handle. A null pointer is accepted as a no-op.
 */
void liteparse_screenshots_free(LiteParseScreenshots *screenshots);

/**
 * Return the number of rendered pages, or zero for a null/invalid handle.
 */
size_t liteparse_screenshots_get_count(const LiteParseScreenshots *screenshots);

/**
 * Return one rendered page and its borrowed PNG bytes.
 */
bool liteparse_screenshots_get(const LiteParseScreenshots *screenshots,
                               size_t index,
                               LiteParseScreenshot *out);

/**
 * Return one detected rect of one rendered page.
 */
bool liteparse_screenshots_get_rect(const LiteParseScreenshots *screenshots,
                                    size_t index,
                                    size_t rect_index,
                                    LiteParseScreenshotRect *out);

/**
 * Compute per-page complexity signals, a cheap pre-OCR check for deciding
 * whether a document needs advanced parsing. Pages dropped by
 * `continue_on_page_error` are absent, so match entries to source pages by
 * `page_number`, not by index.
 *
 * # Safety
 *
 * Same contract as `liteparse_parse_path`, with `out_complexity` in place of
 * `out_json`.
 */
LiteParseStatus liteparse_is_complex_path(const LiteParseParser *parser,
                                          const char *path,
                                          LiteParseComplexity **out_complexity,
                                          char **out_error);

/**
 * Compute per-page complexity signals for an in-memory document. The input
 * bytes are copied before parsing; `data` follows `liteparse_parse_bytes`.
 */
LiteParseStatus liteparse_is_complex_bytes(const LiteParseParser *parser,
                                           const uint8_t *data,
                                           size_t data_len,
                                           LiteParseComplexity **out_complexity,
                                           char **out_error);

/**
 * Destroy a complexity handle. A null pointer is accepted as a no-op.
 */
void liteparse_complexity_free(LiteParseComplexity *complexity);

/**
 * Return the number of analyzed pages, or zero for a null/invalid handle.
 */
size_t liteparse_complexity_get_count(const LiteParseComplexity *complexity);

/**
 * Return one page's flat complexity signals.
 */
bool liteparse_complexity_get(const LiteParseComplexity *complexity,
                              size_t index,
                              LiteParsePageComplexity *out);

/**
 * Serialize the full complexity report, including the per-page flag reasons
 * and layout signals absent from the flat struct, as a pretty JSON array.
 * String ownership matches `liteparse_parse_path`.
 */
LiteParseStatus liteparse_complexity_to_json(const LiteParseComplexity *complexity,
                                             char **out_json,
                                             char **out_error);

/**
 * Open a document for bounded-memory batch parsing.
 *
 * The session snapshots the parser's configuration and may outlive the
 * parser handle. `liteparse_session_next_batch` yields `batch_size` pages
 * at a time (zero selects the default of 25); cross-page passes see only
 * their own batch, so output can differ from a whole-document parse. Fails
 * with `LITEPARSE_STATUS_INVALID_CONFIG` when `target_pages` is configured.
 * Sessions are single-owner: `next_batch` mutates the handle.
 *
 * # Safety
 *
 * Same contract as `liteparse_parse_path`, with `out_session` in place of
 * `out_json`.
 */
LiteParseStatus liteparse_session_open_path(const LiteParseParser *parser,
                                            const char *path,
                                            size_t batch_size,
                                            LiteParseSession **out_session,
                                            char **out_error);

/**
 * Open an in-memory document for bounded-memory batch parsing. The input
 * bytes are copied before opening; `data` follows `liteparse_parse_bytes`.
 * See `liteparse_session_open_path` for session semantics.
 */
LiteParseStatus liteparse_session_open_bytes(const LiteParseParser *parser,
                                             const uint8_t *data,
                                             size_t data_len,
                                             size_t batch_size,
                                             LiteParseSession **out_session,
                                             char **out_error);

/**
 * Destroy a session handle, releasing the converted temporary PDF for a
 * non-PDF source. A null pointer is accepted as a no-op. Must not race with
 * an active `liteparse_session_next_batch` call.
 */
void liteparse_session_free(LiteParseSession *session);

/**
 * Return the total pages in the source document (before `max_pages` or
 * batching), or zero for a null/invalid handle.
 */
uint32_t liteparse_session_get_total_pages(const LiteParseSession *session);

/**
 * Parse and return the next batch as a typed result handle, with the
 * batch's inclusive 1-based page range in the non-null page outputs. Once
 * every page within `max_pages` has been yielded, returns
 * `LITEPARSE_STATUS_OK` with `*out_result` left null.
 *
 * # Safety
 *
 * `session` must be a live handle with no concurrent call using it; each
 * output slot must be valid for one non-overlapping write (page and error
 * outputs may be null).
 */
LiteParseStatus liteparse_session_next_batch(LiteParseSession *session,
                                             LiteParseResult **out_result,
                                             uint32_t *out_start_page,
                                             uint32_t *out_end_page,
                                             char **out_error);

/**
 * Free an owned string returned by this library (never the static
 * `liteparse_version` pointer). A null pointer is accepted as a no-op.
 */
void liteparse_string_free(char *string);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LITEPARSE_H */
