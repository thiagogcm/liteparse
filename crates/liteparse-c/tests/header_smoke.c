/* Compiles the generated header with -std=c11 -Wall -Wextra -Werror -pedantic
 * and exercises every handle on the fixtures given as arguments. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "liteparse.h"

#define BYTES(s) ((const uint8_t *)(s)), strlen(s)

/* Every result view starts with its arenas. */
_Static_assert(offsetof(LiteParseResultView, content) == 0, "result arenas");
_Static_assert(offsetof(LiteParseContentArrays, arenas) == 0, "content arenas");
_Static_assert(offsetof(LiteParseScreenshotsView, arenas) == 0, "screenshot arenas");
_Static_assert(offsetof(LiteParseComplexityView, arenas) == 0, "complexity arenas");
_Static_assert(offsetof(LiteParseRawTextView, arenas) == 0, "raw-text arenas");
_Static_assert(offsetof(LiteParsePageObjectsView, arenas) == 0, "page-object arenas");
_Static_assert(offsetof(LiteParseOcrRasterView, arenas) == 0, "OCR raster arenas");

/* Resolve a pooled string; every output string is NUL-terminated, so the
 * result is a plain C string. */
static const char *pooled(const uint8_t *pool, size_t pool_len, LiteParseStr s) {
  if (pool == NULL || (size_t)s.offset + (size_t)s.len >= pool_len) return NULL;
  if (pool[(size_t)s.offset + (size_t)s.len] != 0) return NULL;
  return (const char *)pool + s.offset;
}

static int fail(const char *what) {
  LiteParseByteView error;
  liteparse_last_error(&error);
  fprintf(stderr, "%s: %.*s\n", what, (int)error.len,
          error.ptr ? (const char *)error.ptr : "");
  return 1;
}

static int check_str(const uint8_t *pool, size_t pool_len, LiteParseStr s, const char *what) {
  const char *text = pooled(pool, pool_len, s);
  if (text == NULL || strlen(text) != s.len) {
    fprintf(stderr, "%s: string %u+%u is not a NUL-terminated pool entry\n", what, s.offset,
            s.len);
    return 1;
  }
  return 0;
}

static int check_range(uint32_t offset, uint32_t count, size_t len, const char *what) {
  if ((size_t)offset + (size_t)count > len) {
    fprintf(stderr, "%s: range %u+%u outside %zu\n", what, offset, count, len);
    return 1;
  }
  return 0;
}

static int check_array(const void *ptr, size_t len, const char *what) {
  if ((ptr == NULL) != (len == 0)) {
    fprintf(stderr, "%s: null/len mismatch\n", what);
    return 1;
  }
  return 0;
}

static LiteParseParser *new_parser(uint64_t extra_flags, uint32_t output_format) {
  LiteParseConfig config;
  liteparse_config_init(&config);
  if (config.struct_size != sizeof(LiteParseConfig)) return NULL;
  if (LITEPARSE_DEFAULT_DPI < 149.0f || LITEPARSE_DEFAULT_DPI > 151.0f) return NULL;
  config.options |= extra_flags;
  if (!(extra_flags & LITEPARSE_FLAG_OCR_ENABLED)) config.options &= ~LITEPARSE_FLAG_OCR_ENABLED;
  config.output_format = output_format;
  LiteParseParser *parser = NULL;
  if (liteparse_parser_new(&config, &parser) != LITEPARSE_STATUS_OK) return NULL;
  return parser;
}

