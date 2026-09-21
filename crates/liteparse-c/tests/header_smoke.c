/* usage: header_smoke <document> [ocr-image] */

#include "liteparse.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static LiteParseByteView cstr(const char *s) {
  LiteParseByteView view = {(const uint8_t *)s, strlen(s)};
  return view;
}

static bool view_contains(LiteParseByteView haystack, const char *needle) {
  size_t n = strlen(needle);
  if (haystack.ptr == NULL || haystack.len < n) return false;
  for (size_t i = 0; i + n <= haystack.len; i++) {
    if (memcmp(haystack.ptr + i, needle, n) == 0) return true;
  }
  return false;
}

static bool fail(const char *what) {
  LiteParseByteView error = liteparse_last_error();
  fprintf(stderr, "%s: %.*s\n", what, (int)error.len,
          error.ptr ? (const char *)error.ptr : "");
  return false;
}

static bool read_file(const char *path, uint8_t **out_data, size_t *out_len) {
  FILE *file = fopen(path, "rb");
  if (file == NULL) return false;
  bool ok = fseek(file, 0, SEEK_END) == 0;
  long file_len = ok ? ftell(file) : -1;
  ok = ok && file_len >= 0 && fseek(file, 0, SEEK_SET) == 0;
  uint8_t *data = ok ? (uint8_t *)malloc((size_t)file_len + 1) : NULL;
  ok = data != NULL && fread(data, 1, (size_t)file_len, file) == (size_t)file_len;
  fclose(file);
  if (!ok) {
    free(data);
    return false;
  }
  *out_data = data;
  *out_len = (size_t)file_len;
  return true;
}

static LiteParseParser *new_parser(uint64_t extra_flags, uint32_t output_format) {
  LiteParseConfig config = liteparse_config_default();
  config.bools_set = LITEPARSE_FLAG_QUIET | LITEPARSE_FLAG_OCR_ENABLED |
                     LITEPARSE_FLAG_EMIT_WORD_BOXES | extra_flags;
  config.bools_values = LITEPARSE_FLAG_QUIET | LITEPARSE_FLAG_EMIT_WORD_BOXES |
                        extra_flags;
  config.output_format = output_format;
  LiteParseParserNew created = liteparse_parser_new(&config);
  if (created.status != LITEPARSE_STATUS_OK) fail("parser_new");
  return created.handle;
}

