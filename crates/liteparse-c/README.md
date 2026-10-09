# LiteParse C bindings

`liteparse-c` exposes LiteParse through a C ABI for foreign runtimes. Results
use flat arrays of pointer-free records, with view structs and shared string
and binary arenas. The checked-in header `include/liteparse.h` is generated
with [cbindgen](https://github.com/mozilla/cbindgen).

| Handle | Created by | Used with / read through |
|---|---|---|
| `LiteParseParser` | `liteparse_parser_new` | document opening and `liteparse_parser_parse_content` |
| `LiteParseDocument` | `liteparse_document_open_path` / `_open_bytes` | `liteparse_document_info` |
| `LiteParseJob` | `liteparse_document_begin` | `liteparse_job_render_ocr`, `_merge_ocr`, `_finish` |
| `LiteParseResult` | `liteparse_document_parse`, `_extract`, `liteparse_job_finish`, `liteparse_parser_parse_content` | `liteparse_result_view` |
| `LiteParseScreenshots` | `liteparse_document_screenshot` | `liteparse_screenshots_view` |
| `LiteParseComplexity` | `liteparse_document_complexity` | `liteparse_complexity_view` |
| `LiteParseRawText` | `liteparse_document_raw_text` | `liteparse_raw_text_view` |
| `LiteParsePageObjects` | `liteparse_document_page_objects` | `liteparse_page_objects_view` |

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
    const uint8_t *pool = view->content.arenas.pool;
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
(`LiteParseContentArrays`, `LiteParseResultView`, and the other `*View` and
`*Info` structs, which hold the array pointers and lengths), and **inputs**
(`LiteParseConfig`, `LiteParseContentInput`, `LiteParseRenderRegion`,
`LiteParseOcrWord`). The layout is designed to be read by foreign consumers
without helper code: records hold no pointers, and a whole result can be
copied out of the handle with one `memcpy` per array.

- **Records are pointer-free.** Records hold fixed-width scalars, nested records, string references, and `LiteParseBlobRef` ranges into the owning input or output binary arena. There are no `bool` or `size_t` fields in records. Optional values and boolean properties use record flags.
- **Collections are flat arrays with ranges.** A handle owns one flat array
  per record type; parents carry `uint32_t` `x_offset`/`x_count` pairs into
  it. Index fields such as `parent_index` are absolute within their array,
  with `LITEPARSE_NO_PARENT` for roots.
- **Array views start with `LiteParseArenas`**: the string pool, binary
  arena, and page errors from tolerant processing. `LiteParseResultView`
  exposes it as `content.arenas`; screenshot, complexity, raw-text,
  page-object, and OCR raster views expose it as `arenas`. This shared
  prefix lets consumers copy the arenas and page errors uniformly.
- **Strings live in a pool.** Every string in a view's records and scalars
  is a `LiteParseStr {offset, len}` into `arenas.pool`. Output pools start
  with a NUL byte and NUL-terminate every string, so `pool + offset` is a
  valid C string and `{0, 0}` reads as `""`. Absent and empty strings are
  both `len == 0`. Input pools (`liteparse_parser_parse_content`) need only
  the ranges to be in bounds and valid UTF-8.
- **Results are bounded.** A result's arrays and arenas total at most
  `LITEPARSE_MAX_RESULT_BYTES` (1 GiB), so every offset, count, and arena
  length fits in `int32_t`. Packing checks the budget before copying each
  binary payload and after each page; a larger result returns
  `LITEPARSE_STATUS_RESOURCE_LIMIT` and publishes nothing.
- **Views are borrowed.** Result handles expose a `*_view` struct of
  `{ptr, len}` array pairs and scalars, valid until the matching `*_free`.
  `liteparse_job_render_ocr` returns a borrowed OCR raster view that remains
  valid until the next job call. Empty arrays are a null pointer with zero
  length. Since records are pointer-free, copying the arrays and arenas out
  of a view leaves them readable after freeing the handle.
- **Byte buffers** in records use `LiteParseBlobRef {offset, len}` into `arenas.binary`. Direct byte-view outputs (version and errors) and configuration strings use `LiteParseByteView`. Empty byte views have a null pointer and zero length. Colors are packed ARGB `uint32_t`; enumerations are `uint32_t` constants.
- **Function arguments** never pass structs by value: caller strings are
  `(const uint8_t *, size_t)` pairs, not NUL-terminated, and everything
  else is a pointer, scalar, or out pointer.
- **Creation** takes an out pointer and returns a `LiteParseStatus`; the out
  pointer receives null on failure. `liteparse_last_error(&view)` then
  yields a thread-local message valid until the next failed call on the
  same thread: read it on the thread that made the failing call, before any
  other call that may fail. `LITEPARSE_STATUS_PANIC` means a Rust panic was
  caught at the boundary: free the handle involved and do not reuse it.
  Missing or unloadable PDFium returns
  `LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE` as an expected failure.
  Explicit page lists longer than `LITEPARSE_MAX_SELECTED_PAGES` return
  `LITEPARSE_STATUS_RESOURCE_LIMIT` before the list is copied.
- **Fallible outputs** require a non-null out pointer. A null pointer returns
  `LITEPARSE_STATUS_INVALID_ARGUMENT`; byte-view outputs are cleared on other
  failures. Null remains valid for free functions and void output helpers.
- **Layout introspection.** `liteparse_abi_descriptor` reports pointer width, endianness, compiled capabilities, resource limits, and a layout fingerprint. The fingerprint hashes the generated header; `liteparse_abi.h` (included by `liteparse.h`) publishes it as `LITEPARSE_ABI_FINGERPRINT`, so a program can compare the two to detect a library built from a different header. `liteparse_sizeof(LITEPARSE_TYPE_*)` reports individual struct sizes.
- **Configuration** starts with `liteparse_config_init`, which writes actual defaults. Set or clear `options` bits for boolean values; unknown bits are invalid. Core processing never writes progress output, and every text item carries its word boxes: neither is configurable. `max_pages = 0` selects no pages; DPI must be finite and positive. `struct_size` must equal `sizeof(LiteParseConfig)`. Parser creation copies all input views.
- **Threads.** Parser and document handles may be used from several threads
  at once; destruction must wait for in-flight operations. All ABI calls are
  synchronous and block until the operation completes. Hosts should dispatch
  them through a blocking-work executor when their scheduling model requires
  it.

## Consuming packed views

The packed layout needs no per-string native calls:

1. Assert every hand-declared struct size against
   `liteparse_sizeof(LITEPARSE_TYPE_*)` during binding initialization.
2. Pass caller strings as pointer/length pairs; no temporary C string is
   required.
3. After `liteparse_document_parse`, read `liteparse_result_view` once,
   copy the required arrays and `content.arenas`, then free
   the result. The copied records stay valid because they contain only
   offsets.
4. Resolve each binary reference from `arenas.binary` using its offset and
   length.
5. For `liteparse_parser_parse_content`, build one byte pool, record each
   string's offset and length as you append, and pass the pool with the
   record arrays; the library copies everything during the call.

## Results

`LiteParseResultView.content` is a `LiteParseContentArrays`: the arenas
(string pool, binary arena, page errors), source page count, and optional
document-wide block range, plus pages, text items, word boxes, char codes,
layout graphics, struct nodes, marked-content ids, image refs, images,
annotations, quadpoints, form fields, option strings, structure nodes and
attributes, blocks, cells, rows, vector shapes and lines, and outline. The
same struct is `LiteParseContentInput.content`. The remaining
view fields are result-only: page outputs, document text, creator, producer,
`doc_meta`, `form_type`, screenshots and their rects, XFA packets, figure
rects, item frames, projected lines and spans, and the XY-cut region tree.

`LiteParsePage` carries a range into every page-scoped content array, the
page label, geometry (visible box, user unit, rotation), content bounds, and
inline complexity. Its `HAS_*` flags distinguish "extraction enabled, none
found" from "disabled". A page whose visible box is empty, such as one whose
crop box misses its media box, is still a page: it is extracted and parsed
with zero width and height, an empty visible box, and whatever content lies
inside that box (normally none). It has no pixels, so OCR skips it and a
screenshot of it fails as a page error. `page_outputs[i]` holds the text, Markdown (under
`LITEPARSE_OUTPUT_FORMAT_MARKDOWN`), and figure, item-frame, projected-line,
and region ranges of `content.pages[i]`; extract-only results have no page
outputs. Every string in the result is a `LiteParseStr` range into
`content.arenas.pool`.

Text metadata (font metrics, colors, char codes, marked-content ids) is
always exported when the core holds it; `LITEPARSE_RESULT_FLAG_TEXT_METADATA`
reports whether it was requested.

Struct nodes and structure trees are per page. A page's tree holds the
tagged elements with content on it and their ancestors, and every element's
marked-content ids are those of its content on that page: an MCID only names
content within one page, so an element that spans pages appears on each with
that page's ids. Search and JSON serialization are left to
the caller, which has every text item and field in the arrays.

`liteparse_document_extract` returns a result with
`LITEPARSE_RESULT_FLAG_EXTRACT_ONLY`: pre-projection pages with heuristic
text items, graphics, and the configured extras, but no page outputs, text,
Markdown, or projection. Links are stamped as parse stamps them, only under Markdown, and
every text item carries its word boxes.

Parse and extract results report how extraction recovered form content.
`LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS` and `flattened_page_numbers`
report pages flattened on a temporary document to recover widget text.
`LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM` reports that extraction read an
AcroForm-repaired copy: with form-field extraction on, a PDF whose page
widgets are orphaned from a missing `/AcroForm` is rewritten in memory with
one adopting their fields, and everything extracted (text, form fields, the
form type) comes from that copy. The repair is the document's, so the flag
does not depend on the selection; `repaired_page_numbers` names the result's
pages holding the adopted widgets. Results of `liteparse_parser_parse_content`
report neither.

Projected lines index `projected_spans` (text-item records sharing the
content's `words` and `char_codes`) and `region_paths`. Regions are
flattened in pre-order with absolute `parent_index` and child ranges; a
line's region path is the sequence of child ordinals from its page's root.
`item_frames` map projected geometry back to page space on pages where
rotation handling displaced content.

## Operations on a document

| Function | Notes |
|---|---|
| `liteparse_document_info` | Page count, `LITEPARSE_DOCUMENT_FLAG_CONVERTED`, and bookmarks walked at open, each listed once: an outline whose links loop ends. |
| `liteparse_document_parse(doc, pages, len, &out)` | Parse the given 1-based pages, or every page when `pages` is null with zero length. `max_pages` caps either. Blocks are classified against the signals of the pages parsed. |
| `liteparse_document_parse_with_document_signals(doc, pages, len, &out)` | Parse the given pages as pages of the whole document; see Document signals. |
| `liteparse_document_extract(doc, pages, len, &out)` | Pre-projection pages with source page count, outline, and page geometry; set `input.content = view->content` to project and classify with `liteparse_parser_parse_content`. |
| `liteparse_document_screenshot(doc, pages, len, dpi, region, &out)` | Render PNGs. `0` keeps the configured DPI. A non-null `region` (viewport points, top-left origin) renders only that part of each page, at the size and scale of the matching crop of a whole-page render; glyph anti-aliasing can differ slightly from the crop. Rectangle detection renders the whole page, then clips detected rects and makes them region-relative. |
| `liteparse_document_complexity(doc, pages, len, &out)` | Cheap pre-OCR signals per page; tolerant failures are page errors. |
| `liteparse_document_begin(doc, pages, len, &out)` | Start a parse whose OCR the host performs; see Staged OCR. |
| `liteparse_document_raw_text(doc, pages, len, &out)` | Heuristic-free PDFium runs: no gap merge, projection, OCR, or Markdown. Items carry baseline-aware angles, tight grounding bounds, generated-space gaps, and visible form-widget appearance text. Rectangular-clip visibility follows the extraction rules below. |
| `liteparse_document_page_objects(doc, pages, len, flags, &out)` | Unfiltered content-stream snapshot: kinds, matrices, PDFium y-up bounds, form children, path segments, clip stacks, image metadata. `flags` selects `LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW` / `_DECODED` / `_BITMAP` payloads. `LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_*_UNAVAILABLE` marks requested payloads PDFium did not return. |

Parse, extract, staged jobs, complexity, and raw text omit glyphs fully hidden by known rectangular PDF graphics-state clips. A partially clipped glyph is retained with its complete text and geometry when its ink box, widened by 2% of its em, meets the clip; a glyph without ink is judged by its loose box. Nested forms compose ancestor matrices and inherit ancestor clips. Unsupported clips preserve source text, including invisible OCR layers.

The page-object snapshot remains unfiltered. Each object's `clip_path_offset/count` indexes `LiteParsePageObjectsView.clip_paths`; each `LiteParseClipPath.segment_offset/count` indexes the shared `segments` array independently of the object's own path range. Clip points are y-up coordinates in the containing form (page coordinates at top level), with the object's own matrix already applied: apply only ancestor form matrices to reach page space. Paths retain curves and compound subpaths, not just rectangular bounds. PDFium does not expose text-based clipping or clip fill rules. `LITEPARSE_PAGE_OBJECT_FLAG_HAS_CLIP_PATHS` distinguishes a successfully read empty clip stack from unavailable APIs, failed reads, or clip stacks exceeding LiteParse's 1,024-segment clip-read limit across the entire stack. An unavailable stack has zero clip paths; it does not prove that the object is unclipped.

Complexity's `LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED` includes substantial failed Unicode mappings as well as substitution-cipher text. A stray unmappable symbol on an otherwise healthy page does not set it. The C pipeline uses the core's same OCR-selection predicates.

Dedicated screenshots and parsing with screenshot extraction preflight each
planned PDFium bitmap: the region alone for a region render without rectangle
detection, the full page otherwise. A bitmap is limited to 64 Mi pixels, and
the sum of planned RGBA bitmap sizes in one operation is limited to 256 MiB.
These are work budgets, not bounds on peak live memory (PDFium, cropped
copies, and PNG encoding can coexist). Exceeding either returns
`LITEPARSE_STATUS_RESOURCE_LIMIT` before the oversized bitmap is allocated.

### Document signals

Block classification needs two facts no page holds alone: the document's body
font size, which heading levels are ranked against, and the lines that repeat
over its pages, which are its running headers and footers. A parse computes
these signals over the pages it is given, so `liteparse_document_parse` of a
selection ranks headings against the selection, and the same page can come
out with other heading levels than in a parse of every page.

`liteparse_document_parse_with_document_signals` classifies the selected
pages against the signals of the whole document instead: each page is the
page a parse of every page holds. The signals are those of the pages such a
parse reads (`max_pages` caps them; a selected page past the cap is
classified against them too). The first call on a document reads and projects
those pages once, without the source metadata, marked-content scoping,
complexity, screenshot, or packing work of a parse, and with the configured
OCR when it is enabled, since recognized text is part of what the signals are
read from; the document keeps them for every later call. The signals cost one
read of every page and each selection is then read again, so when every page
is wanted, parse every page in one call.

- That first read is of the whole document. Where tolerant processing is off,
  a page outside the selection that cannot be read fails the call; a pass that
  fails is not kept, and the next call reads the document again.
- The signals are kept as the first pass found them. Where OCR failures are
  not fatal, a page an engine failed on in that pass adds no text to them.
- What a result names beyond its pages stays the selection's own. Under
  Markdown, an image drawn again from a page outside the selection is
  referenced as the selection's image, which the result holds, not as the
  earlier one a parse of every page would name.
- A selection where nothing is classified (no blocks and no Markdown) is a
  plain parse, and so is no selection.
- `liteparse_document_begin` has no such form: a staged parse is classified
  against its own pages, with the text the host recognized.

Opening a document reads its page count and outline only. The source file's
metadata (`LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA`) is read by the first
parse on the document and kept: resolving the catalog's XMP loads every object
of the file, which on a file of a few megabytes with many objects takes
seconds, more than parsing it. Leave the flag off on a parser whose results do
not need it.

Page selections are validated against the document's page count (any page
outside it is `LITEPARSE_STATUS_INVALID_ARGUMENT`), de-duplicated, and
processed in ascending order. With `LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR` a
page whose render or extraction fails is skipped; argument errors are never
skipped. Configured orientation corrections apply to every operation.

For requested raw and decoded image data, PDFium reports zero for both an
empty source stream and an extraction failure, so both are marked
unavailable. An empty payload alone does not indicate whether extraction was
requested or succeeded; the availability flags do.

Page-object traversal retains at most 100,000 objects and descends at most 32
Form XObject levels. Exceeding either limit returns
`LITEPARSE_STATUS_RESOURCE_LIMIT` rather than a partial object tree.

## Caller-supplied content

`liteparse_parser_parse_content` parses pages the host already extracted:
no conversion, PDFium, screenshots, or OCR. Start from
`liteparse_content_init` and fill `LiteParseContentInput.content`, the same
`LiteParseContentArrays` a result exposes, so extracted or parsed content
passes back unchanged. Images reference `content.arenas.binary` by offset and
length; only referenced payloads are copied. Input pools need neither a
leading NUL nor terminators. Input arrays and referenced payloads are copied
during the call.

`content.total_pages` is the source document page count before page
selection. Every page number, outline target, and page error must lie within
it, so content with pages needs a non-zero count. Supplied outline, page geometry,
images, complexity, and page errors survive projection. Page width and
height must be finite and non-negative. A flagged page geometry must have
finite ordered box edges (an empty box, as a zero-area page reports, is
ordered), positive user unit, and a quarter-turn rotation from zero through
three.

Caller-supplied content is limited to 100,000 pages, 256 MiB of flat input
arrays (including its string pool), and 256 MiB of copies. Ranges may alias,
so each copy is charged against that budget before it is allocated. Image
payloads are limited to 32 MiB each and 128 MiB in total, checked before any
payload is copied. Exceeding a limit returns
`LITEPARSE_STATUS_RESOURCE_LIMIT` without publishing a result.

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

LiteParseContentInput input;
liteparse_content_init(&input);
input.content.total_pages = 1;
input.content.arenas.pool = (const uint8_t *)pool;
input.content.arenas.pool_len = sizeof pool - 1;
input.content.pages = &page;
input.content.pages_len = 1;
input.content.items = &item;
input.content.items_len = 1;
LiteParseResult *result = NULL;
LiteParseStatus status = liteparse_parser_parse_content(parser, &input, &result);
```

With no block ranges the pages run spatial projection (and the configured
classifier). Any per-page `block_offset/count` or a document-level block
range makes those blocks the Markdown structure, including `merged_table`
cells with `colspan`/`rowspan`, so content taken from a parse result with
extracted blocks keeps those blocks; `images` and per-page complexity are
forwarded on that path. Annotations, form fields, structure trees (absolute
`parent_index`, parents first), vector graphics, and content bounds are
accepted when the page's `HAS_*` flag is set. Unknown `flags` bits,
non-finite floats, and string ranges outside the pool or not valid UTF-8 in
any record are `LITEPARSE_STATUS_INVALID_ARGUMENT`.
`max_pages` truncates the supplied list and drops document-level blocks when
it does. `crop_box` and
`skip_diagonal_text` apply before projection, matching `document_parse`.

## Staged OCR

The host performs OCR between native calls; the library never calls back
into it.

1. `liteparse_document_begin(doc, pages, len, &job)` runs extraction and
   configured pre-OCR work for the same page selection, then releases
   PDFium. Projection and classification happen in `liteparse_job_finish`,
   after OCR. The job never runs the configured OCR engine and may outlive
   the document.
2. `liteparse_job_render_ocr(job, pages, len, max_rasters, pixel_format,
   &view)` renders the next round of at most `max_rasters` pages (zero uses
   `num_workers`, and rounds shrink to fit the 256 MiB raster budget) in
   `LITEPARSE_OCR_PIXEL_FORMAT_RGB` or `_GRAYSCALE`. Rounds move forward
   through the job's pages. The core OCR predicates choose the pages; a
   non-empty `pages` list names job pages and renders only those not yet
   passed. A page with no area is never rendered. Repeat the same list on every call until an empty round. Each
   `LiteParseOcrRaster` carries its page number, dimensions, effective DPI,
   a native-text flag, embedded image rectangles,
   and tightly packed pixels in `view->arenas.binary`. Render failures under
   `LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR` appear in `view->arenas.page_errors`.
   The view is valid until the next job call. An empty round means OCR is
   complete; a failed call leaves the job unchanged.
3. `liteparse_job_merge_ocr(job, inputs, n, words, n, pool, len)` merges the
   round: one `LiteParseOcrPageInput` per recognized page, with a range of
   `LiteParseOcrWord`s in raster pixels whose text indexes `pool`, or a
   recognition error. A rendered page without an input recognized nothing. Boxes must have finite, ordered edges; confidence must
   be finite and within `[0, 1]`; a flagged polygon must form a
   nondegenerate, non-self-intersecting quadrilateral inside the raster.
   Every input is validated before the job changes. Each recognized word
   becomes a text item carrying its word box, as native text items always
   do. Errors become page
   errors; with `LITEPARSE_FLAG_OCR_FAILURE_FATAL` the merge fails with
   `LITEPARSE_STATUS_OCR_ERROR` when every page in the round failed and one
   had sparse native text, and the job then accepts only
   `liteparse_job_free`. A parse that runs the configured engine itself
   judges this once over all of its pages, as core parsing does, so it fails
   only when every recognized page failed.
4. `liteparse_job_finish(&job, &result)` consumes the job, even on failure,
   and returns the result `liteparse_document_parse` produces with the same
   recognition. A rendered round left unmerged receives no OCR.

Job calls require exclusive access to the job.

## Tests

`cargo test -p liteparse-c` runs the Rust-side ABI tests and, when a C
compiler is on `PATH`, compiles `tests/header_smoke.c` with
`-std=c11 -Wall -Wextra -Werror -pedantic` against the checked-in header and
runs it on the fixtures in `integration_tests_data/`. Set
`LITEPARSE_REQUIRE_SMOKE=1` to fail instead of skipping when either is
missing.
