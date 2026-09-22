# LiteParse C bindings

`liteparse-c` exposes LiteParse through a small, packed C ABI meant to be
bound from foreign runtimes. The checked-in header `include/liteparse.h` is
generated with [cbindgen](https://github.com/mozilla/cbindgen).

| Handle | Created by | Read through |
|---|---|---|
| `LiteParseParser` | `liteparse_parser_new` | configuration plus an optional in-process OCR callback |
| `LiteParseDocument` | `liteparse_document_open_path` / `_open_bytes` | `liteparse_document_info` |
| `LiteParseResult` | `liteparse_document_parse`, `_extract`, `liteparse_parser_parse_content` | `liteparse_result_view` |
| `LiteParseScreenshots` | `liteparse_document_screenshot` | `liteparse_screenshots_view` |
| `LiteParseComplexity` | `liteparse_document_complexity` | `liteparse_complexity_view` |
| `LiteParseRawText` | `liteparse_document_raw_text` | `liteparse_raw_text_view` |
| `LiteParsePageObjects` | `liteparse_document_page_objects` | `liteparse_page_objects_view` |
| `LiteParseSearchMatches` | `liteparse_result_search` | `liteparse_search_matches_view` |

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

/* Every output string is NUL-terminated inside the view's pool. */
static const char *str(const uint8_t *pool, LiteParseStr s) {
  return (const char *)pool + s.offset;
}

int main(void) {
  LiteParseConfig config;
  liteparse_config_init(&config);
  config.bools_set = LITEPARSE_FLAG_QUIET;
  config.bools_values = LITEPARSE_FLAG_QUIET;

  const char *path = "document.pdf";
  LiteParseParser *parser = NULL;
  LiteParseDocument *document = NULL;
  LiteParseResult *result = NULL;
  LiteParseStatus status = liteparse_parser_new(&config, &parser);
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_document_open_path(parser, (const uint8_t *)path, strlen(path),
                                          &document);
  }
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_document_parse(document, NULL, 0, &result);
  }
  if (status == LITEPARSE_STATUS_OK) {
    const LiteParseResultView *view = liteparse_result_view(result);
    const uint8_t *pool = view->content.pool;
    fputs(str(pool, view->text), stdout);
    for (size_t i = 0; i < view->content.pages_len; i++) {
      const LiteParsePage *page = &view->content.pages[i];
      const LiteParseTextItem *items = view->content.items + page->item_offset;
      printf("page %u (%s): %u items\n", page->page_number, str(pool, page->label),
             page->item_count);
      if (page->item_count > 0) printf("  first: %s\n", str(pool, items[0].text));
    }
  } else {
    LiteParseByteView error;
    liteparse_last_error(&error);
    fprintf(stderr, "%.*s\n", (int)error.len, (const char *)error.ptr);
  }

  liteparse_result_free(result);
  liteparse_document_free(document);
  liteparse_parser_free(parser);
  return status == LITEPARSE_STATUS_OK ? 0 : 1;
}
```

Every `*_free` accepts null, so cleanup never needs a `goto`. Compile and
link against the release library:

```bash
cc -std=c11 example.c -I crates/liteparse-c/include -L target/release \
  -lliteparse_c -o example