static bool check_typed_result(const LiteParseResult *result) {
  LiteParseByteView json = {NULL, 0};
  if (liteparse_result_to_json(result, &json) != LITEPARSE_STATUS_OK ||
      !view_contains(json, "\"total_pages\"") || !view_contains(json, "\"text\"")) {
    return fail("result_to_json");
  }
  if (liteparse_result_page_count(result) == 0 ||
      liteparse_result_total_pages(result) == 0 ||
      liteparse_result_page_number(result, 0) == 0 ||
      (liteparse_result_page_label(result, 0).ptr == NULL &&
       liteparse_result_page_label(result, 0).len != 0) ||
      liteparse_result_text(result).len == 0 ||
      liteparse_result_page_text(result, 0).ptr == NULL ||
      liteparse_result_page_size(result, 0).width <= 0.0f) {
    return fail("result accessors");
  }
  (void)liteparse_result_creator(result);
  (void)liteparse_result_producer(result);

  size_t item_count = 0;
  const LiteParseTextItem *items =
      liteparse_result_text_items(result, 0, &item_count);
  if (items == NULL || item_count == 0 || items[0].text.len == 0) {
    return fail("text_items");
  }

  bool found_word = false;
  for (size_t i = 0; i < item_count && !found_word; i++) {
    size_t word_count = 0;
    const LiteParseWordBox *words =
        liteparse_result_word_boxes(result, 0, i, &word_count);
    found_word = words != NULL && word_count > 0 && words[0].text.len > 0;
  }
  if (!found_word) return fail("word_boxes");

  size_t image_count = 0;
  (void)liteparse_result_images(result, &image_count);

  LiteParseProjectedLayoutAbi layout_abi = liteparse_projected_layout_abi();
  if (layout_abi.version != LITEPARSE_PROJECTED_LAYOUT_ABI_VERSION ||
      layout_abi.layout_bytes_size != sizeof(LiteParseLayoutBytes) ||
      layout_abi.layout_bytes_align != _Alignof(LiteParseLayoutBytes) ||
      layout_abi.projected_page_size != sizeof(LiteParseProjectedLayoutPage) ||
      layout_abi.projected_page_align != _Alignof(LiteParseProjectedLayoutPage) ||
      layout_abi.projected_line_size != sizeof(LiteParseProjectedLine) ||
      layout_abi.projected_line_align != _Alignof(LiteParseProjectedLine) ||
      layout_abi.projected_span_size != sizeof(LiteParseProjectedSpan) ||
      layout_abi.projected_span_align != _Alignof(LiteParseProjectedSpan) ||
      layout_abi.projected_word_size != sizeof(LiteParseProjectedWord) ||
      layout_abi.projected_word_align != _Alignof(LiteParseProjectedWord) ||
      layout_abi.projected_region_size != sizeof(LiteParseProjectedRegion) ||
      layout_abi.projected_region_align != _Alignof(LiteParseProjectedRegion) ||
      layout_abi.line_span_offset != offsetof(LiteParseProjectedLine, span_offset) ||
      layout_abi.line_region_path_offset !=
          offsetof(LiteParseProjectedLine, region_path_offset) ||
      layout_abi.span_char_code_offset !=
          offsetof(LiteParseProjectedSpan, char_code_offset) ||
      layout_abi.span_word_offset != offsetof(LiteParseProjectedSpan, word_offset) ||
      layout_abi.page_region_offset !=
          offsetof(LiteParseProjectedLayoutPage, region_offset) ||
      layout_abi.region_child_offset !=
          offsetof(LiteParseProjectedRegion, child_offset) ||
      layout_abi.layout_bytes_len_offset != offsetof(LiteParseLayoutBytes, len)) {
    return fail("projected_layout_abi");
  }

  LiteParseProjectedLayoutSnapshot snapshot =
      liteparse_result_projected_layout_snapshot(result);
  size_t page_count = 0;
  const LiteParseProjectedLayoutPage *pages =
      liteparse_result_projected_layout_pages(result, &page_count);
  size_t line_count = 0;
  const LiteParseProjectedLine *lines =
      liteparse_result_projected_layout_lines(result, &line_count);
  size_t span_count = 0;
  const LiteParseProjectedSpan *spans =
      liteparse_result_projected_layout_spans(result, &span_count);
  size_t word_count = 0;
  const LiteParseProjectedWord *projected_words =
      liteparse_result_projected_layout_words(result, &word_count);
  size_t char_code_count = 0;
  const uint32_t *char_codes =
      liteparse_result_projected_layout_char_codes(result, &char_code_count);
  size_t path_count = 0;
  const uint16_t *paths =
      liteparse_result_projected_layout_region_paths(result, &path_count);
  size_t region_count = 0;
  const LiteParseProjectedRegion *regions =
      liteparse_result_projected_layout_regions(result, &region_count);
  size_t region_item_count = 0;
  const uint64_t *region_items =
      liteparse_result_projected_layout_region_items(result, &region_item_count);
  size_t region_child_count = 0;
  const uint64_t *region_children =
      liteparse_result_projected_layout_region_children(result, &region_child_count);
  if (snapshot.version != LITEPARSE_PROJECTED_LAYOUT_SNAPSHOT_VERSION ||
      page_count != snapshot.page_count || line_count != snapshot.line_count ||
      span_count != snapshot.span_count || word_count != snapshot.word_count ||
      char_code_count != snapshot.char_code_count || path_count != snapshot.region_path_count ||
      region_count != snapshot.region_count || region_item_count != snapshot.region_item_count ||
      region_child_count != snapshot.region_child_count ||
      (page_count != 0 && pages == NULL) || (line_count != 0 && lines == NULL) ||
      (span_count != 0 && spans == NULL) || (region_count != 0 && regions == NULL) ||
      (word_count != 0 && projected_words == NULL) ||
      (char_code_count != 0 && char_codes == NULL) ||
      (path_count != 0 && paths == NULL) ||
      (region_item_count != 0 && region_items == NULL) ||
      (region_child_count != 0 && region_children == NULL) ||
      (snapshot.flags & LITEPARSE_PROJECTED_LAYOUT_FLAG_SOURCE_PROVENANCE_UNAVAILABLE) == 0 ||
      (snapshot.flags & LITEPARSE_PROJECTED_LAYOUT_FLAG_BLOCK_ASSOCIATIONS_UNAVAILABLE) == 0) {
    return fail("projected_layout_snapshot");
  }
  for (size_t i = 0; i < page_count; i++) {
    const LiteParseProjectedLayoutPage *page = &pages[i];
    if (page->line_offset + page->line_count > line_count ||
        page->span_offset + page->span_count > span_count ||
        page->word_offset + page->word_count > word_count ||
        page->char_code_offset + page->char_code_count > char_code_count ||
        page->region_path_offset + page->region_path_count > path_count ||
        page->region_offset + page->region_count > region_count ||
        page->region_item_offset + page->region_item_count > region_item_count ||
        page->region_child_offset + page->region_child_count > region_child_count) {
      return fail("projected_layout_page_ranges");
    }
  }
  for (size_t i = 0; i < line_count; i++) {
    if (lines[i].span_offset + lines[i].span_count > span_count ||
        lines[i].region_path_offset + lines[i].region_path_count > path_count) {
      return fail("projected_layout_line_ranges");
    }
  }
  for (size_t i = 0; i < span_count; i++) {
    if (spans[i].word_offset + spans[i].word_count > word_count ||
        spans[i].char_code_offset + spans[i].char_code_count > char_code_count) {
      return fail("projected_layout_span_ranges");
    }
  }
  for (size_t i = 0; i < region_count; i++) {
    if (regions[i].child_offset + regions[i].child_count > region_child_count ||
        regions[i].item_offset + regions[i].item_count > region_item_count) {
      return fail("projected_layout_region_ranges");
    }
  }
  return true;
}