/* Every offset/count in a content view must land inside its array. */
static int check_content(const LiteParseContentArrays *c) {
  int bad = 0;
  bad |= check_array(c->pages, c->pages_len, "pages");
  bad |= check_array(c->items, c->items_len, "items");
  bad |= check_array(c->words, c->words_len, "words");
  bad |= check_array(c->annotations, c->annotations_len, "annotations");
  bad |= check_array(c->blocks, c->blocks_len, "blocks");
  bad |= check_array(c->outline, c->outline_len, "outline");
  for (size_t i = 0; i < c->items_len; i++) {
    bad |= check_range(c->items[i].word_offset, c->items[i].word_count, c->words_len, "item words");
    bad |= check_range(c->items[i].char_code_offset, c->items[i].char_code_count,
                       c->char_codes_len, "item char codes");
  }
  for (size_t i = 0; i < c->annotations_len; i++) {
    bad |= check_range(c->annotations[i].quadpoint_offset, c->annotations[i].quadpoint_count,
                       c->quadpoints_len, "quadpoints");
  }
  for (size_t i = 0; i < c->form_fields_len; i++) {
    bad |= check_range(c->form_fields[i].option_offset, c->form_fields[i].option_count,
                       c->strings_len, "options");
  }
  for (size_t i = 0; i < c->structure_nodes_len; i++) {
    const LiteParseStructureNode *n = &c->structure_nodes[i];
    bad |= check_range(n->mcid_offset, n->mcid_count, c->mcids_len, "structure mcids");
    bad |= check_range(n->attribute_offset, n->attribute_count, c->structure_attributes_len,
                       "structure attributes");
    if (n->parent_index != LITEPARSE_NO_PARENT && n->parent_index >= i) {
      fprintf(stderr, "structure parent must precede child\n");
      bad = 1;
    }
  }
  for (size_t i = 0; i < c->blocks_len; i++) {
    const LiteParseLayoutBlock *b = &c->blocks[i];
    bad |= check_range(b->line_offset, b->line_count, c->strings_len, "block lines");
    bad |= check_range(b->header_cell_offset, b->header_cell_count, c->cells_len, "header cells");
    bad |= check_range(b->row_offset, b->row_count, c->rows_len, "rows");
  }
  for (size_t i = 0; i < c->rows_len; i++) {
    bad |= check_range(c->rows[i].cell_offset, c->rows[i].cell_count, c->cells_len, "row cells");
  }
  for (size_t i = 0; i < c->pages_len; i++) {
    const LiteParsePage *p = &c->pages[i];
    bad |= check_range(p->item_offset, p->item_count, c->items_len, "page items");
    bad |= check_range(p->graphic_offset, p->graphic_count, c->graphics_len, "page graphics");
    bad |= check_range(p->struct_node_offset, p->struct_node_count, c->struct_nodes_len,
                       "page struct nodes");
    bad |= check_range(p->image_ref_offset, p->image_ref_count, c->image_refs_len,
                       "page image refs");
    bad |= check_range(p->annotation_offset, p->annotation_count, c->annotations_len,
                       "page annotations");
    bad |= check_range(p->form_field_offset, p->form_field_count, c->form_fields_len,
                       "page form fields");
    bad |= check_range(p->structure_node_offset, p->structure_node_count,
                       c->structure_nodes_len, "page structure nodes");
    bad |= check_range(p->block_offset, p->block_count, c->blocks_len, "page blocks");
    bad |= check_range(p->vector_shape_offset, p->vector_shape_count, c->vector_shapes_len,
                       "page vector shapes");
    bad |= check_range(p->vector_line_offset, p->vector_line_count, c->vector_lines_len,
                       "page vector lines");
  }
  return bad;
}

