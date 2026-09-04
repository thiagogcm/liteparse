# LiteParse C bindings

`liteparse-c` exposes LiteParse through a small, typed C ABI meant to be
bound from foreign runtimes (Java, .NET, Go, …). The checked-in header
`include/liteparse.h` is generated with [cbindgen](https://github.com/mozilla/cbindgen).

Three kinds of handle cover everything:

| Handle | Created by | Purpose |
|---|---|---|
| `LiteParseParser` | `liteparse_parser_new` | Configuration plus an optional in-process OCR callback. |
| `LiteParseDocument` | `liteparse_document_open_path` / `_open_bytes` | A source opened once (non-PDF inputs are converted here; bytes are copied at open and again per parse, so prefer paths for large inputs). Parse, screenshot, and analyze any page selection against it. |
| `LiteParseResult`, `LiteParseScreenshots`, `LiteParseComplexity`, `LiteParseSearchMatches` | document and result operations | Owned outputs read through packed `repr(C)` views. |

## Build

```bash
cargo build --release -p liteparse-c --no-default-features
```

Drop `--no-default-features` to compile the Tesseract backend. The build
produces a shared library and a static library. At runtime PDFium is loaded
dynamically: it must sit beside the library, on the platform library path, or
in the directory named by `PDFIUM_LIB_PATH`.

Regenerate the header whenever the exported ABI changes:

```bash
cargo install cbindgen --version 0.29.4 --locked
cbindgen --config crates/liteparse-c/cbindgen.toml \
  --crate liteparse-c --output crates/liteparse-c/include/liteparse.h
```

## Minimal example

```c
#include <stdio.h>
#include <string.h>
#include "liteparse.h"

static LiteParseByteView cstr(const char *s) {
  LiteParseByteView view = {(const uint8_t *)s, strlen(s)};
  return view;
}

int main(void) {
  LiteParseConfig config = liteparse_config_default();
  config.bools_set = LITEPARSE_FLAG_QUIET;
  config.bools_values = LITEPARSE_FLAG_QUIET;

  LiteParseParserNew parser = liteparse_parser_new(&config);
  if (parser.status != LITEPARSE_STATUS_OK) return 1;

  LiteParseDocumentNew document =
      liteparse_document_open_path(parser.handle, cstr("document.pdf"));
  LiteParseResultNew result = {LITEPARSE_STATUS_PARSE_ERROR, NULL};
  if (document.status == LITEPARSE_STATUS_OK) {
    result = liteparse_document_parse(document.handle, NULL, 0);
  }
  if (result.status == LITEPARSE_STATUS_OK) {
    LiteParseByteView text = liteparse_result_text(result.handle);
    fwrite(text.ptr, 1, text.len, stdout);
  } else {
    LiteParseByteView error = liteparse_last_error();
    fprintf(stderr, "%.*s\n", (int)error.len, (const char *)error.ptr);
  }

  liteparse_result_free(result.handle);
  liteparse_document_free(document.handle);
  liteparse_parser_free(parser.handle);
  return result.status == LITEPARSE_STATUS_OK ? 0 : 1;
}
```

Every `*_free` accepts null, so cleanup never needs a `goto`. Compile and
link against the release library:

```bash
cc -std=c11 example.c -I crates/liteparse-c/include -L target/release \
  -lliteparse_c -o example
```

## ABI rules

- Begin with `liteparse_config_default()` and set only the fields you need.
  `size_of_config` must equal `sizeof(LiteParseConfig)`.
- Set a flag bit in `bools_set` and its value in `bools_values`. Zero numeric
  fields and `LITEPARSE_UNSET` enum fields keep the core defaults.
- Strings and byte buffers are `LiteParseByteView`s: borrowed, not
  NUL-terminated, and a null pointer with zero length means absent.
  Configuration views are copied during `liteparse_parser_new`.
- Views and slices returned from a handle borrow from that handle and stay
  valid until it is freed. Search matches copy their items and outlive the
  result they came from.
- Optional scalars are a value plus a `has_*` flag. Follow the field order and
  platform ABI padding represented by the checked-in header exactly. Most
  structures group boolean fields at the end, but `LiteParseConfig` places
  `has_crop_box` immediately after `crop_box` and may require padding before
  the following `LiteParseByteView`.
- Slice accessors write `*out_len` and return null with zero length for an
  invalid handle, an invalid index, or an empty collection.
- Every fallible function returns a `LiteParseStatus`; creation functions
  return `{status, handle}` by value with a null handle on failure.
  `liteparse_last_error()` then returns a thread-local message valid until the
  next failed call on the same thread.
- `LITEPARSE_STATUS_PANIC` means a Rust panic was caught at the boundary.
  Free the handle involved and do not reuse it.
- Handles are caller-owned and released once with their matching `*_free`.
  Parser and document handles may be used from several threads at once,
  including `liteparse_parser_set_ocr_callback`; destruction must wait for
  in-flight operations.

## Operations on a document

| Function | Notes |
|---|---|
| `liteparse_document_total_pages` | Page count recorded at open. |
| `liteparse_document_outline` | Bookmarks, walked once at open. |
| `liteparse_document_parse(doc, pages, len)` | Parse the given 1-based pages, or every page when `pages` is null with zero length. `max_pages` caps either. |
| `liteparse_document_screenshot(doc, pages, len, dpi, region)` | Render PNGs. `0` keeps the configured DPI. A non-null `region` (viewport points, top-left origin) crops each page; detected rects are then region-relative. The whole page is rasterized before cropping, so cost follows page size and DPI, not region size. |
| `liteparse_document_complexity(doc, pages, len)` | Cheap pre-OCR signals per page with a `reasons_mask`; `liteparse_complexity_json` returns the full report. |

Page selections are validated against the document's page count (any page
outside it is `LITEPARSE_STATUS_INVALID_ARGUMENT`), de-duplicated, and
processed in ascending order by every operation. With
`LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR` a page whose render or extraction
fails is skipped; argument errors are never skipped.

Results are read with `liteparse_result_text`, `liteparse_result_page_text`,
`liteparse_result_page_markdown` (filled under
`LITEPARSE_OUTPUT_FORMAT_MARKDOWN`, while page text stays plain),
`liteparse_result_text_items`, `liteparse_result_word_boxes`
(`LITEPARSE_FLAG_EMIT_WORD_BOXES`), and the packed slices for images,
screenshots, outline, page errors, annotations, form fields, structure trees,
layout blocks, vector graphics, and XFA packets. `liteparse_result_to_json`
returns the whole result as JSON, and `liteparse_result_search` finds phrase
matches with merged bounding boxes.

## OCR callback

`liteparse_parser_set_ocr_callback` installs an in-process OCR engine. The
callback receives a page raster (RGB, or grayscale when registered with
`prefers_grayscale`) and submits words through `liteparse_ocr_sink_add` or
`liteparse_ocr_sink_add_batch`; on failure it records a message with
`liteparse_ocr_sink_set_error` and returns nonzero. It may run concurrently on
worker threads and must be thread-safe, non-unwinding, and valid for the
parser's lifetime. Documents opened before a callback change keep the engine
they were opened with.

## Tests

`cargo test -p liteparse-c` runs the Rust-side ABI tests and, when a C
compiler is on `PATH`, compiles `tests/header_smoke.c` with
`-std=c11 -Wall -Wextra -Werror -pedantic` against the checked-in header and
runs it on the fixtures in `integration_tests_data/`.