static bool check_search(const LiteParseResult *result) {
  size_t item_count = 0;
  const LiteParseTextItem *items =
      liteparse_result_text_items(result, 0, &item_count);
  if (items == NULL || item_count == 0) return fail("text_items for search");

  char phrase[8] = {0};
  size_t phrase_len = items[0].text.len < 4 ? items[0].text.len : 4;
  memcpy(phrase, items[0].text.ptr, phrase_len);
  LiteParseByteView phrase_view = {(const uint8_t *)phrase, phrase_len};

  LiteParseSearchMatchesNew search =
      liteparse_result_search(result, 0, phrase_view, false);
  if (search.status != LITEPARSE_STATUS_OK) return fail("result_search");
  size_t match_count = 0;
  const LiteParseTextItem *matches =
      liteparse_search_matches_slice(search.handle, &match_count);
  bool ok = matches != NULL && match_count > 0 && matches[0].text.ptr != NULL;
  liteparse_search_matches_free(search.handle);
  return ok || fail("search_matches_slice");
}

static bool stage_parse(const LiteParseDocument *document) {
  LiteParseResultNew parsed = liteparse_document_parse(document, NULL, 0);
  if (parsed.status != LITEPARSE_STATUS_OK) return fail("document_parse");
  bool ok = check_typed_result(parsed.handle) && check_search(parsed.handle);
  liteparse_result_free(parsed.handle);
  return ok;
}

static bool stage_page_selection(const LiteParseDocument *document) {
  uint32_t total = liteparse_document_total_pages(document);
  if (total == 0) return fail("document_total_pages");
  uint32_t last = total;
  LiteParseResultNew parsed = liteparse_document_parse(document, &last, 1);
  if (parsed.status != LITEPARSE_STATUS_OK) return fail("document_parse(last)");
  bool ok = liteparse_result_page_count(parsed.handle) == 1 &&
            liteparse_result_page_number(parsed.handle, 0) == total;
  liteparse_result_free(parsed.handle);
  return ok || fail("page selection");
}

static bool stage_outline(const LiteParseDocument *document) {
  size_t count = 7;
  const LiteParseOutlineEntry *entries = liteparse_document_outline(document, &count);
  return (entries == NULL) == (count == 0) || fail("document_outline");
}

static bool stage_screenshots(const LiteParseDocument *document) {
  LiteParseScreenshotsNew rendered =
      liteparse_document_screenshot(document, NULL, 0, 0.0f, NULL);
  if (rendered.status != LITEPARSE_STATUS_OK) return fail("document_screenshot");
  size_t count = 0;
  const LiteParseScreenshot *shots =
      liteparse_screenshots_slice(rendered.handle, &count);
  size_t rect_count = 0;
  (void)liteparse_screenshots_rects(rendered.handle, 0, &rect_count);
  bool ok = shots != NULL && count == liteparse_document_total_pages(document) &&
            shots[0].png.ptr != NULL && shots[0].png.len > 4 &&
            memcmp(shots[0].png.ptr, "\x89PNG", 4) == 0;
  liteparse_screenshots_free(rendered.handle);
  return ok || fail("screenshots_slice");
}

