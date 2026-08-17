// For memmem in the OCR smoke leg.
#define _GNU_SOURCE

#include "liteparse.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int has_expected_json(const char *json) {
  return json != NULL && strstr(json, "\"total_pages\"") != NULL &&
         strstr(json, "\"text\"") != NULL;
}

static int read_document(const char *path, uint8_t **out_data,
                         size_t *out_len) {
  FILE *file = fopen(path, "rb");
  if (file == NULL || fseek(file, 0, SEEK_END) != 0) {
    if (file != NULL) fclose(file);
    return 0;
  }
  long file_len = ftell(file);
  if (file_len < 0 || fseek(file, 0, SEEK_SET) != 0) {
    fclose(file);
    return 0;
  }
  size_t len = (size_t)file_len;
  uint8_t *data = (uint8_t *)malloc(len == 0 ? 1 : len);
  if (data == NULL || fread(data, 1, len, file) != len) {
    free(data);
    fclose(file);
    return 0;
  }
  fclose(file);
  *out_data = data;
  *out_len = len;
  return 1;
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
    liteparse_ocr_sink_set_error(sink, "unexpected raster");
    return 1;
  }
  *(int *)user_data += 1;
  return liteparse_ocr_sink_add(sink, "SMOKEOCRWORD", 10.0f, 10.0f, 120.0f,
                                40.0f, 0.9f, NULL) == LITEPARSE_STATUS_OK
             ? 0
             : 1;
}

