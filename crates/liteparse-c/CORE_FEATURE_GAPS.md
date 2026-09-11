# Core Feature Gaps in the C Bindings

The C bindings cover the primary document workflow: opening paths or byte
buffers, converting supported formats, parsing selected pages, running OCR,
rendering screenshots, calculating complexity, extracting raw text runs,
extracting pre-projection pages, snapshotting unfiltered page objects,
searching text, and reading typed parse results. Caller-supplied pages and
layout blocks are accepted through one parser-level packed content API.

They do not currently expose every entry point or extension mechanism in the
Rust core. This document describes those remaining gaps.

## Coverage summary

| Core capability | Current C coverage | Impact |
| --- | --- | --- |
| Parse a path or byte buffer | Complete | Normal document parsing is covered. |
| Select pages for parsing, rendering, complexity, raw text, extract, or page objects | Complete | Explicit page arrays replace the core `target_pages` string. |
| Extract heuristic-free text runs | Complete through `liteparse_document_raw_text` | Hosts that own segmentation get pdfium glyphs without projection, including baseline-aware angles, tight grounding bounds, generated-space gaps, and visible form-widget appearance text. |
| Extract pre-projection pages and images | Complete through `liteparse_document_extract` | Hosts inspect heuristic items, graphics, and extras, or feed `liteparse_extract_as_content` into `parse_content`. |
| Walk unfiltered page content objects | Complete through `liteparse_document_page_objects` | Hosts reproduce another extractor's form recursion and matrix rules; extract graphics stay the filtered viewport view. |
| Read text, Markdown, images, screenshots, metadata, annotations, forms, structure trees, blocks, vectors, and XFA | Broadly covered | Most result data has typed accessors. |
| Supply a custom OCR engine | Complete through a callback | Foreign runtimes can provide their own OCR implementation. |
| Parse caller-supplied pages and blocks | Complete through `liteparse_parser_parse_content` | External extractors and native Office parsers reuse projection and Markdown without opening a PDF. |
| Use a bounded batch-session cursor | Partially covered | Callers can request ranges manually, but do not get core session semantics. |
| Supply an arbitrary glyph resolver | Partially covered | A font database directory is supported, but a host callback is not. |
| Configure `max_pages = 0` | Missing value representation | Zero selects the native default instead of requesting an empty parse. |

## Parsing workflows

The parsing paths operate at different levels of abstraction:

```text
Normal parse
file/bytes -> conversion -> PDFium extraction -> optional OCR
           -> grid projection -> block classification -> result

liteparse_document_extract
file/bytes -> conversion -> PDFium extraction
           (no OCR, projection, or Markdown)

liteparse_document_page_objects
file/bytes -> conversion -> unfiltered page-object tree
           (no path_objects / image_objects policy)

liteparse_parser_parse_content (no blocks)
caller pages + positioned text -> grid projection
                               -> optional block classification -> result

liteparse_parser_parse_content (any block range)
caller pages + positioned text + semantic blocks
                               -> grid projection for plain text
                               -> supplied blocks for Markdown -> result
```

## Caller-supplied content

`liteparse_parser_parse_content` is the C entry point for both core paths.
The caller fills a `LiteParseContent` packed the same way results are read:
pages hold offset/count ranges into shared text-item, graphic, and layout-block
arrays. Views are copied during the call.

- Empty block ranges run `parse_from_pages`: projection, and classification
  when Markdown or `extract_blocks` is configured.
- Any per-page or document-level block range runs `parse_from_blocks`:
  projection still produces page text, but Markdown comes from the supplied
  structure. `merged_table` cells carry `colspan` / `rowspan`.
- `max_pages` truncates the supplied page list on both paths and drops
  document-level blocks when it does. C does not invent `target_pages`.
- `crop_box` and `skip_diagonal_text` are applied before projection.
- Word boxes, struct-tree nodes (`H1`–`H6` via `mcid`), and image refs are
  accepted so an extract snapshot can re-enter parse.
- Logical structure trees, decoded image payloads, complexity stats,
  annotations, and forms are not accepted on this path yet;
  `size_of_content` is the ABI extension point.

## Bounded batch sessions

The Rust core exposes `open_batch_session` and `next_batch`. A session resolves
or converts the input once, records the outline and total page count, enforces
the document-wide `max_pages` limit, and advances an internal page cursor.

The C document handle already avoids repeating non-PDF conversion and lets a
caller request any explicit page range. Therefore, bounded-memory parsing is
possible today by manually dividing the document into ranges.

What is missing is the core's session behavior:

- Automatic cursor progression.
- Explicit start and end page metadata for each batch.
- A definitive end-of-session result.
- Session-wide enforcement of `max_pages`.
- A default batch size consistent with the core.

### Use cases

- Processing very large PDFs without materializing every parsed page at once.
- Streaming batches to another service or persistence layer.
- Implementing backpressure in Java, .NET, Go, or other foreign runtimes.

This gap is less severe than it first appears because callers can
implement the behavior with `liteparse_document_total_pages` and repeated
`liteparse_document_parse` calls.

## Generic glyph resolver callbacks

The Rust core accepts any implementation of `GlyphResolver`. It invokes the
resolver when built-in cmap and Adobe Glyph List recovery cannot decode an
untrusted glyph, passing the glyph's vector-outline segments to the resolver.

The C bindings currently support the core `FontDbResolver` through
`LiteParseConfig.font_db_dir`, but they cannot call a resolver implemented by
the host runtime.

### Use cases

- A host maintains a proprietary glyph-outline database.
- Glyph recovery is implemented in Java, .NET, or another native library.
- A consumer wants telemetry or fallback behavior around unresolved glyphs.

A callback could follow the existing OCR callback design. It would receive a
borrowed array of `(segment_type, x, y)` values and write the resolved UTF-8
text into a caller-provided sink or through a two-call size/buffer protocol.
Callback concurrency and lifetime requirements would need to be documented.

## `max_pages = 0`

The Rust configuration allows `max_pages` to be zero. The extraction loop then
returns no parsed pages. The C configuration reserves zero to mean "keep the
native default," so this core value cannot be selected.

### Use cases

This is mainly a consistency and boundary-value issue. It can be useful for:

- Reading document-level information without parsing a page.
- Testing empty-result behavior.
- Passing through a caller configuration without changing its semantics.

The ABI could represent this with a `has_max_pages` flag, or reserve a separate
sentinel value instead of zero.

## Test coverage gaps

The C tests cover the principal parse flow, page selection, caller-supplied
content (projection, supplied blocks, `max_pages`, merged tables), OCR
callbacks, screenshots, complexity, search, text metadata, annotations, forms,
page geometry, unfiltered page objects, concurrency, and compilation of the
generated header from C.

Some complex result packers are currently tested only in their disabled or
empty states:

- Populated structure trees and their nested attributes and annotations.
- Populated layout blocks, table rows, cells, and source lines.
- Populated vector shapes and lines.
- Extracted image payloads and duplicate-image relationships.
- Populated XFA packet data.

Positive fixtures for these paths would validate the offset/count relationships
and borrowed view lifetimes that are most likely to fail in foreign runtimes.

## Suggested priority

1. Add populated-result tests for complex typed result structures.
2. Add a batch-session API if consumers want cursor-based streaming rather
   than manual page ranges.
3. Add a glyph-resolver callback only when a host implementation needs it.
4. Make `max_pages = 0` representable as part of the next configuration ABI
   revision.