static bool stage_region(const LiteParseDocument *document) {
  const float dpi = 144.0f;
  LiteParseRenderRegion region = {10.3f, 20.7f, 100.2f, 50.4f};
  uint32_t first = 1;
  LiteParseScreenshotsNew rendered =
      liteparse_document_screenshot(document, &first, 1, dpi, &region);
  if (rendered.status != LITEPARSE_STATUS_OK) return fail("region screenshot");
  size_t count = 0;
  const LiteParseScreenshot *shots =
      liteparse_screenshots_slice(rendered.handle, &count);
  uint32_t expected_w = (uint32_t)lroundf(region.width * dpi / 72.0f);
  uint32_t expected_h = (uint32_t)lroundf(region.height * dpi / 72.0f);
  bool ok = shots != NULL && count == 1 && shots[0].width == expected_w &&
            shots[0].height == expected_h;
  liteparse_screenshots_free(rendered.handle);
  if (!ok) return fail("region dimensions");

  LiteParseRenderRegion outside = {0.0f, 0.0f, 99999.0f, 1.0f};
  LiteParseScreenshotsNew rejected =
      liteparse_document_screenshot(document, NULL, 0, 0.0f, &outside);
  liteparse_screenshots_free(rejected.handle);
  return rejected.status == LITEPARSE_STATUS_INVALID_ARGUMENT ||
         fail("region validation");
}

static bool stage_raw_text(const LiteParseDocument *document) {
  LiteParseRawTextNew created = liteparse_document_raw_text(document, NULL, 0);
  if (created.status != LITEPARSE_STATUS_OK) return fail("document_raw_text");
  size_t page_count = 0;
  const LiteParseRawTextPage *pages =
      liteparse_raw_text_pages(created.handle, &page_count);
  size_t item_count = 0;
  const LiteParseRawTextItem *items =
      liteparse_raw_text_items(created.handle, 0, &item_count);
  bool ok = pages != NULL && page_count == liteparse_document_total_pages(document) &&
            pages[0].page_number == 1 && items != NULL && item_count > 0 &&
            (items[0].text.len > 0 || items[0].char_codes_len > 0);
  liteparse_raw_text_free(created.handle);
  return ok || fail("raw_text_pages");
}

static bool stage_extract(const LiteParseDocument *document) {
  LiteParseExtractNew created = liteparse_document_extract(document, NULL, 0);
  if (created.status != LITEPARSE_STATUS_OK) return fail("document_extract");
  size_t page_count = 0;
  const LiteParseContentPage *pages =
      liteparse_extract_pages(created.handle, &page_count);
  size_t item_count = 0;
  const LiteParseTextItem *items =
      liteparse_extract_items(created.handle, &item_count);
  LiteParseContent content = liteparse_extract_as_content(created.handle);
  bool ok = pages != NULL && page_count == liteparse_document_total_pages(document) &&
            pages[0].page_number == 1 && items != NULL && item_count > 0 &&
            content.pages_len == page_count && content.items_len == item_count &&
            content.size_of_content == sizeof(LiteParseContent);
  liteparse_extract_free(created.handle);
  return ok || fail("extract_pages");
}

static bool stage_page_objects(const LiteParseDocument *document) {
  LiteParsePageObjectsNew created =
      liteparse_document_page_objects(document, NULL, 0, 0);
  if (created.status != LITEPARSE_STATUS_OK) return fail("document_page_objects");
  size_t page_count = 0;
  const LiteParsePageObjectPage *pages =
      liteparse_page_objects_pages(created.handle, &page_count);
  size_t object_count = 0;
  const LiteParsePageObject *objects =
      liteparse_page_objects_objects(created.handle, &object_count);
  size_t segment_count = 7;
  (void)liteparse_page_objects_segments(created.handle, &segment_count);
  bool ok = pages != NULL && page_count == liteparse_document_total_pages(document) &&
            pages[0].page_number == 1 &&
            liteparse_page_objects_page_count(created.handle) == page_count &&
            (objects != NULL) == (object_count > 0);
  liteparse_page_objects_free(created.handle);
  return ok || fail("page_objects_pages");
}

