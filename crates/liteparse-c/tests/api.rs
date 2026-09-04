use std::ffi::{c_char, c_void};
use std::ptr;

use liteparse_c::*;

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../integration_tests_data")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn blank_pdf(width: u32, height: u32) -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] >>").into_bytes(),
    ])
}

/// Four cropped pages with a non-default user unit, one per rotation.
fn rotated_and_cropped_pdf() -> Vec<u8> {
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>".to_vec(),
    ];
    for rotation in [0, 90, 180, 270] {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 200] \
                 /CropBox [50 25 350 175] /Rotate {rotation} /UserUnit 1.5 >>"
            )
            .into_bytes(),
        );
    }
    assemble(&objects)
}

/// One page with byte-identical links in distinct PDF objects.
fn twin_links_pdf() -> Vec<u8> {
    let link = b"<< /Type /Annot /Subtype /Link /Rect [72 700 300 720] \
                 /A << /S /URI /URI (https://example.invalid/twin) >> >>"
        .to_vec();
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R] >>".to_vec(),
        link.clone(),
        link,
    ])
}

fn assemble(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

fn view(bytes: &[u8]) -> LiteParseByteView {
    LiteParseByteView {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn view_str(view: LiteParseByteView) -> String {
    if view.ptr.is_null() {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
    String::from_utf8(bytes.to_vec()).expect("library views are UTF-8")
}

fn slice<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

fn last_error_contains(fragment: &str) {
    let message = view_str(liteparse_last_error());
    assert!(message.contains(fragment), "last error was: {message}");
}

struct Parser(*mut LiteParseParser);

impl Parser {
    fn new(tweak: impl FnOnce(&mut LiteParseConfig)) -> Self {
        let mut config = liteparse_config_default();
        config.bools_set |= LITEPARSE_FLAG_QUIET | LITEPARSE_FLAG_OCR_ENABLED;
        config.bools_values |= LITEPARSE_FLAG_QUIET;
        tweak(&mut config);
        let created = unsafe { liteparse_parser_new(&config) };
        assert_eq!(
            created.status,
            LITEPARSE_STATUS_OK,
            "{}",
            view_str(liteparse_last_error())
        );
        Self(created.handle)
    }

    fn plain() -> Self {
        Self::new(|_| {})
    }

    fn open(&self, name: &str) -> Document {
        let path = fixture(name);
        let opened = unsafe { liteparse_document_open_path(self.0, view(path.as_bytes())) };
        assert_eq!(
            opened.status,
            LITEPARSE_STATUS_OK,
            "{}",
            view_str(liteparse_last_error())
        );
        Document(opened.handle)
    }

    fn open_bytes(&self, bytes: &[u8]) -> Document {
        let opened = unsafe { liteparse_document_open_bytes(self.0, bytes.as_ptr(), bytes.len()) };
        assert_eq!(
            opened.status,
            LITEPARSE_STATUS_OK,
            "{}",
            view_str(liteparse_last_error())
        );
        Document(opened.handle)
    }
}

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe { liteparse_parser_free(self.0) };
    }
}

struct Document(*mut LiteParseDocument);

impl Document {
    fn parse(&self, pages: &[u32]) -> Result {
        let parsed = unsafe { liteparse_document_parse(self.0, pages.as_ptr(), pages.len()) };
        assert_eq!(
            parsed.status,
            LITEPARSE_STATUS_OK,
            "{}",
            view_str(liteparse_last_error())
        );
        Result(parsed.handle)
    }

    fn screenshot(
        &self,
        dpi: f32,
        region: Option<LiteParseRenderRegion>,
    ) -> LiteParseScreenshotsNew {
        unsafe {
            liteparse_document_screenshot(
                self.0,
                ptr::null(),
                0,
                dpi,
                region.as_ref().map_or(ptr::null(), ptr::from_ref),
            )
        }
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        unsafe { liteparse_document_free(self.0) };
    }
}

struct Result(*mut LiteParseResult);

impl Drop for Result {
    fn drop(&mut self) {
        unsafe { liteparse_result_free(self.0) };
    }
}

#[test]
fn version_is_a_static_c_string() {
    let version = unsafe { std::ffi::CStr::from_ptr(liteparse_version()) };
    assert_eq!(version.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn last_error_starts_empty_and_fills_on_failure() {
    let created = unsafe { liteparse_parser_new(ptr::null()) };
    assert_eq!(created.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    assert!(created.handle.is_null());
    last_error_contains("config must not be null");
}

#[test]
fn parser_new_validates_config_fields() {
    let mut config = liteparse_config_default();
    config.dpi = -1.0;
    assert_eq!(
        unsafe { liteparse_parser_new(&config) }.status,
        LITEPARSE_STATUS_INVALID_CONFIG
    );
    last_error_contains("dpi");

    let mut config = liteparse_config_default();
    config.has_crop_box = true;
    config.crop_box = [0.9, 0.2, 0.9, 0.2];
    assert_eq!(
        unsafe { liteparse_parser_new(&config) }.status,
        LITEPARSE_STATUS_INVALID_CONFIG
    );
    last_error_contains("crop_box");

    let mut config = liteparse_config_default();
    config.image_output_dir = view(b"out");
    assert_eq!(
        unsafe { liteparse_parser_new(&config) }.status,
        LITEPARSE_STATUS_INVALID_CONFIG
    );
    last_error_contains("image_output_dir requires");

    let mut config = liteparse_config_default();
    config.size_of_config = 7;
    assert_eq!(
        unsafe { liteparse_parser_new(&config) }.status,
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("size_of_config");
}

#[test]
fn font_db_dir_defaults_to_absent_and_rejects_bad_views() {
    let config = liteparse_config_default();
    assert!(config.font_db_dir.ptr.is_null());
    assert_eq!(config.font_db_dir.len, 0);

    let mut config = liteparse_config_default();
    config.font_db_dir = LiteParseByteView {
        ptr: b"\xff\xfe".as_ptr(),
        len: 2,
    };
    assert_eq!(
        unsafe { liteparse_parser_new(&config) }.status,
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("font_db_dir");

    let mut config = liteparse_config_default();
    config.font_db_dir = LiteParseByteView {
        ptr: ptr::null(),
        len: 3,
    };
    assert_eq!(
        unsafe { liteparse_parser_new(&config) }.status,
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("font_db_dir");
}

#[test]
fn font_db_dir_is_copied_and_a_missing_directory_never_fails_extraction() {
    for dir in [Some(String::from("/nonexistent/liteparse/font/db")), None] {
        let parser = Parser::new(|config| {
            if let Some(dir) = &dir {
                config.font_db_dir = view(dir.as_bytes());
            }
        });
        drop(dir);
        let result = parser.open("sample.pdf").parse(&[]);
        assert!(!view_str(unsafe { liteparse_result_text(result.0) }).is_empty());
    }
}

#[test]
fn font_db_dir_parsers_stay_isolated_under_concurrency() {
    let bytes = std::fs::read(fixture("sample.pdf")).unwrap();
    fn parse_with(dir: &str, bytes: &[u8]) -> [String; 2] {
        let parser = Parser::new(|config| config.font_db_dir = view(dir.as_bytes()));
        let from_path = parser.open("sample.pdf").parse(&[]);
        let from_bytes = parser.open_bytes(bytes).parse(&[]);
        [
            view_str(unsafe { liteparse_result_text(from_path.0) }),
            view_str(unsafe { liteparse_result_text(from_bytes.0) }),
        ]
    }
    let texts: Vec<String> = std::thread::scope(|scope| {
        let ha = scope.spawn(|| parse_with("/nonexistent/liteparse/font/db-a", &bytes));
        let hb = scope.spawn(|| parse_with("/nonexistent/liteparse/font/db-b", &bytes));
        [ha.join().unwrap(), hb.join().unwrap()].concat()
    });
    assert!(texts.iter().all(|t| !t.is_empty() && t == &texts[0]));
}

#[test]
fn null_handles_degrade_to_empty_without_crashing() {
    assert_eq!(unsafe { liteparse_document_total_pages(ptr::null()) }, 0);
    assert_eq!(unsafe { liteparse_result_page_count(ptr::null()) }, 0);
    let mut len = 7usize;
    let items = unsafe { liteparse_result_text_items(ptr::null(), 0, &mut len) };
    assert!(items.is_null());
    assert_eq!(len, 0);
    last_error_contains("result must not be null");
    let parsed = unsafe { liteparse_document_parse(ptr::null(), ptr::null(), 0) };
    assert_eq!(parsed.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    unsafe {
        liteparse_document_free(ptr::null_mut());
        liteparse_result_free(ptr::null_mut());
        liteparse_parser_free(ptr::null_mut());
    }
}

#[test]
fn document_reports_pages_and_parses_typed_slices() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
        config.bools_values |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
    });
    let document = parser.open("sample.pdf");
    let total = unsafe { liteparse_document_total_pages(document.0) };
    assert!(total >= 1);

    let result = document.parse(&[]);
    let page_count = unsafe { liteparse_result_page_count(result.0) };
    assert_eq!(page_count as u32, total);
    assert_eq!(unsafe { liteparse_result_total_pages(result.0) }, total);
    assert_eq!(unsafe { liteparse_result_page_number(result.0, 0) }, 1);
    assert!(!view_str(unsafe { liteparse_result_text(result.0) }).is_empty());
    assert!(!view_str(unsafe { liteparse_result_page_text(result.0, 0) }).is_empty());

    let mut count = 0usize;
    let items = slice(
        unsafe { liteparse_result_text_items(result.0, 0, &mut count) },
        count,
    );
    assert!(!items.is_empty());
    assert!(!view_str(items[0].text).is_empty());

    let size = unsafe { liteparse_result_page_size(result.0, 0) };
    assert!(size.width > 0.0 && size.height > 0.0);

    let found_words = (0..items.len()).any(|index| {
        let mut words = 0usize;
        let boxes = slice(
            unsafe { liteparse_result_word_boxes(result.0, 0, index, &mut words) },
            words,
        );
        boxes.iter().any(|word| word.text.len > 0)
    });
    assert!(found_words, "emit_word_boxes should populate word boxes");
}

#[test]
fn page_selections_are_validated_deduplicated_and_ordered_everywhere() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let total = unsafe { liteparse_document_total_pages(document.0) };

    let whole = document.parse(&[]);
    assert_eq!(
        unsafe { liteparse_result_page_count(whole.0) } as u32,
        total
    );

    let selection = [total, 1, total];
    let expected: Vec<u32> = if total == 1 { vec![1] } else { vec![1, total] };
    let parsed = document.parse(&selection);
    let got: Vec<u32> = (0..expected.len())
        .map(|index| unsafe { liteparse_result_page_number(parsed.0, index) })
        .collect();
    assert_eq!(got, expected);

    let shots = unsafe {
        liteparse_document_screenshot(
            document.0,
            selection.as_ptr(),
            selection.len(),
            0.0,
            ptr::null(),
        )
    };
    assert_eq!(shots.status, LITEPARSE_STATUS_OK);
    let mut count = 0usize;
    let rendered = slice(
        unsafe { liteparse_screenshots_slice(shots.handle, &mut count) },
        count,
    );
    let got: Vec<u32> = rendered.iter().map(|shot| shot.page_number).collect();
    assert_eq!(got, expected);
    unsafe { liteparse_screenshots_free(shots.handle) };

    let stats =
        unsafe { liteparse_document_complexity(document.0, selection.as_ptr(), selection.len()) };
    assert_eq!(stats.status, LITEPARSE_STATUS_OK);
    let pages = slice(
        unsafe { liteparse_complexity_slice(stats.handle, &mut count) },
        count,
    );
    let got: Vec<u32> = pages.iter().map(|page| page.page_number as u32).collect();
    assert_eq!(got, expected);
    unsafe { liteparse_complexity_free(stats.handle) };

    for bad in [[0u32], [total + 1]] {
        let parsed = unsafe { liteparse_document_parse(document.0, bad.as_ptr(), 1) };
        assert_eq!(parsed.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        last_error_contains("out of range");
        let shots =
            unsafe { liteparse_document_screenshot(document.0, bad.as_ptr(), 1, 0.0, ptr::null()) };
        assert_eq!(shots.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    }
}

#[test]
fn continue_on_page_error_never_swallows_argument_errors() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
        config.bools_values |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
    });
    let document = parser.open("sample.pdf");
    let outside = LiteParseRenderRegion {
        x: 0.0,
        y: 0.0,
        width: 99999.0,
        height: 1.0,
    };
    let rejected = document.screenshot(0.0, Some(outside));
    assert_eq!(rejected.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    assert!(rejected.handle.is_null());
    let bad = [0u32];
    let rejected =
        unsafe { liteparse_document_screenshot(document.0, bad.as_ptr(), 1, 0.0, ptr::null()) };
    assert_eq!(rejected.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
}

#[test]
fn open_bytes_matches_open_path() {
    let parser = Parser::plain();
    let bytes = std::fs::read(fixture("sample.pdf")).unwrap();
    let opened = unsafe { liteparse_document_open_bytes(parser.0, bytes.as_ptr(), bytes.len()) };
    assert_eq!(opened.status, LITEPARSE_STATUS_OK);
    let document = Document(opened.handle);
    drop(bytes);
    let from_bytes = document.parse(&[]);
    let from_path = parser.open("sample.pdf").parse(&[]);
    assert_eq!(
        view_str(unsafe { liteparse_result_text(from_bytes.0) }),
        view_str(unsafe { liteparse_result_text(from_path.0) })
    );
}

#[test]
fn json_is_cached_and_borrowed_from_the_handle() {
    let parser = Parser::plain();
    let result = parser.open("sample.pdf").parse(&[]);

    let mut first = LiteParseByteView::default();
    assert_eq!(
        unsafe { liteparse_result_to_json(result.0, &mut first) },
        LITEPARSE_STATUS_OK
    );
    let json = view_str(first);
    assert!(json.contains("\"pages\"") && json.contains("\"text\""));

    let mut second = LiteParseByteView::default();
    assert_eq!(
        unsafe { liteparse_result_to_json(result.0, &mut second) },
        LITEPARSE_STATUS_OK
    );
    assert_eq!(first.ptr, second.ptr, "second call must hit the cache");
}

#[test]
fn search_matches_outlive_the_result_handle() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let result = document.parse(&[]);
    let page_text = view_str(unsafe { liteparse_result_page_text(result.0, 0) });
    let word = page_text
        .split_whitespace()
        .next()
        .expect("page has text")
        .to_owned();

    let found = unsafe { liteparse_result_search(result.0, 0, view(word.as_bytes()), false) };
    assert_eq!(found.status, LITEPARSE_STATUS_OK);
    let mut count = 0usize;
    let matches = slice(
        unsafe { liteparse_search_matches_slice(found.handle, &mut count) },
        count,
    );
    assert!(!matches.is_empty());
    drop(result);
    assert!(!view_str(matches[0].text).is_empty());
    unsafe { liteparse_search_matches_free(found.handle) };

    let again = document.parse(&[]);
    let missing = unsafe { liteparse_result_search(again.0, 99, view(b"x"), false) };
    assert_eq!(missing.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("page_index 99");
}

#[test]
fn complexity_reason_bits_match_needs_ocr() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let created = unsafe { liteparse_document_complexity(document.0, ptr::null(), 0) };
    assert_eq!(created.status, LITEPARSE_STATUS_OK);

    let mut count = 0usize;
    let pages = slice(
        unsafe { liteparse_complexity_slice(created.handle, &mut count) },
        count,
    );
    assert_eq!(count as u32, unsafe {
        liteparse_document_total_pages(document.0)
    });
    for page in pages {
        assert_eq!(page.needs_ocr, page.reasons_mask != 0);
    }
    assert!(view_str(unsafe { liteparse_complexity_json(created.handle) }).starts_with('['));
    unsafe { liteparse_complexity_free(created.handle) };
}

#[test]
fn screenshots_render_whole_pages_and_cropped_regions() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
        config.bools_values |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
    });
    let document = parser.open("sample.pdf");

    let full = document.screenshot(0.0, None);
    assert_eq!(full.status, LITEPARSE_STATUS_OK);
    let mut count = 0usize;
    let shots = slice(
        unsafe { liteparse_screenshots_slice(full.handle, &mut count) },
        count,
    );
    assert_eq!(count as u32, unsafe {
        liteparse_document_total_pages(document.0)
    });
    assert_eq!(&slice(shots[0].png.ptr, shots[0].png.len)[..4], b"\x89PNG");
    let mut rect_count = 0usize;
    let _ = unsafe { liteparse_screenshots_rects(full.handle, 0, &mut rect_count) };
    unsafe { liteparse_screenshots_free(full.handle) };

    let region = LiteParseRenderRegion {
        x: 10.3,
        y: 20.7,
        width: 100.2,
        height: 50.4,
    };
    let clipped = document.screenshot(144.0, Some(region));
    assert_eq!(
        clipped.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );
    let shots = slice(
        unsafe { liteparse_screenshots_slice(clipped.handle, &mut count) },
        count,
    );
    let expect = |pt: f32| (pt * 144.0 / 72.0).round() as u32;
    assert_eq!(
        (shots[0].width, shots[0].height),
        (expect(region.width), expect(region.height))
    );
    let rects = slice(
        unsafe { liteparse_screenshots_rects(clipped.handle, 0, &mut rect_count) },
        rect_count,
    );
    for rect in rects {
        assert!(rect.x >= 0.0 && rect.y >= 0.0);
        assert!(rect.x + rect.width <= region.width + 0.01);
        assert!(rect.y + rect.height <= region.height + 0.01);
    }
    unsafe { liteparse_screenshots_free(clipped.handle) };

    let outside = LiteParseRenderRegion {
        x: 0.0,
        y: 0.0,
        width: 99999.0,
        height: 1.0,
    };
    assert_eq!(
        document.screenshot(0.0, Some(outside)).status,
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("region");
    assert_eq!(
        document.screenshot(-3.0, None).status,
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("dpi_override");
}

#[test]
fn effective_dpi_reports_the_long_edge_cap_on_both_render_paths() {
    let parser = Parser::new(|config| {
        config.dpi = 400.0;
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
    });
    let document = parser.open_bytes(&blank_pdf(7_200, 72));

    let rendered = document.screenshot(0.0, None);
    assert_eq!(rendered.status, LITEPARSE_STATUS_OK);
    let mut count = 0usize;
    let shots = slice(
        unsafe { liteparse_screenshots_slice(rendered.handle, &mut count) },
        count,
    );
    assert_eq!(count, 1);
    assert_eq!(shots[0].width, 30_000);
    assert_eq!(shots[0].effective_dpi, 300.0);
    unsafe { liteparse_screenshots_free(rendered.handle) };

    let parsed = document.parse(&[]);
    let shots = slice(
        unsafe { liteparse_result_screenshots(parsed.0, &mut count) },
        count,
    );
    assert_eq!(count, 1);
    assert_eq!(shots[0].width, 30_000);
    assert_eq!(shots[0].effective_dpi, 300.0);
}

#[test]
fn region_render_is_identical_whether_or_not_rects_are_detected() {
    let region = LiteParseRenderRegion {
        x: 11.4,
        y: 23.6,
        width: 97.3,
        height: 44.1,
    };
    let png = |detect: bool| {
        let parser = Parser::new(|config| {
            config.bools_set |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
            if detect {
                config.bools_values |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
            }
        });
        let document = parser.open("sample.pdf");
        let shots = document.screenshot(150.0, Some(region));
        assert_eq!(shots.status, LITEPARSE_STATUS_OK);
        let mut count = 0usize;
        let rendered = slice(
            unsafe { liteparse_screenshots_slice(shots.handle, &mut count) },
            count,
        );
        let first = &rendered[0];
        let bytes = slice(first.png.ptr, first.png.len).to_vec();
        let size = (first.width, first.height);
        unsafe { liteparse_screenshots_free(shots.handle) };
        (size, bytes)
    };

    let (detected_size, detected) = png(true);
    let (plain_size, plain) = png(false);
    assert_eq!(detected_size, plain_size);
    assert!(!plain.is_empty());
    assert_eq!(detected, plain, "crop paths disagree on pixels");
}

#[test]
fn extract_screenshots_flag_attaches_rendered_pages_to_the_result() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
    });
    let result = parser.open("sample.pdf").parse(&[]);
    let mut count = 0usize;
    let shots = slice(
        unsafe { liteparse_result_screenshots(result.0, &mut count) },
        count,
    );
    assert_eq!(count, unsafe { liteparse_result_page_count(result.0) });
    assert!(shots.iter().all(|shot| shot.png.len > 0 && shot.width > 0));

    let plain = Parser::plain().open("sample.pdf").parse(&[]);
    let mut count = 1usize;
    assert!(unsafe { liteparse_result_screenshots(plain.0, &mut count) }.is_null());
    assert_eq!(count, 0);
}

#[test]
fn markdown_format_fills_page_markdown_and_keeps_page_text_plain() {
    let parser = Parser::new(|config| config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN);
    let result = parser.open("sample.pdf").parse(&[]);
    let page_count = unsafe { liteparse_result_page_count(result.0) };
    let whole = view_str(unsafe { liteparse_result_text(result.0) });
    let per_page: Vec<String> = (0..page_count)
        .map(|index| view_str(unsafe { liteparse_result_page_markdown(result.0, index) }))
        .collect();
    assert!(!per_page[0].is_empty());
    assert_eq!(whole, per_page.join("\n\n-----\n\n"));
    let plain_page = view_str(unsafe { liteparse_result_page_text(result.0, 0) });
    assert!(!plain_page.is_empty());

    let plain = Parser::plain().open("sample.pdf").parse(&[]);
    assert_eq!(unsafe { liteparse_result_page_markdown(plain.0, 0) }.len, 0);
    assert_eq!(
        unsafe { liteparse_result_page_markdown(plain.0, 99) }.len,
        0
    );
}

#[test]
fn extras_accessors_degrade_to_empty_on_plain_documents() {
    let parser = Parser::plain();
    let result = parser.open("sample.pdf").parse(&[]);
    let mut len = 7usize;
    unsafe {
        assert!(liteparse_result_outline(result.0, &mut len).is_null() && len == 0);
        assert!(liteparse_result_page_errors(result.0, &mut len).is_null());
        assert!(liteparse_result_annotations(result.0, 0, &mut len).is_null());
        assert!(liteparse_result_form_fields(result.0, 0, &mut len).is_null());
        assert!(liteparse_result_structure_nodes(result.0, 0, &mut len).is_null());
        assert!(liteparse_result_blocks(result.0, 0, &mut len).is_null());
        assert!(liteparse_result_vector_shapes(result.0, 0, &mut len).is_null());
        assert!(liteparse_result_vector_lines(result.0, 0, &mut len).is_null());
        assert!(liteparse_result_xfa_packets(result.0, &mut len).is_null());
        assert!(!liteparse_result_page_complexity(result.0, 0).present);
        assert!(!liteparse_result_doc_meta(result.0).present);
        let bounds = liteparse_result_page_content_bounds(result.0, 0);
        assert!(!bounds.present || bounds.rect.width > 0.0);
        assert!(liteparse_result_page_geometry(result.0, 0).present);
        assert!(!liteparse_result_page_geometry(result.0, 99).present);
    }
}

#[test]
fn annotations_report_the_object_number_that_tells_twins_apart() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
    });
    let pdf = twin_links_pdf();
    let result = parser.open_bytes(&pdf).parse(&[]);

    let mut len = 0usize;
    let annotations = slice(
        unsafe { liteparse_result_annotations(result.0, 0, &mut len) },
        len,
    );
    assert_eq!(annotations.len(), 2);
    for annotation in annotations {
        assert_eq!(view_str(annotation.subtype), "link");
        assert_eq!(view_str(annotation.uri), "https://example.invalid/twin");
        assert!(annotation.has_object_number);
    }
    assert_ne!(
        annotations[0].object_number, annotations[1].object_number,
        "twin annotations must not share an object number"
    );
}