```

## ABI model

Three kinds of struct cross the boundary: **records** (array elements such
as `LiteParsePage`, `LiteParseTextItem`, `LiteParseAnnotation`), **views**
(`LiteParseContent`, `LiteParseResultView`, and the other `*View` and
`*Info` structs, which hold the array pointers and lengths), and **inputs**
(`LiteParseConfig`, `LiteParseContent`, `LiteParseRenderRegion`,
`LiteParseOcrWord`). The layout is designed to be read by hosted runtimes
(Java FFM, .NET, Go) without helper code: records hold no pointers, and a
whole result can be copied out of the handle with one `memcpy` per array.

- **Records are packed and pointer-free.** Every record is `repr(C)` and
  holds only fixed-width scalars, nested records, and `LiteParseStr`
  ranges. There are no `bool` or `size_t` fields. The only exception is the
  `LiteParseByteView` carrying a binary payload (`LiteParseImage.bytes`,
  `LiteParseScreenshot.png`, `LiteParsePageObject.image_*`), which borrows
  from the handle. Optional values and boolean properties are bits in the
  record's `flags` (`LITEPARSE_<RECORD>_FLAG_HAS_*`,
  `LITEPARSE_<RECORD>_FLAG_*`).
- **Collections are flat arrays with ranges.** A handle owns one flat array
  per record type; parents carry `uint32_t` `x_offset`/`x_count` pairs into
  it. Index fields such as `parent_index` are absolute within their array,
  with `LITEPARSE_NO_PARENT` for roots.
- **Strings live in a pool.** Every view that carries text has a
  `pool`/`pool_len` byte array, and every string in its records and scalars
  is a `LiteParseStr {offset, len}` into that pool (`LiteParseResultView`
  strings index `content.pool`). Output pools start with a NUL byte and
  NUL-terminate every string, so `pool + offset` is a valid C string and
  `{0, 0}` reads as `""`. Absent and empty strings are both `len == 0`.
  Input pools (`liteparse_parser_parse_content`) need only the ranges to be
  in bounds and valid UTF-8. Offsets are unsigned 32-bit: a pool is at most
  4 GiB.
- **One view per handle.** `liteparse_<handle>_view` returns a pointer to a
  struct of `{ptr, len}` array pairs and scalars. It is borrowed from the
  handle, valid until the matching `*_free`, and null for a null handle.
  Empty arrays are a null pointer with zero length. Because records are
  pointer-free, copying the arrays and the pool out of the view and then
  freeing the handle leaves a fully readable result.
- **Byte buffers** are `LiteParseByteView`s: borrowed raw bytes (PNG and
  image payloads, JSON, the version and error strings). Absent and empty are
  both a null pointer with zero length. Colors are packed ARGB `uint32_t`.
  Enumerations are `uint32_t` constants.
- **Function arguments** never pass structs by value: caller strings are
  `(const uint8_t *, size_t)` pairs, not NUL-terminated, and everything
  else is a pointer, scalar, or out pointer.
- **Creation** takes an out pointer and returns a `LiteParseStatus`; the out
  pointer receives null on failure. `liteparse_last_error(&view)` then
  yields a thread-local message valid until the next failed call on the
  same thread: read it on the thread that made the failing call, before any
  other call that may fail. `LITEPARSE_STATUS_PANIC` means a Rust panic was
  caught at the boundary: free the handle involved and do not reuse it.
- **ABI introspection.** `LITEPARSE_ABI_VERSION` / `liteparse_abi_version()`
  change together whenever an exported struct, constant, or signature
  changes incompatibly. `liteparse_sizeof(LITEPARSE_TYPE_*)` reports the
  size of every exported struct so a binding that declares layouts by hand
  can assert them at load time.
- **Configuration** starts with `liteparse_config_init` and sets only what it
  needs. Core booleans use a bit in `bools_set` plus its value in
  `bools_values`; `LITEPARSE_UNSET` keeps the native default in `u32`
  fields, so `max_pages = 0` is representable; zero `dpi` keeps the default.
  `size_of_config` must equal `sizeof(LiteParseConfig)`. Views are copied
  during `liteparse_parser_new`.
- **Threads.** Parser and document handles may be used from several threads
  at once, including `liteparse_parser_set_ocr_callback`; destruction must
  wait for in-flight operations. Every operation blocks its calling thread
  for the whole parse; a runtime with green threads (Java virtual threads)
  should run them on a dedicated platform-thread executor rather than pin
  its carriers.

## Consuming from Java FFM

The layout is arranged so a hand-written or jextract-generated binding
needs no per-string native calls:

1. Assert `liteparse_abi_version() == LITEPARSE_ABI_VERSION` and, for each
   hand-declared `StructLayout`, `layout.byteSize() ==
   liteparse_sizeof(LITEPARSE_TYPE_*)` once at class initialisation.
2. Pass caller strings as `(MemorySegment, long)` from an arena; no struct
   needs to be allocated to call any function.
3. After `liteparse_document_parse`, read `liteparse_result_view` once,
   `MemorySegment.copy` the arrays you need and `content.pool` into heap
   segments (or `byte[]`s), then `liteparse_result_free` immediately. The
   copied records stay valid because they contain only offsets. Strings
   decode as `new String(pool, offset, len, UTF_8)`.
4. Binary payloads (`png`, image `bytes`, page-object `image_*`) are the one
   thing to copy before freeing; they are `LiteParseByteView`s into the
   handle.
5. For `liteparse_parser_parse_content`, build one pool `byte[]`, record each
   string's offset and length as you append, and pass the pool with the
   record arrays; the library copies everything during the call.

## Results

`LiteParseResultView` embeds a `LiteParseContent` (pages, text items, word
boxes, char codes, layout graphics, struct nodes, marked-content ids, image
refs, images, annotations, quadpoints, form fields, option strings, structure
nodes and attributes, blocks, cells, rows, vector shapes and lines, outline,
page errors) plus result-only data: document text, creator, producer,
`doc_meta`, `form_type`, screenshots and their rects, XFA packets, figure
rects, item frames, projected lines and spans, and the XY-cut region tree.

`LiteParsePage` carries a range into every page-scoped array, the page label,
text, Markdown (under `LITEPARSE_OUTPUT_FORMAT_MARKDOWN`), geometry (visible
box, user unit, rotation), content bounds, and inline complexity. Its
`HAS_*` flags distinguish "extraction enabled, none found" from "disabled".
All of its strings, like every string in the result, are `LiteParseStr`
ranges into `content.pool`.

Text metadata (font metrics, colors, char codes, marked-content ids) is
always exported when the core holds it; `LITEPARSE_RESULT_FLAG_TEXT_METADATA`
reports whether it was requested. `liteparse_result_to_json` returns the
core's JSON form of a parse result.

`liteparse_document_extract` returns a result with
`LITEPARSE_RESULT_FLAG_EXTRACT_ONLY`: pre-projection pages with heuristic
text items, graphics, and the configured extras, but no text, Markdown, or
projection. Link stamping and word boxes follow the parse rules (links only
under Markdown; word boxes when requested or under Markdown).
`LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS` and `flattened_page_numbers`
report pages flattened on a temporary document to recover widget text.

Projected lines index `projected_spans` (text-item records sharing the
content's `words` and `char_codes`) and `region_paths`. Regions are
flattened in pre-order with absolute `parent_index` and child ranges; a
line's region path is the sequence of child ordinals from its page's root.
`item_frames` map projected geometry back to page space on pages where
rotation handling displaced content.

## Operations on a document

| Function | Notes |
|---|---|
| `liteparse_document_info` | Page count, `LITEPARSE_DOCUMENT_FLAG_CONVERTED`, and bookmarks walked at open. |
| `liteparse_document_parse(doc, pages, len, &out)` | Parse the given 1-based pages, or every page when `pages` is null with zero length. `max_pages` caps either. |
| `liteparse_document_extract(doc, pages, len, &out)` | Pre-projection pages; feed `&view->content` to `liteparse_parser_parse_content` to project and classify. |
| `liteparse_document_screenshot(doc, pages, len, dpi, region, &out)` | Render PNGs. `0` keeps the configured DPI. A non-null `region` (viewport points, top-left origin) crops each page; detected rects are then region-relative. The whole page is rasterized before cropping. |
| `liteparse_document_complexity(doc, pages, len, &out)` | Cheap pre-OCR signals per page; `liteparse_complexity_to_json` returns the full report. |
| `liteparse_document_raw_text(doc, pages, len, &out)` | Heuristic-free PDFium runs: no gap merge, projection, OCR, or Markdown. Items carry baseline-aware angles, tight grounding bounds, generated-space gaps, and visible form-widget appearance text. |
| `liteparse_document_page_objects(doc, pages, len, flags, &out)` | Unfiltered content-stream snapshot: kinds, matrices, PDFium y-up bounds, form children, path segments, image metadata. `flags` selects `LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW` / `_DECODED` / `_BITMAP` payloads. |

Page selections are validated against the document's page count (any page
outside it is `LITEPARSE_STATUS_INVALID_ARGUMENT`), de-duplicated, and
processed in ascending order. With `LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR` a
page whose render or extraction fails is skipped; argument errors are never
skipped. Configured orientation corrections apply to every operation.

## Caller-supplied content

`liteparse_parser_parse_content` parses pages the host already extracted:
no conversion, PDFium, screenshots, or OCR. Start from
`liteparse_content_init` and fill the same `LiteParseContent` layout a result
exposes; every view and array is copied during the call.

```c
static const char pool[] = "hello";  /* input pools need no NUL bytes */