static bool stage_complexity(const LiteParseDocument *document) {
  LiteParseComplexityNew created =
      liteparse_document_complexity(document, NULL, 0);
  if (created.status != LITEPARSE_STATUS_OK) return fail("document_complexity");
  size_t count = 0;
  const LiteParsePageComplexity *pages =
      liteparse_complexity_slice(created.handle, &count);
  bool ok = pages != NULL && count > 0 && pages[0].page_number != 0 &&
            pages[0].needs_ocr == (pages[0].reasons_mask != 0) &&
            view_contains(liteparse_complexity_json(created.handle), "needs_ocr");
  liteparse_complexity_free(created.handle);
  return ok || fail("complexity");
}

static bool stage_result_screenshots(const char *path) {
  LiteParseParser *parser =
      new_parser(LITEPARSE_FLAG_EXTRACT_SCREENSHOTS, LITEPARSE_UNSET);
  if (parser == NULL) return false;
  LiteParseDocumentNew opened = liteparse_document_open_path(parser, cstr(path));
  bool ok = opened.status == LITEPARSE_STATUS_OK || fail("document_open_path");
  if (ok) {
    LiteParseResultNew parsed = liteparse_document_parse(opened.handle, NULL, 0);
    ok = parsed.status == LITEPARSE_STATUS_OK || fail("document_parse");
    if (ok) {
      size_t shot_count = 0;
      const LiteParseScreenshot *shots =
          liteparse_result_screenshots(parsed.handle, &shot_count);
      ok = shots != NULL &&
           shot_count == liteparse_result_page_count(parsed.handle) &&
           shots[0].png.len > 0;
      if (!ok) fail("result_screenshots");
    }
    liteparse_result_free(parsed.handle);
  }
  liteparse_document_free(opened.handle);
  liteparse_parser_free(parser);
  return ok;
}

static bool stage_markdown(const char *path) {
  LiteParseParser *parser = new_parser(0, LITEPARSE_OUTPUT_FORMAT_MARKDOWN);
  if (parser == NULL) return false;
  LiteParseDocumentNew opened = liteparse_document_open_path(parser, cstr(path));
  bool ok = opened.status == LITEPARSE_STATUS_OK || fail("document_open_path");
  if (ok) {
    LiteParseResultNew parsed = liteparse_document_parse(opened.handle, NULL, 0);
    ok = parsed.status == LITEPARSE_STATUS_OK || fail("document_parse");
    if (ok) {
      ok = liteparse_result_page_markdown(parsed.handle, 0).len > 0 &&
           liteparse_result_page_text(parsed.handle, 0).len > 0;
      if (!ok) fail("page_markdown");
    }
    liteparse_result_free(parsed.handle);
  }
  liteparse_document_free(opened.handle);
  liteparse_parser_free(parser);
  return ok;
}

static bool stage_content(void) {
  LiteParseParser *parser = new_parser(0, LITEPARSE_UNSET);
  if (parser == NULL) return false;

  LiteParseTextItem item;
  memset(&item, 0, sizeof(item));
  item.text = cstr("hello");
  item.x = 72.0f;
  item.y = 100.0f;
  item.width = 80.0f;
  item.height = 12.0f;

  LiteParseContentPage page;
  memset(&page, 0, sizeof(page));
  page.page_number = 1;
  page.page_width = 612.0f;
  page.page_height = 792.0f;
  page.item_count = 1;

  LiteParseContent content = liteparse_content_default();
  content.pages = &page;
  content.pages_len = 1;
  content.items = &item;
  content.items_len = 1;

  LiteParseResultNew parsed = liteparse_parser_parse_content(parser, &content);
  bool ok = parsed.status == LITEPARSE_STATUS_OK || fail("parse_content");
  if (ok) {
    ok = liteparse_result_page_count(parsed.handle) == 1 &&
         view_contains(liteparse_result_text(parsed.handle), "hello");
    if (!ok) fail("parse_content text");
  }
  liteparse_result_free(parsed.handle);
  liteparse_parser_free(parser);
  return ok;
}