static int check_result(const LiteParseResult *result, int expect_text) {
  const LiteParseResultView *v = liteparse_result_view(result);
  if (v == NULL) return fail("result view");

  int bad = check_content(&v->content);
  if (v->content.pages_len == 0) return fail("no pages");
  const LiteParsePage *page = &v->content.pages[0];
  if (page->page_number != 1 || page->width <= 0.0f) return fail("page facts");
  const uint8_t *pool = v->content.arenas.pool;
  size_t pool_len = v->content.arenas.pool_len;
  if (pool == NULL || pool_len == 0 || pool[0] != 0) return fail("pool starts with NUL");
  bad |= check_str(pool, pool_len, page->label, "page label");
  bad |= check_str(pool, pool_len, v->text, "text");
  bad |= check_str(pool, pool_len, v->creator, "creator");
  for (size_t i = 0; i < v->content.items_len; i++) {
    bad |= check_str(pool, pool_len, v->content.items[i].text, "item text");
    bad |= check_str(pool, pool_len, v->content.items[i].font_name, "item font");
  }
  for (size_t i = 0; i < v->content.strings_len; i++) {
    bad |= check_str(pool, pool_len, v->content.strings[i], "strings");
  }
  if (expect_text) {
    if (v->page_outputs_len != v->content.pages_len) return fail("page outputs");
    if (v->text.len == 0 || v->page_outputs[0].text.len == 0) return fail("text");
    if (strlen(pooled(pool, pool_len, v->text)) != v->text.len) return fail("text NUL");
    if (v->flags & LITEPARSE_RESULT_FLAG_EXTRACT_ONLY) return fail("extract flag");
  } else if (!(v->flags & LITEPARSE_RESULT_FLAG_EXTRACT_ONLY) || v->page_outputs_len != 0) {
    return fail("extract-only shape");
  }
  int found_words = 0;
  for (size_t i = 0; i < v->content.items_len; i++) {
    if (v->content.items[i].word_count > 0) found_words = 1;
  }
  if (!found_words) return fail("word boxes");
  for (size_t i = 0; i < v->projected_lines_len; i++) {
    const LiteParseProjectedLine *line = &v->projected_lines[i];
    bad |= check_range(line->span_offset, line->span_count, v->projected_spans_len, "spans");
    bad |= check_range(line->region_path_offset, line->region_path_count, v->region_paths_len,
                       "region paths");
    if (line->anchor > LITEPARSE_ANCHOR_FLOATING) bad = fail("anchor");
  }
  for (size_t i = 0; i < v->regions_len; i++) {
    const LiteParseProjectedRegion *r = &v->regions[i];
    bad |= check_range(r->child_offset, r->child_count, v->region_children_len, "region children");
  }
  for (size_t i = 0; i < v->page_outputs_len; i++) {
    const LiteParsePageOutput *p = &v->page_outputs[i];
    bad |= check_range(p->projected_line_offset, p->projected_line_count, v->projected_lines_len,
                       "page lines");
    bad |= check_range(p->region_offset, p->region_count, v->regions_len, "page regions");
    bad |= check_range(p->figure_offset, p->figure_count, v->figures_len, "page figures");
  }
  for (size_t i = 0; i < v->screenshots_len; i++) {
    bad |= check_range(v->screenshots[i].rect_offset, v->screenshots[i].rect_count,
                       v->screenshot_rects_len, "screenshot rects");
  }
  return bad;
}

static int stage_parse(LiteParseDocument *document) {
  LiteParseResult *result = NULL;
  if (liteparse_document_parse(document, NULL, 0, &result) != LITEPARSE_STATUS_OK) {
    return fail("parse");
  }
  int bad = check_result(result, 1);
  liteparse_result_free(result);
  return bad;
}

static int stage_page_selection(LiteParseDocument *document) {
  const LiteParseDocumentInfo *info = liteparse_document_info(document);
  if (info == NULL || info->total_pages == 0) return fail("document info");
  if (check_array(info->outline, info->outline_len, "outline")) return 1;
  uint32_t last = info->total_pages;
  LiteParseResult *result = NULL;
  if (liteparse_document_parse(document, &last, 1, &result) != LITEPARSE_STATUS_OK) {
    return fail("parse last page");
  }
  const LiteParseResultView *v = liteparse_result_view(result);
  int bad = v->content.pages_len != 1 || v->content.pages[0].page_number != last;
  if (bad) fail("page selection");
  liteparse_result_free(result);
  uint32_t zero = 0;
  if (liteparse_document_parse(document, &zero, 1, &result) != LITEPARSE_STATUS_INVALID_ARGUMENT ||
      result != NULL) {
    return fail("page 0 must be rejected");
  }
  return bad;
}