#[test]
fn page_geometry_reports_the_visible_box_user_unit_and_rotation() {
    let parser = Parser::plain();
    let pdf = rotated_and_cropped_pdf();
    let document = parser.open_bytes(&pdf);
    let result = document.parse(&[]);

    assert_eq!(unsafe { liteparse_result_page_count(result.0) }, 4);
    for index in 0..4usize {
        let quarter_turns = index as u32;
        let value = unsafe { liteparse_result_page_geometry(result.0, index) };
        assert!(value.present, "page {index}");
        let geometry = value.geometry;
        assert_eq!(geometry.box_left, 50.0, "page {index}");
        assert_eq!(geometry.box_bottom, 25.0, "page {index}");
        assert_eq!(geometry.box_right, 350.0, "page {index}");
        assert_eq!(geometry.box_top, 175.0, "page {index}");
        assert_eq!(geometry.user_unit, 1.5, "page {index}");
        assert!(geometry.has_rotation, "page {index}");
        assert_eq!(geometry.rotation_quarter_turns, quarter_turns);

        let size = unsafe { liteparse_result_page_size(result.0, index) };
        let width = (geometry.box_right - geometry.box_left) * geometry.user_unit;
        let height = (geometry.box_top - geometry.box_bottom) * geometry.user_unit;
        let (expected_width, expected_height) = if quarter_turns % 2 == 1 {
            (height, width)
        } else {
            (width, height)
        };
        assert!((size.width - expected_width).abs() < 0.01, "page {index}");
        assert!((size.height - expected_height).abs() < 0.01, "page {index}");
    }
}