static bool stage_bytes(const char *path) {
  uint8_t *data = NULL;
  size_t data_len = 0;
  if (!read_file(path, &data, &data_len)) return fail("read_file");
  LiteParseParser *parser = new_parser(0, LITEPARSE_UNSET);
  bool ok = parser != NULL;
  if (ok) {
    LiteParseDocumentNew opened =
        liteparse_document_open_bytes(parser, data, data_len);
    ok = opened.status == LITEPARSE_STATUS_OK || fail("document_open_bytes");
    if (ok) {
      LiteParseResultNew parsed = liteparse_document_parse(opened.handle, NULL, 0);
      ok = parsed.status == LITEPARSE_STATUS_OK &&
           liteparse_result_page_count(parsed.handle) > 0;
      if (!ok) fail("parse from bytes");
      liteparse_result_free(parsed.handle);
    }
    liteparse_document_free(opened.handle);
  }
  liteparse_parser_free(parser);
  free(data);
  return ok;
}

static uint32_t smoke_ocr_recognize(void *user_data, const uint8_t *pixels,
                                    size_t pixels_len, uint32_t width,
                                    uint32_t height, uint32_t pixel_format,
                                    const char *language, float dpi,
                                    LiteParseOcrSink *sink) {
  (void)language;
  (void)dpi;
  if (pixels == NULL || pixel_format != LITEPARSE_OCR_PIXEL_FORMAT_RGB ||
      pixels_len != (size_t)width * height * 3) {
    liteparse_ocr_sink_set_error(sink, cstr("unexpected raster"));
    return 1;
  }
  *(int *)user_data += 1;
  return liteparse_ocr_sink_add(sink, cstr("SMOKEOCRWORD"), 10.0f, 10.0f,
                                120.0f, 40.0f, 0.9f, NULL) == LITEPARSE_STATUS_OK
             ? 0
             : 1;
}

static bool stage_ocr(const char *image_path) {
  int calls = 0;
  LiteParseConfig config = liteparse_config_default();
  config.bools_set = LITEPARSE_FLAG_QUIET | LITEPARSE_FLAG_OCR_ENABLED |
                     LITEPARSE_FLAG_OCR_FAILURE_FATAL;
  config.bools_values = config.bools_set;
  LiteParseParserNew created = liteparse_parser_new(&config);
  if (created.status != LITEPARSE_STATUS_OK) return fail("ocr parser_new");
  LiteParseParser *parser = created.handle;

  bool ok = liteparse_parser_set_ocr_callback(parser, smoke_ocr_recognize, &calls,
                                              cstr("smoke"), false) ==
            LITEPARSE_STATUS_OK;
  if (!ok) fail("set_ocr_callback");
  if (ok) {
    LiteParseDocumentNew opened =
        liteparse_document_open_path(parser, cstr(image_path));
    ok = opened.status == LITEPARSE_STATUS_OK || fail("open image");
    if (ok) {
      LiteParseResultNew parsed = liteparse_document_parse(opened.handle, NULL, 0);
      ok = parsed.status == LITEPARSE_STATUS_OK || fail("ocr parse");
      if (ok) {
        ok = calls > 0 &&
             view_contains(liteparse_result_text(parsed.handle), "SMOKEOCRWORD");
        if (!ok) fprintf(stderr, "OCR callback text missing from result\n");
      }
      liteparse_result_free(parsed.handle);
    }
    liteparse_document_free(opened.handle);
  }
  liteparse_parser_free(parser);
  return ok;
}

static bool run_document_stages(const char *path) {
  LiteParseParser *parser =
      new_parser(LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS, LITEPARSE_UNSET);
  if (parser == NULL) return false;
  LiteParseDocumentNew opened = liteparse_document_open_path(parser, cstr(path));
  bool ok = opened.status == LITEPARSE_STATUS_OK || fail("document_open_path");
  ok = ok && stage_parse(opened.handle) && stage_page_selection(opened.handle) &&
       stage_outline(opened.handle) && stage_screenshots(opened.handle) &&
       stage_region(opened.handle) && stage_complexity(opened.handle) &&
       stage_raw_text(opened.handle) && stage_extract(opened.handle) &&
       stage_page_objects(opened.handle);
  liteparse_document_free(opened.handle);
  liteparse_parser_free(parser);
  return ok;
}

int main(int argc, char **argv) {
  if (argc != 2 && argc != 3) {
    fprintf(stderr, "usage: %s <document> [ocr-image]\n", argv[0]);
    return 2;
  }
  const char *path = argv[1];
  bool ok = liteparse_version() != NULL && run_document_stages(path) &&
            stage_result_screenshots(path) && stage_markdown(path) &&
            stage_bytes(path) && stage_content() &&
            (argc < 3 || stage_ocr(argv[2]));
  return ok ? 0 : 1;
}
