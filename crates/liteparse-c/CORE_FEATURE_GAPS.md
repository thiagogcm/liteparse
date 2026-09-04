# Core Feature Gaps in the C Bindings

The C bindings cover the primary document workflow: opening paths or byte
buffers, converting supported formats, parsing selected pages, running OCR,
rendering screenshots, calculating complexity, searching text, and reading
typed parse results.

They do not currently expose every entry point or extension mechanism in the
Rust core. This document describes those gaps, the use cases affected by each
one, and possible C API shapes for closing them.

## Coverage summary

| Core capability | Current C coverage | Impact |
| --- | --- | --- |
| Parse a path or byte buffer | Complete | Normal document parsing is covered. |
| Select pages for parsing, rendering, or complexity analysis | Complete | Explicit page arrays replace the core `target_pages` string. |
| Read text, Markdown, images, screenshots, metadata, annotations, forms, structure trees, blocks, vectors, and XFA | Broadly covered | Most result data has typed accessors. |
| Supply a custom OCR engine | Complete through a callback | Foreign runtimes can provide their own OCR implementation. |
| Parse caller-supplied pages | Missing | External text extractors cannot reuse LiteParse projection directly. |
| Parse caller-supplied blocks | Missing | Native document parsers cannot preserve their semantic blocks through LiteParse. |
| Use a bounded batch-session cursor | Partially covered | Callers can request ranges manually, but do not get core session semantics. |
| Supply an arbitrary glyph resolver | Partially covered | A font database directory is supported, but a host callback is not. |
| Configure `max_pages = 0` | Missing value representation | Zero selects the native default instead of requesting an empty parse. |

## Parsing workflows

The three core parsing paths operate at different levels of abstraction:

```text
Normal parse
file/bytes -> conversion -> PDFium extraction -> optional OCR
           -> grid projection -> block classification -> result

parse_from_pages
caller pages + positioned text -> grid projection
                               -> optional block classification -> result

parse_from_blocks
caller pages + positioned text + semantic blocks
                               -> grid projection for plain text
                               -> supplied blocks for Markdown -> result
```

## `parse_from_pages`

### What it does

`LiteParse::parse_from_pages` accepts pages that have already been extracted.
Each page can contain:

- Its source page number and viewport dimensions.
- Text items with text, bounding boxes, rotation, and font information.
- Optional vector graphics and image references.
- Optional PDF structure nodes, annotations, forms, geometry, and content
  bounds.

LiteParse runs its spatial grid projection over those text items. This
reconstructs lines, spacing, columns, and reading order and produces the plain
text for each page.

If Markdown output or public layout blocks are requested, LiteParse also runs
its own block classifier. That classifier determines headings, paragraphs,
lists, tables, code blocks, rules, and figures from the supplied page data.

The method returns an ordinary `ParseResult`, but it does not open a PDF, run
format conversion, extract content with PDFium, or invoke OCR.

### Use cases

- A host application already uses another PDF engine but wants LiteParse's
  spatial projection and Markdown output.
- A proprietary extractor has better font recovery and can supply accurate
  text boxes to LiteParse.
- A document-processing pipeline caches low-level extraction results and wants
  to rerun layout without reopening the original document.
- Tests or tools need to exercise projection independently from PDFium.

### Behavioral difference to note

The current Rust implementation processes every supplied page. Unlike the
normal parse and `parse_from_blocks`, it does not apply `target_pages` or
`max_pages`.

### Possible C API

A C API would need input views for pages, text items, graphics, structure
nodes, image references, annotations, and related nested data. All input data
should be copied during the call so that the caller does not need to retain
the buffers after the function returns.

A minimal first version could accept only page dimensions and text items,
leaving the optional inputs for later ABI extensions.

## `parse_from_blocks`

### What it does

`LiteParse::parse_from_blocks` accepts both low-level page content and a block
model already produced by the caller. Supported block concepts include:

- Headings
- Paragraphs
- Ordered and unordered list items
- Tables, rows, cells, and merged cells
- Code blocks
- Grid fallbacks
- Horizontal rules
- Figures

LiteParse still runs grid projection over the supplied text items to produce
plain page text. It does not reclassify the supplied blocks. Instead, it uses
them as the authoritative structure for per-page and document-level Markdown.

The caller may also provide:

- A document-wide block list, allowing structure to span page boundaries.
- An outline.
- Extracted images.
- Per-page complexity statistics.

This path applies `target_pages` and `max_pages`. If pages are filtered, the
document-wide block list is discarded because it may no longer align with the
remaining pages.

### Use cases

- A native DOCX parser already knows which content is a heading, list, table,
  or merged cell and should not lose that information through PDF conversion.
- A PPTX or spreadsheet extractor has semantic regions that are more accurate
  than geometry-based inference.
- Another layout model has already classified a PDF and LiteParse is used only
  to normalize results and render its output formats.
- A pipeline needs stable, caller-controlled Markdown rather than heuristic
  reclassification.

### Possible C API

This needs all of the `parse_from_pages` input structures plus a recursive or
flattened block representation. A flattened representation is usually easier
to keep ABI-safe: parent indexes and offset/count pairs can represent nested
blocks, table rows, and cells without exposing Rust ownership.

Because the C result API already exposes flattened layout blocks, rows, and
cells, the input model could reuse similar conventions where practical.

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

This gap is less severe than the page/block input gaps because callers can
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

The C tests cover the principal parse flow, page selection, OCR callbacks,
screenshots, complexity, search, text metadata, annotations, forms, page
geometry, concurrency, and compilation of the generated header from C.

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

1. Expose `parse_from_blocks` if native Office or external layout integration
   is planned.
2. Expose `parse_from_pages` for external text-extraction interoperability.
3. Add populated-result tests for complex typed result structures.
4. Add a batch-session API if consumers want cursor-based streaming rather
   than manual page ranges.
5. Add a glyph-resolver callback only when a host implementation needs it.
6. Make `max_pages = 0` representable as part of the next configuration ABI
   revision.