#[test]
fn page_geometry_follows_a_page_selection_rather_than_its_position() {
    let parser = Parser::plain();
    let pdf = rotated_and_cropped_pdf();
    let document = parser.open_bytes(&pdf);
    let result = document.parse(&[4, 2]);

    assert_eq!(unsafe { liteparse_result_page_count(result.0) }, 2);
    assert_eq!(unsafe { liteparse_result_page_number(result.0, 0) }, 2);
    assert_eq!(unsafe { liteparse_result_page_number(result.0, 1) }, 4);
    assert_eq!(
        unsafe { liteparse_result_page_geometry(result.0, 0) }
            .geometry
            .rotation_quarter_turns,
        1
    );
    assert_eq!(
        unsafe { liteparse_result_page_geometry(result.0, 1) }
            .geometry
            .rotation_quarter_turns,
        3
    );
}

#[test]
fn form_fields_and_annotations_pack_per_page() {
    let parser = Parser::new(|config| {
        let flags = LITEPARSE_FLAG_EXTRACT_FORM_FIELDS
            | LITEPARSE_FLAG_EXTRACT_ANNOTATIONS
            | LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA;
        config.bools_set |= flags;
        config.bools_values |= flags;
    });
    let result = parser.open("filled_acroform.pdf").parse(&[]);
    let mut len = 0usize;
    let fields = slice(
        unsafe { liteparse_result_form_fields(result.0, 0, &mut len) },
        len,
    );
    assert!(!fields.is_empty(), "fixture carries AcroForm widgets");
    assert!(!view_str(fields[0].field_type).is_empty());
    for (index, field) in fields.iter().enumerate() {
        let mut options = 0usize;
        unsafe { liteparse_result_form_field_options(result.0, 0, index, &mut options) };
        assert_eq!(options, field.options_len);
    }
    assert!(unsafe { liteparse_result_form_type(result.0) }.present);
    assert!(unsafe { liteparse_result_doc_meta(result.0) }.present);
}