static int run_ocr_smoke(const char *image_path) {
  LiteParseOptions *options = NULL;
  LiteParseParser *parser = NULL;
  char *error = NULL;
  int calls = 0;
  LiteParseStatus status = liteparse_options_new(&options, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_quiet(options, true, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_ocr_enabled(options, true, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_ocr_failure_fatal(options, true, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_options_set_ocr_callback(options, smoke_ocr_recognize,
                                                &calls, "smoke", false, &error);
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_parser_new_with_options(options, &parser, &error);
  liteparse_options_free(options);

  LiteParseResult *result = NULL;
  if (status == LITEPARSE_STATUS_OK)
    status = liteparse_parse_path_result(parser, image_path, &result, &error);
  LiteParseByteView text = {NULL, 0};
  if (status == LITEPARSE_STATUS_OK &&
      (calls == 0 || !liteparse_result_get_text(result, &text) ||
       text.ptr == NULL ||
       memmem(text.ptr, text.len, "SMOKEOCRWORD", 12) == NULL)) {
    fprintf(stderr, "OCR callback text missing from result\n");
    status = LITEPARSE_STATUS_PARSE_ERROR;
  }
  if (status != LITEPARSE_STATUS_OK)
    fprintf(stderr, "%s\n", error ? error : "OCR smoke failed");
  liteparse_result_free(result);
  liteparse_string_free(error);
  liteparse_parser_free(parser);
  return status == LITEPARSE_STATUS_OK ? 0 : 1;
}

int main(int argc, char **argv) {
  if (argc != 2 && argc != 3) {
    fprintf(stderr, "usage: %s <document> [ocr-image]\n", argv[0]);
    return 2;
  }
  if (argc == 3 && run_ocr_smoke(argv[2]) != 0) return 1;
  if (liteparse_version() == NULL) return 1;

  LiteParseOptions *options = NULL;
  LiteParseParser *parser = NULL;
  char *json = NULL;
  char *error = NULL;
  LiteParseStatus status = liteparse_options_new(&options, &error);
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_options_set_ocr_enabled(options, false, &error);
  }
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_options_set_quiet(options, true, &error);
  }
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_options_set_emit_word_boxes(options, true, &error);
  }
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_parser_new_with_options(options, &parser, &error);
  }
  liteparse_options_free(options);
  if (status != LITEPARSE_STATUS_OK) {
    liteparse_string_free(error);
    return (int)status;
  }

  status = liteparse_parse_path(parser, argv[1], &json, &error);
  if (status != LITEPARSE_STATUS_OK || !has_expected_json(json)) {
    fprintf(stderr, "%s\n", error ? error : "path parse failed");
    status = LITEPARSE_STATUS_PARSE_ERROR;
  }
  liteparse_string_free(json);
  liteparse_string_free(error);
  json = NULL;
  error = NULL;

  LiteParseResult *typed = NULL;
  status = liteparse_parse_path_result(parser, argv[1], &typed, &error);
  LiteParseByteView text = {NULL, 0};
  if (status != LITEPARSE_STATUS_OK || typed == NULL ||
      !liteparse_result_get_text(typed, &text) || text.ptr == NULL ||
      text.len == 0 || liteparse_result_get_page_count(typed) == 0) {
    fprintf(stderr, "%s\n", error ? error : "typed parse failed");
    status = LITEPARSE_STATUS_PARSE_ERROR;
  }
  liteparse_string_free(error);
  error = NULL;

  if (status == LITEPARSE_STATUS_OK) {
    // Search for the first text item's leading characters so the phrase
    // tracks the fixture document.
    LiteParseTextItem item;
    char phrase[8] = {0};
    if (!liteparse_result_get_text_item(typed, 0, 0, &item) ||
        item.text.len == 0) {
      fprintf(stderr, "no text item to search for\n");
      status = LITEPARSE_STATUS_PARSE_ERROR;
    } else {
      size_t phrase_len = item.text.len < 4 ? item.text.len : 4;
      memcpy(phrase, item.text.ptr, phrase_len);
      LiteParseSearchMatches *matches = NULL;
      status = liteparse_result_search(typed, 0, phrase, false, &matches,
                                       &error);
      LiteParseTextItem match;
      if (status != LITEPARSE_STATUS_OK ||
          liteparse_search_matches_get_count(matches) == 0 ||
          !liteparse_search_matches_get(matches, 0, &match) ||
          match.text.ptr == NULL) {
        fprintf(stderr, "%s\n", error ? error : "search failed");
        status = LITEPARSE_STATUS_PARSE_ERROR;
      }
      liteparse_search_matches_free(matches);
      liteparse_string_free(error);
      error = NULL;
    }
  }

  if (status == LITEPARSE_STATUS_OK) {
    // The parser opted into word boxes; some item on page 0 should split.
    size_t items = liteparse_result_get_text_item_count(typed, 0);
    int found = 0;
    for (size_t i = 0; i < items && !found; i++) {
      LiteParseWordBox word;
      if (liteparse_result_get_word_box_count(typed, 0, i) > 0 &&
          liteparse_result_get_word_box(typed, 0, i, 0, &word) &&
          word.text.ptr != NULL && word.text.len > 0) {
        found = 1;
      }
    }
    if (!found) {
      fprintf(stderr, "no word boxes emitted\n");
      status = LITEPARSE_STATUS_PARSE_ERROR;
    }
  }
  liteparse_result_free(typed);

  uint8_t *data = NULL;
  size_t data_len = 0;
  if (status == LITEPARSE_STATUS_OK &&
      !read_document(argv[1], &data, &data_len)) {
    status = LITEPARSE_STATUS_INVALID_ARGUMENT;
  }
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_parse_bytes(parser, data, data_len, &json, &error);
    if (status != LITEPARSE_STATUS_OK || !has_expected_json(json)) {
      fprintf(stderr, "%s\n", error ? error : "byte parse failed");
      status = LITEPARSE_STATUS_PARSE_ERROR;
    }
  }
  free(data);
  liteparse_string_free(json);
  liteparse_string_free(error);
  json = NULL;
  error = NULL;

  LiteParseScreenshots *screenshots = NULL;
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_screenshot_path(parser, argv[1], NULL, 0, &screenshots,
                                       &error);
    LiteParseScreenshot screenshot;
    if (status != LITEPARSE_STATUS_OK ||
        liteparse_screenshots_get_count(screenshots) == 0 ||
        !liteparse_screenshots_get(screenshots, 0, &screenshot) ||
        screenshot.png.ptr == NULL || screenshot.png.len == 0) {
      fprintf(stderr, "%s\n", error ? error : "screenshot failed");
      status = LITEPARSE_STATUS_PARSE_ERROR;
    }
  }
  liteparse_screenshots_free(screenshots);
  liteparse_string_free(error);
  error = NULL;

  LiteParseComplexity *complexity = NULL;
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_is_complex_path(parser, argv[1], &complexity, &error);
    LiteParsePageComplexity page;
    if (status != LITEPARSE_STATUS_OK ||
        liteparse_complexity_get_count(complexity) == 0 ||
        !liteparse_complexity_get(complexity, 0, &page) ||
        page.page_number == 0 ||
        liteparse_complexity_to_json(complexity, &json, &error) !=
            LITEPARSE_STATUS_OK ||
        strstr(json, "needs_ocr") == NULL) {
      fprintf(stderr, "%s\n", error ? error : "complexity failed");
      status = LITEPARSE_STATUS_PARSE_ERROR;
    }
  }
  liteparse_complexity_free(complexity);
  liteparse_string_free(json);
  liteparse_string_free(error);
  json = NULL;
  error = NULL;

  LiteParseSession *session = NULL;
  if (status == LITEPARSE_STATUS_OK) {
    status = liteparse_session_open_path(parser, argv[1], 1, &session, &error);
    if (status != LITEPARSE_STATUS_OK ||
        liteparse_session_get_total_pages(session) == 0) {
      fprintf(stderr, "%s\n", error ? error : "session open failed");
      status = LITEPARSE_STATUS_PARSE_ERROR;
    }
  }
  uint32_t batch_pages = 0;
  while (status == LITEPARSE_STATUS_OK) {
    LiteParseResult *batch = NULL;
    uint32_t start_page = 0;
    uint32_t end_page = 0;
    status = liteparse_session_next_batch(session, &batch, &start_page,
                                          &end_page, &error);
    if (status != LITEPARSE_STATUS_OK) {
      fprintf(stderr, "%s\n", error ? error : "next batch failed");
      break;
    }
    if (batch == NULL) break;
    if (end_page < start_page ||
        liteparse_result_get_page_count(batch) == 0) {
      status = LITEPARSE_STATUS_PARSE_ERROR;
    }
    batch_pages += end_page - start_page + 1;
    liteparse_result_free(batch);
  }
  if (status == LITEPARSE_STATUS_OK &&
      batch_pages != liteparse_session_get_total_pages(session)) {
    fprintf(stderr, "batches yielded %u of %u pages\n", batch_pages,
            liteparse_session_get_total_pages(session));
    status = LITEPARSE_STATUS_PARSE_ERROR;
  }
  liteparse_session_free(session);
  liteparse_string_free(error);
  liteparse_parser_free(parser);
  return status == LITEPARSE_STATUS_OK ? 0 : 1;
}