static int stage_screenshots(LiteParseDocument *document) {
  LiteParseScreenshots *shots = NULL;
  if (liteparse_document_screenshot(document, NULL, 0, 0.0f, NULL, &shots) != LITEPARSE_STATUS_OK) {
    return fail("screenshot");
  }
  const LiteParseScreenshotsView *v = liteparse_screenshots_view(shots);
  const LiteParseDocumentInfo *info = liteparse_document_info(document);
  int bad = v == NULL || v->screenshots_len != info->total_pages;
  for (size_t i = 0; !bad && i < v->screenshots_len; i++) {
    const LiteParseScreenshot *s = &v->screenshots[i];
    if (s->png.len < 8 || memcmp(v->arenas.binary + s->png.offset, "\x89PNG", 4) != 0) bad = fail("png magic");
    bad |= check_range(s->rect_offset, s->rect_count, v->rects_len, "rects");
  }
  liteparse_screenshots_free(shots);

  LiteParseRenderRegion region = {10.0f, 20.0f, 100.0f, 50.0f};
  if (liteparse_document_screenshot(document, NULL, 0, 144.0f, &region, &shots) !=
      LITEPARSE_STATUS_OK) {
    return fail("region screenshot");
  }
  v = liteparse_screenshots_view(shots);
  if (v->screenshots[0].width != 200 || v->screenshots[0].height != 100) bad = fail("region size");
  liteparse_screenshots_free(shots);
  region.width = 1e9f;
  if (liteparse_document_screenshot(document, NULL, 0, 0.0f, &region, &shots) !=
      LITEPARSE_STATUS_INVALID_ARGUMENT) {
    bad = fail("out-of-page region must be rejected");
  }
  return bad;
}

static int stage_complexity(LiteParseDocument *document) {
  LiteParseComplexity *complexity = NULL;
  if (liteparse_document_complexity(document, NULL, 0, &complexity) != LITEPARSE_STATUS_OK) {
    return fail("complexity");
  }
  const LiteParseComplexityView *v = liteparse_complexity_view(complexity);
  int bad = v == NULL || v->pages_len == 0 || v->pages[0].page_number != 1 ||
            v->arenas.page_errors_len != 0;
  liteparse_complexity_free(complexity);
  return bad;
}

static int stage_raw_text(LiteParseDocument *document) {
  LiteParseRawText *raw = NULL;
  if (liteparse_document_raw_text(document, NULL, 0, &raw) != LITEPARSE_STATUS_OK) {
    return fail("raw text");
  }
  const LiteParseRawTextView *v = liteparse_raw_text_view(raw);
  int str_bad = 0;
  for (size_t i = 0; v != NULL && i < v->pages_len; i++) {
    str_bad |= check_str(v->arenas.pool, v->arenas.pool_len, v->pages[i].label, "raw page label");
  }
  for (size_t i = 0; v != NULL && i < v->items_len; i++) {
    str_bad |= check_str(v->arenas.pool, v->arenas.pool_len, v->items[i].text, "raw item text");
  }
  for (size_t i = 0; v != NULL && i < v->glyph_names_len; i++) {
    str_bad |= check_str(v->arenas.pool, v->arenas.pool_len, v->glyph_names[i], "raw glyph name");
  }
  int bad = v == NULL || v->pages_len == 0 || str_bad;
  for (size_t i = 0; !bad && i < v->pages_len; i++) {
    bad |= check_range(v->pages[i].item_offset, v->pages[i].item_count, v->items_len, "raw items");
  }
  for (size_t i = 0; !bad && i < v->items_len; i++) {
    bad |= check_range(v->items[i].char_code_offset, v->items[i].char_code_count,
                       v->char_codes_len, "raw char codes");
    bad |= check_range(v->items[i].glyph_name_offset, v->items[i].glyph_name_count,
                       v->glyph_names_len, "raw glyph names");
  }
  liteparse_raw_text_free(raw);
  return bad;
}

/* A result's content is valid parse input as is. */
static int project_extracted(LiteParseParser *parser, const LiteParseResultView *v,
                             LiteParseResult **out) {
  LiteParseContentInput input;
  liteparse_content_init(&input);
  input.content = v->content;
  return liteparse_parser_parse_content(parser, &input, out) != LITEPARSE_STATUS_OK;
}

static int stage_extract(LiteParseParser *parser, LiteParseDocument *document) {
  LiteParseResult *extracted = NULL;
  if (liteparse_document_extract(document, NULL, 0, &extracted) != LITEPARSE_STATUS_OK) {
    return fail("extract");
  }
  int bad = check_result(extracted, 0);
  LiteParseResult *projected = NULL;
  const LiteParseResultView *v = liteparse_result_view(extracted);
  if (project_extracted(parser, v, &projected)) {
    bad = fail("parse_content from extract");
  } else {
    bad |= check_result(projected, 1);
    liteparse_result_free(projected);
  }
  liteparse_result_free(extracted);
  return bad;
}

