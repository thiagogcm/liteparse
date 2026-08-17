# LiteParse C bindings

This crate exposes a small, typed C ABI over LiteParse:

- configure and destroy an opaque options handle;
- create and destroy an opaque parser handle;
- parse a document into a typed result handle or JSON;
- inspect pages and text items through borrowed `ptr + len` views;
- render pages to PNG screenshots;
- run the per-page complexity (pre-OCR) check;
- parse very large documents in bounded-memory page batches;
- search parsed pages for phrases with merged bounding boxes;
- plug a custom in-process OCR engine in via a C callback;
- receive owned JSON/error strings and free them with the library allocator.

The C layer does not mirror the full Rust object graph. It exposes a focused
typed result surface for common document, page, and text-item access while
retaining explicit JSON serialization for fields not yet covered by typed
getters.

## Build

```bash
cargo build --release -p liteparse-c --no-default-features
```

Omit `--no-default-features` to include the built-in Tesseract OCR engine. The build emits `libliteparse_c.so`, `libliteparse_c.dylib`, or `liteparse_c.dll` and a static library, depending on the target. PDFium is still a runtime dependency and must be discoverable beside the library or through the platform's dynamic-library search path.

The checked-in header is generated with cbindgen 0.29.4:

```bash
cargo install cbindgen --version 0.29.4 --locked
make c-header
```

## Example

```c
#include <stdio.h>
#include "liteparse.h"

int main(void) {
  LiteParseOptions *options = NULL;
  LiteParseParser *parser = NULL;
  char *json = NULL;
  char *error = NULL;

  LiteParseStatus status = liteparse_options_new(&options, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_ocr_enabled(options, false, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_quiet(options, true, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_output_format(
        options, LITEPARSE_OUTPUT_FORMAT_MARKDOWN, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_parser_new_with_options(options, &parser, &error);
  liteparse_options_free(options);
  if (status != LITEPARSE_STATUS_OK) {
    fprintf(stderr, "%s\n", error ? error : "unknown error");
    liteparse_string_free(error);
    return 1;
  }

  status = liteparse_parse_path(parser, "document.pdf", &json, &error);
  if (status == LITEPARSE_STATUS_OK) {
    puts(json);
  } else {
    fprintf(stderr, "%s\n", error ? error : "unknown error");
  }

  liteparse_string_free(json);
  liteparse_string_free(error);
  liteparse_parser_free(parser);
  return status == LITEPARSE_STATUS_OK ? 0 : 1;
}
```

Compile it against the dynamic library:

```bash
cc example.c -I crates/liteparse-c/include -L target/release -lliteparse_c -o example
```

## Configuration

Create an options handle with `liteparse_options_new`, then configure it with
typed setters before calling `liteparse_parser_new_with_options`. Omitted
settings use core defaults, and invalid values are rejected by the setter or
when the parser is created. The builder covers booleans, numeric limits and
resolution, paths and page selectors, OCR settings, output/image modes, crop
boxes, request headers, and OCR hedge delays. The `LITEPARSE_*` constants in
the header define the accepted output and image-mode values.

`image_output_dir` requires image extraction or embedded image mode. The C
result exposes embedded-image metadata and bytes through typed getters. Page
renders are available separately through the screenshot API below.

The JSON result follows `lit parse --format json` and adds the top-level
`text`, `creator`, `producer`, and `doc_meta` fields used by the higher-level
language APIs. Embedded-image metadata is included; use `image_output_dir` to
persist image bytes.

Parser handles support concurrent parse calls. Destruction must not race with an active call. Calls that write embedded images concurrently should use distinct `image_output_dir` values or be serialized, because generated image names can collide.

Every result or error string is owned by the caller and must be released exactly once with `liteparse_string_free`. The static string returned by `liteparse_version` must not be freed. `LITEPARSE_STATUS_PANIC` contains an unwinding Rust panic, after which the parser should only be freed, not reused. Process-aborting panics and allocation failures cannot be contained by the C boundary.

## Typed results

Use `liteparse_parse_path_result` or `liteparse_parse_bytes_result` when the
caller needs structured access:

```c
LiteParseResult *result = NULL;
LiteParseByteView text = {NULL, 0};

LiteParseStatus status = liteparse_parse_path_result(
    parser, "document.pdf", &result, &error);
if (status == LITEPARSE_STATUS_OK &&
    liteparse_result_get_text(result, &text)) {
  fwrite(text.ptr, 1, text.len, stdout);
}

liteparse_result_free(result);
```

`LiteParseByteView` values are borrowed, are not NUL-terminated, and remain
valid until the owning `LiteParseResult` is freed. A successful getter may
return a non-NULL pointer with length zero for an empty value. Result getters
are total: null handles and invalid indexes return zero/false and clear output
views. Enable `extract_images` or `image_mode = embed` in the options builder
to access embedded image metadata and bytes through
`liteparse_result_get_image`. `liteparse_result_to_json` serializes fields not
yet represented by typed getters.

Like every other handle, typed results are caller-owned and must be released
exactly once with their matching `*_free` function; the ownership and
concurrency rules from the Configuration section apply unchanged.

## Custom OCR engines