#[test]
fn text_metadata_bits_follow_the_flag() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_TEXT_METADATA;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_TEXT_METADATA;
    });
    let result = parser.open("sample.pdf").parse(&[]);
    let mut count = 0usize;
    let items = slice(
        unsafe { liteparse_result_text_items(result.0, 0, &mut count) },
        count,
    );
    assert!(items[0].has_font_is_buggy && items[0].has_trailing_space_generated);

    let plain = Parser::plain().open("sample.pdf").parse(&[]);
    let items = slice(
        unsafe { liteparse_result_text_items(plain.0, 0, &mut count) },
        count,
    );
    assert!(!items[0].has_font_is_buggy && !items[0].has_trailing_space_generated);
}

#[test]
fn one_document_serves_concurrent_operations() {
    struct SendPtr<T>(*mut T);
    unsafe impl<T> Send for SendPtr<T> {}
    unsafe impl<T> Sync for SendPtr<T> {}

    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let shared = SendPtr(document.0);
    let texts: Vec<String> = std::thread::scope(|scope| {
        (0..4)
            .map(|_| {
                let shared = &shared;
                scope.spawn(move || {
                    let parsed = unsafe { liteparse_document_parse(shared.0, ptr::null(), 0) };
                    assert_eq!(parsed.status, LITEPARSE_STATUS_OK);
                    let text = view_str(unsafe { liteparse_result_text(parsed.handle) });
                    unsafe { liteparse_result_free(parsed.handle) };
                    text
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("no unwind across the ABI"))
            .collect()
    });
    assert!(
        texts
            .iter()
            .all(|text| text == &texts[0] && !text.is_empty())
    );
}

unsafe extern "C" fn recognize_single(
    calls: *mut c_void,
    _pixels: *const u8,
    pixels_len: usize,
    width: u32,
    height: u32,
    pixel_format: u32,
    _language: *const c_char,
    _dpi: f32,
    sink: *mut LiteParseOcrSink,
) -> u32 {
    assert_eq!(pixel_format, LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE);
    assert_eq!(pixels_len, width as usize * height as usize);
    unsafe { *calls.cast::<u32>() += 1 };
    unsafe {
        liteparse_ocr_sink_add(
            sink,
            view(b"zzocrzz"),
            0.0,
            0.0,
            40.0,
            10.0,
            0.9,
            ptr::null(),
        )
    }
}

unsafe extern "C" fn recognize_batch(
    _user_data: *mut c_void,
    _pixels: *const u8,
    _pixels_len: usize,
    _width: u32,
    _height: u32,
    _pixel_format: u32,
    _language: *const c_char,
    _dpi: f32,
    sink: *mut LiteParseOcrSink,
) -> u32 {
    let blob = b"zzocrzzzzrotated";
    let words = [
        LiteParseOcrWordIn {
            text_offset: 0,
            text_length: 7,
            x1: 0.0,
            y1: 0.0,
            x2: 40.0,
            y2: 10.0,
            confidence: 0.9,
            polygon: [0.0; 8],
            has_polygon: false,
        },
        LiteParseOcrWordIn {
            text_offset: 7,
            text_length: 8,
            x1: 0.0,
            y1: 0.0,
            x2: 60.0,
            y2: 12.0,
            confidence: 0.8,
            polygon: [0.0, 0.0, 20.0, 2.0, 22.0, 12.0, 2.0, 10.0],
            has_polygon: true,
        },
    ];
    unsafe {
        liteparse_ocr_sink_add_batch(sink, blob.as_ptr(), blob.len(), words.as_ptr(), words.len())
    }
}

unsafe extern "C" fn recognize_failing(
    _user_data: *mut c_void,
    _pixels: *const u8,
    _pixels_len: usize,
    _width: u32,
    _height: u32,
    _pixel_format: u32,
    _language: *const c_char,
    _dpi: f32,
    sink: *mut LiteParseOcrSink,
) -> u32 {
    unsafe { liteparse_ocr_sink_set_error(sink, view(b"engine exploded")) };
    1
}

fn ocr_parser(
    recognize: LiteParseOcrRecognizeFn,
    user_data: *mut c_void,
    grayscale: bool,
) -> Parser {
    let parser = Parser::new(|config| {
        config.bools_values |= LITEPARSE_FLAG_OCR_ENABLED | LITEPARSE_FLAG_OCR_FAILURE_FATAL;
        config.bools_set |= LITEPARSE_FLAG_OCR_FAILURE_FATAL;
    });
    let status = unsafe {
        liteparse_parser_set_ocr_callback(
            parser.0,
            recognize,
            user_data,
            view(b"test-engine"),
            grayscale,
        )
    };
    assert_eq!(status, LITEPARSE_STATUS_OK);
    parser
}

#[test]
fn ocr_words_land_in_the_result_through_both_sink_paths() {
    let mut calls = 0u32;
    let single = ocr_parser(
        Some(recognize_single),
        ptr::from_mut(&mut calls).cast(),
        true,
    );
    let document = single.open("receipt.png");
    let result = document.parse(&[]);
    let text = view_str(unsafe { liteparse_result_text(result.0) });
    assert!(text.contains("zzocrzz"), "text was: {text}");
    drop((result, document, single));
    assert!(calls >= 1);

    let batch = ocr_parser(Some(recognize_batch), ptr::null_mut(), false);
    let document = batch.open("receipt.png");
    let result = document.parse(&[]);
    let text = view_str(unsafe { liteparse_result_text(result.0) });
    assert!(
        text.contains("zzocrzz") && text.contains("zzrotate"),
        "text was: {text}"
    );
}

#[test]
fn ocr_callback_failure_surfaces_when_fatal() {
    let parser = ocr_parser(Some(recognize_failing), ptr::null_mut(), false);
    let document = parser.open("receipt.png");
    let parsed = unsafe { liteparse_document_parse(document.0, ptr::null(), 0) };
    assert_eq!(parsed.status, LITEPARSE_STATUS_OCR_ERROR);
    assert!(parsed.handle.is_null());
    last_error_contains("engine exploded");
}

#[test]
fn clearing_the_ocr_callback_restores_the_default_engine_choice() {
    let parser = ocr_parser(Some(recognize_failing), ptr::null_mut(), false);
    let status = unsafe {
        liteparse_parser_set_ocr_callback(
            parser.0,
            None,
            ptr::null_mut(),
            LiteParseByteView::default(),
            false,
        )
    };
    assert_eq!(status, LITEPARSE_STATUS_OK);
    // Built-in OCR availability varies, so only the removed callback is checked.
    let document = parser.open("sample.pdf");
    let parsed = unsafe { liteparse_document_parse(document.0, ptr::null(), 0) };
    if parsed.status != LITEPARSE_STATUS_OK {
        assert!(!view_str(liteparse_last_error()).contains("engine exploded"));
    }
    unsafe { liteparse_result_free(parsed.handle) };
}