static int stage_page_objects(LiteParseDocument *document) {
  LiteParsePageObjects *objects = NULL;
  if (liteparse_document_page_objects(document, NULL, 0, 0, &objects) != LITEPARSE_STATUS_OK) {
    return fail("page objects");
  }
  const LiteParsePageObjectsView *v = liteparse_page_objects_view(objects);
  int bad = v == NULL || v->pages_len == 0;
  for (size_t i = 0; !bad && i < v->filters_len; i++) {
    bad |= check_str(v->arenas.pool, v->arenas.pool_len, v->filters[i], "filter name");
  }
  for (size_t i = 0; !bad && i < v->pages_len; i++) {
    bad |= check_range(v->pages[i].object_offset, v->pages[i].object_count, v->objects_len,
                       "page objects");
  }
  for (size_t i = 0; !bad && i < v->objects_len; i++) {
    bad |= check_range(v->objects[i].child_offset, v->objects[i].child_count, v->objects_len,
                       "children");
    bad |= check_range(v->objects[i].segment_offset, v->objects[i].segment_count,
                       v->segments_len, "segments");
    bad |= check_range(v->objects[i].filter_offset, v->objects[i].filter_count, v->filters_len,
                       "filters");
  }
  liteparse_page_objects_free(objects);
  return bad;
}

static int stage_result_screenshots(const char *path) {
  LiteParseParser *parser = new_parser(LITEPARSE_FLAG_EXTRACT_SCREENSHOTS, LITEPARSE_OUTPUT_FORMAT_TEXT);
  if (parser == NULL) return fail("parser");
  LiteParseDocument *document = NULL;
  LiteParseResult *result = NULL;
  int bad = 0;
  if (liteparse_document_open_path(parser, BYTES(path), &document) != LITEPARSE_STATUS_OK ||
      liteparse_document_parse(document, NULL, 0, &result) != LITEPARSE_STATUS_OK) {
    bad = fail("screenshots parse");
  } else {
    const LiteParseResultView *v = liteparse_result_view(result);
    if (v->screenshots_len != v->content.pages_len) bad = fail("result screenshots");
  }
  liteparse_result_free(result);
  liteparse_document_free(document);
  liteparse_parser_free(parser);
  return bad;
}

static int stage_markdown(const char *path) {
  LiteParseParser *parser = new_parser(0, LITEPARSE_OUTPUT_FORMAT_MARKDOWN);
  if (parser == NULL) return fail("parser");
  LiteParseDocument *document = NULL;
  LiteParseResult *result = NULL;
  int bad = 0;
  if (liteparse_document_open_path(parser, BYTES(path), &document) != LITEPARSE_STATUS_OK ||
      liteparse_document_parse(document, NULL, 0, &result) != LITEPARSE_STATUS_OK) {
    bad = fail("markdown parse");
  } else {
    const LiteParseResultView *v = liteparse_result_view(result);
    if (v->page_outputs_len == 0 || v->page_outputs[0].markdown.len == 0 ||
        v->page_outputs[0].text.len == 0) {
      bad = fail("markdown and text");
    }
  }
  liteparse_result_free(result);
  liteparse_document_free(document);
  liteparse_parser_free(parser);
  return bad;
}