`liteparse_options_set_ocr_callback` registers an in-process OCR engine.
Parsers created from the options call it for each page raster that needs OCR,
instead of built-in Tesseract or an HTTP server. OCR still runs only when
`ocr_enabled` is set, and only on pages the complexity gate flags. This is the
C equivalent of the WASM build's JS `ocrEngine` callback.

The callback receives a tightly packed raster (RGB or grayscale, chosen by the
`prefers_grayscale` registration flag), the configured language, and the
render DPI. It pushes word-level results (text, `[x1, y1, x2, y2]` pixel
box, 0-1 confidence, optional 4-point polygon for rotated text) through
`liteparse_ocr_sink_add`, and returns zero on success. On a non-zero return
the message set via `liteparse_ocr_sink_set_error` becomes the parse error
when `ocr_failure_fatal` is enabled. The sink is valid only during the
invocation.

```c
static uint32_t recognize(void *user_data, const uint8_t *pixels,
                          size_t pixels_len, uint32_t width, uint32_t height,
                          uint32_t pixel_format, const char *language,
                          float dpi, LiteParseOcrSink *sink) {
  for (/* each word my engine finds in pixels */;;) {
    liteparse_ocr_sink_add(sink, word_text, x1, y1, x2, y2, confidence, NULL);
  }
  return 0;
}
// ...
liteparse_options_set_ocr_callback(options, recognize, my_engine,
                                   "my-engine", false, &error);
```

The callback and its `user_data` must stay valid for the lifetime of every
parser created from the options, must not unwind, and must be thread-safe:
parses run on a multi-threaded runtime and can invoke the callback from
several threads at once. Passing a null callback clears the registration.

## Word boxes

Enable `liteparse_options_set_emit_word_boxes` to compute per-word sub-boxes
within each text item, then read them with
`liteparse_result_get_word_box_count` and `liteparse_result_get_word_box`.
Word boxes never appear in the JSON output; these getters are their only
access path. They roughly double the text-item payload, so leave them off
unless doing word-level bbox attribution. Items that produced no word split
(e.g. OCR-sourced or single-token items) report zero boxes.

## Search

`liteparse_result_search` searches one parsed page's text items for a phrase
and returns an owned `LiteParseSearchMatches` handle. Consecutive items are
concatenated during the search, so a phrase spanning several items yields one
synthetic merged item with a combined bounding box. Matches are read as
`LiteParseTextItem` views via `liteparse_search_matches_get`; their strings
are owned by the match handle, which is independent of the parse result and
may outlive it. Free it with `liteparse_search_matches_free`.

## Screenshots

`liteparse_screenshot_path` / `liteparse_screenshot_bytes` render pages to
PNG. Pass a `uint32_t` array of 1-based page numbers, or NULL with length zero
for every page. Each entry exposes dimensions, a solid-fill flag, and borrowed
PNG bytes valid until the `LiteParseScreenshots` handle is freed. When
`liteparse_options_set_detect_screenshot_rects` is enabled, detected solid
rectangles/lines are readable per page via `liteparse_screenshots_get_rect`.

```c
LiteParseScreenshots *shots = NULL;
if (liteparse_screenshot_path(parser, "document.pdf", NULL, 0, &shots,
                              &error) == LITEPARSE_STATUS_OK) {
  LiteParseScreenshot shot;
  for (size_t i = 0; liteparse_screenshots_get(shots, i, &shot); i++)
    write_png(shot.page_number, shot.png.ptr, shot.png.len);
}
liteparse_screenshots_free(shots);
```

## Complexity (pre-OCR check)

`liteparse_is_complex_path` / `liteparse_is_complex_bytes` run the cheap
per-page complexity pass without a full parse. `liteparse_complexity_get`
fills a flat `LiteParsePageComplexity` (text coverage, image signals, garbled
flag, `needs_ocr` verdict); `liteparse_complexity_to_json` serializes the full
report including the per-page flag reasons and layout signals. Pages dropped
by `continue_on_page_error` are absent, so match entries by `page_number`, not
index.

## Batch sessions

`liteparse_session_open_path` / `liteparse_session_open_bytes` open a document
once for bounded-memory batch parsing. A batch size of zero selects the
default (25 pages). `liteparse_session_next_batch` yields each batch as an
ordinary `LiteParseResult` handle plus its inclusive 1-based source page
range, and returns `LITEPARSE_STATUS_OK` with a null result once every page
within `max_pages` has been yielded. Sessions snapshot the parser
configuration, so the parser handle may be freed while a session lives;
opening fails when `target_pages` is configured. Cross-page passes (repeated
header/footer removal, image deduplication) see only the pages in their own
batch, so output can differ from a whole-document parse.

Unlike parser handles, a session must not be used from two threads at once:
`liteparse_session_next_batch` mutates it. Freeing the session releases the
converted temporary PDF for a non-PDF source.

```c
LiteParseSession *session = NULL;
if (liteparse_session_open_path(parser, "large.pdf", 25, &session, &error) ==
    LITEPARSE_STATUS_OK) {
  for (;;) {
    LiteParseResult *batch = NULL;
    uint32_t start = 0, end = 0;
    if (liteparse_session_next_batch(session, &batch, &start, &end, &error) !=
            LITEPARSE_STATUS_OK ||
        batch == NULL)
      break;
    consume(batch, start, end);
    liteparse_result_free(batch);
  }
}
liteparse_session_free(session);
```