LiteParseTextItem item = {0};
item.text.offset = 0;
item.text.len = 5;
item.x = 72.0f;
item.y = 100.0f;
item.width = 80.0f;
item.height = 12.0f;

LiteParsePage page = {0};
page.page_number = 1;
page.width = 612.0f;
page.height = 792.0f;
page.item_count = 1;

LiteParseContent content;
liteparse_content_init(&content);
content.pool = (const uint8_t *)pool;
content.pool_len = sizeof pool - 1;
content.pages = &page;
content.pages_len = 1;
content.items = &item;
content.items_len = 1;
LiteParseResult *result = NULL;
LiteParseStatus status = liteparse_parser_parse_content(parser, &content, &result);
```

With no block ranges the pages run spatial projection (and the configured
classifier). Any per-page `block_offset/count` or a document-level block
range makes those blocks the Markdown structure, including `merged_table`
cells with `colspan`/`rowspan`; `images` and per-page complexity are
forwarded on that path. Annotations, form fields, structure trees (absolute
`parent_index`, parents first), vector graphics, and content bounds are
accepted when the page's `HAS_*` flag is set. Unknown `flags` bits,
non-finite floats, and string ranges outside `pool` or not valid UTF-8 in
any record are `LITEPARSE_STATUS_INVALID_ARGUMENT`.
`max_pages` truncates the
supplied list and drops document-level blocks when it does. `crop_box` and
`skip_diagonal_text` apply before projection, matching `document_parse`.

## OCR callback

`liteparse_parser_set_ocr_callback` installs an in-process OCR engine. The
callback receives a `LiteParseOcrImage` (RGB, or grayscale when registered
with `LITEPARSE_OCR_FLAG_PREFERS_GRAYSCALE`) and submits `LiteParseOcrWord`s
through `liteparse_ocr_sink_add`; on failure it records a message with
`liteparse_ocr_sink_set_error(sink, bytes, len)` and returns nonzero. It may run concurrently
on worker threads and must be thread-safe, non-unwinding, and valid for the
parser's lifetime. Documents opened before a callback change keep the engine
they were opened with.

## Tests

`cargo test -p liteparse-c` runs the Rust-side ABI tests and, when a C
compiler is on `PATH`, compiles `tests/header_smoke.c` with
`-std=c11 -Wall -Wextra -Werror -pedantic` against the checked-in header and
runs it on the fixtures in `integration_tests_data/`. Set
`LITEPARSE_REQUIRE_SMOKE=1` to fail instead of skipping when either is
missing.