static int stage_content(LiteParseParser *parser) {
  /* The caller's pool needs neither a leading NUL nor terminators. */
  static const char pool[] = "hello smokeA-1";
  LiteParseTextItem item;
  memset(&item, 0, sizeof item);
  item.text.offset = 0;
  item.text.len = 11;
  item.x = 72.0f;
  item.y = 100.0f;
  item.width = 80.0f;
  item.height = 12.0f;
  LiteParsePage page;
  memset(&page, 0, sizeof page);
  page.page_number = 1;
  page.width = 612.0f;
  page.height = 792.0f;
  page.item_count = 1;
  page.label.offset = 11;
  page.label.len = 3;
  LiteParseContentInput input;
  liteparse_content_init(&input);
  if (input.struct_size != sizeof(LiteParseContentInput)) return fail("content size");
  input.content.total_pages = 1;
  input.content.arenas.pool = (const uint8_t *)pool;
  input.content.arenas.pool_len = sizeof pool - 1;
  input.content.pages = &page;
  input.content.pages_len = 1;
  input.content.items = &item;
  input.content.items_len = 1;
  LiteParseResult *result = NULL;
  if (liteparse_parser_parse_content(parser, &input, &result) != LITEPARSE_STATUS_OK) {
    return fail("parse_content");
  }
  const LiteParseResultView *v = liteparse_result_view(result);
  const char *text = pooled(v->content.arenas.pool, v->content.arenas.pool_len, v->text);
  const char *label = pooled(v->content.arenas.pool, v->content.arenas.pool_len, v->content.pages[0].label);
  int bad = text == NULL || strstr(text, "hello smoke") == NULL || label == NULL ||
            strcmp(label, "A-1") != 0;
  if (bad) fail("content text");
  liteparse_result_free(result);
  return bad;
}

static int stage_bytes(LiteParseParser *parser, const char *path) {
  FILE *file = fopen(path, "rb");
  if (file == NULL) return fail("open fixture");
  fseek(file, 0, SEEK_END);
  long size = ftell(file);
  fseek(file, 0, SEEK_SET);
  uint8_t *bytes = malloc((size_t)size);
  if (bytes == NULL || fread(bytes, 1, (size_t)size, file) != (size_t)size) {
    fclose(file);
    free(bytes);
    return fail("read fixture");
  }
  fclose(file);
  LiteParseDocument *document = NULL;
  LiteParseStatus status = liteparse_document_open_bytes(parser, bytes, (size_t)size, &document);
  free(bytes);
  if (status != LITEPARSE_STATUS_OK) return fail("open bytes");
  int bad = stage_parse(document);
  liteparse_document_free(document);
  return bad;
}

static int stage_ocr(const char *image_path) {
  LiteParseParser *parser = new_parser(LITEPARSE_FLAG_OCR_FAILURE_FATAL, LITEPARSE_OUTPUT_FORMAT_TEXT);
  if (parser == NULL) return fail("parser");
  LiteParseDocument *document = NULL;
  LiteParseJob *job = NULL;
  LiteParseResult *result = NULL;
  int bad = 0, rounds = 0;
  if (liteparse_document_open_path(parser, BYTES(image_path), &document) != LITEPARSE_STATUS_OK ||
      liteparse_document_begin(document, NULL, 0, &job) != LITEPARSE_STATUS_OK) {
    bad = fail("begin");
  }
  /* The job outlives its document. */
  liteparse_document_free(document);
  static const char pool[] = "SMOKEOCRWORD";
  for (;;) {
    const LiteParseOcrRasterView *round = NULL;
    if (bad || liteparse_job_render_ocr(job, NULL, 0, 0, LITEPARSE_OCR_PIXEL_FORMAT_RGB, &round) !=
                   LITEPARSE_STATUS_OK) {
      bad = bad || fail("render ocr");
      break;
    }
    if (round->rasters_len == 0) break;
    rounds++;
    /* One merge per round: merging ends the round's view. */
    size_t n = round->rasters_len;
    LiteParseOcrPageInput *inputs = calloc(n, sizeof *inputs);
    LiteParseOcrWord *words = calloc(n, sizeof *words);
    if (inputs == NULL || words == NULL) bad = fail("alloc");
    for (size_t i = 0; i < n && !bad; i++) {
      const LiteParseOcrRaster *raster = &round->rasters[i];
      if (raster->pixels.len != (size_t)raster->width * raster->height * 3 ||
          (size_t)raster->pixels.offset + raster->pixels.len > round->arenas.binary_len) {
        bad = fail("raster pixels");
      }
      inputs[i].page_number = raster->page_number;
      inputs[i].word_offset = (uint32_t)i;
      inputs[i].word_count = 1;
      words[i].text.len = sizeof pool - 1;
      words[i].x2 = 40.0f;
      words[i].y2 = 10.0f;
      words[i].confidence = 0.9f;
    }
    if (!bad && liteparse_job_merge_ocr(job, inputs, n, words, n, (const uint8_t *)pool,
                                        sizeof pool - 1) != LITEPARSE_STATUS_OK) {
      bad = fail("merge ocr");
    }
    free(inputs);
    free(words);
    if (bad) break;
  }
  if (!bad && liteparse_job_finish(&job, &result) != LITEPARSE_STATUS_OK) bad = fail("finish");
  if (!bad) {
    if (job != NULL) bad = fail("finish consumes the job");
    const LiteParseResultView *v = liteparse_result_view(result);
    const char *text = pooled(v->content.arenas.pool, v->content.arenas.pool_len, v->text);
    if (rounds == 0 || text == NULL || strstr(text, "SMOKEOCRWORD") == NULL) bad = fail("ocr words");
  }
  liteparse_job_free(job);
  liteparse_result_free(result);
  liteparse_parser_free(parser);
  return bad;
}

static int run_document_stages(const char *path) {
  LiteParseParser *parser = new_parser(0, LITEPARSE_OUTPUT_FORMAT_TEXT);
  if (parser == NULL) return fail("parser");
  LiteParseDocument *document = NULL;
  if (liteparse_document_open_path(parser, BYTES(path), &document) != LITEPARSE_STATUS_OK) {
    liteparse_parser_free(parser);
    return fail("open");
  }
  int bad = stage_parse(document) | stage_page_selection(document) |
            stage_screenshots(document) | stage_complexity(document) |
            stage_raw_text(document) | stage_extract(parser, document) |
            stage_page_objects(document) | stage_content(parser) | stage_bytes(parser, path);
  liteparse_document_free(document);
  liteparse_parser_free(parser);
  return bad;
}

int main(int argc, char **argv) {
  if (argc < 2 || argc > 3) {
    fprintf(stderr, "usage: %s <document.pdf> [image.png]\n", argv[0]);
    return 2;
  }
  LiteParseByteView version;
  liteparse_version(&version);
  if (version.len == 0) return fail("version");
  LiteParseAbiDescriptor descriptor;
  liteparse_abi_descriptor(&descriptor);
  if (descriptor.pointer_width != sizeof(void *) * 8 ||
      descriptor.layout_fingerprint != LITEPARSE_ABI_FINGERPRINT ||
      descriptor.max_selected_pages != LITEPARSE_MAX_SELECTED_PAGES ||
      liteparse_sizeof(LITEPARSE_TYPE_ABI_DESCRIPTOR) != sizeof descriptor ||
      liteparse_sizeof(LITEPARSE_TYPE_CONTENT_INPUT) != sizeof(LiteParseContentInput) ||
      liteparse_sizeof(LITEPARSE_TYPE_CONTENT_ARRAYS) != sizeof(LiteParseContentArrays) ||
      liteparse_sizeof(LITEPARSE_TYPE_PAGE_OUTPUT) != sizeof(LiteParsePageOutput) ||
      liteparse_sizeof(LITEPARSE_TYPE_ARENAS) != sizeof(LiteParseArenas) ||
      liteparse_sizeof(LITEPARSE_TYPE_OCR_RASTER_VIEW) != sizeof(LiteParseOcrRasterView)) {
    return fail("ABI descriptor");
  }
  if (liteparse_sizeof(LITEPARSE_TYPE_CONFIG) != sizeof(LiteParseConfig) ||
      liteparse_sizeof(LITEPARSE_TYPE_TEXT_ITEM) != sizeof(LiteParseTextItem) ||
      liteparse_sizeof(LITEPARSE_TYPE_RESULT_VIEW) != sizeof(LiteParseResultView) ||
      liteparse_sizeof(LITEPARSE_TYPE_STR) != sizeof(LiteParseStr) || liteparse_sizeof(9999) != 0) {
    return fail("sizeof table");
  }
  int bad = run_document_stages(argv[1]) | stage_result_screenshots(argv[1]) |
            stage_markdown(argv[1]);
  if (argc == 3) bad |= stage_ocr(argv[2]);
  return bad ? 1 : 0;
}
