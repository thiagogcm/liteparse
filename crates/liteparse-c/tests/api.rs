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

/// One tagged page: MCID 0 on "Hello" joined to a struct-tree H1.
fn tagged_heading_pdf() -> Vec<u8> {
    let contents = b"BT /F1 12 Tf 72 700 Td /P << /MCID 0 >> BDC (Hello) Tj EMC ET";
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 5 0 R >>"
            .to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
           /Resources << /Font << /F1 7 0 R >> >> /StructParents 0 >>"
            .to_vec(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", contents.len(), unsafe {
            std::str::from_utf8_unchecked(contents)
        })
        .into_bytes(),
        b"<< /Type /StructTreeRoot /K [6 0 R] /ParentTree 8 0 R >>".to_vec(),
        b"<< /Type /StructElem /S /H1 /P 5 0 R /K 0 /Pg 3 0 R >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        b"<< /Nums [0 [6 0 R]] >>".to_vec(),
    ])
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

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

/// One page: a translated filled rect, a Form XObject with a stroked rect, and a 2×2 grey image.
fn objects_pdf() -> Vec<u8> {
    let content = b"q 1 0 0 1 10 20 cm 0 0 50 30 re f Q q /Fx1 Do Q q 40 0 0 20 100 50 cm /Im1 Do Q";
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R /Fx1 6 0 R >> >> >>".to_vec(),
        stream("", content),
        stream(
            "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8",
            &[0x00, 0xff, 0xff, 0x00],
        ),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Matrix [2 0 0 2 5 5]",
            b"0 0 10 10 re S",
        ),
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

    fn parse_content(&self, content: &LiteParseContent) -> Result {
        let parsed = unsafe { liteparse_parser_parse_content(self.0, content) };
        assert_eq!(
            parsed.status,
            LITEPARSE_STATUS_OK,
            "{}",
            view_str(liteparse_last_error())
        );
        Result(parsed.handle)
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
    assert_eq!(unsafe { liteparse_raw_text_page_count(ptr::null()) }, 0);
    assert_eq!(unsafe { liteparse_extract_page_count(ptr::null()) }, 0);
    assert_eq!(unsafe { liteparse_page_objects_page_count(ptr::null()) }, 0);
    assert!(
        unsafe { liteparse_result_page_label(ptr::null(), 0) }
            .ptr
            .is_null()
    );
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
        liteparse_page_objects_free(ptr::null_mut());
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
    assert_eq!(
        view_str(unsafe { liteparse_result_page_label(result.0, 0) }),
        ""
    );
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

    let raw =
        unsafe { liteparse_document_raw_text(document.0, selection.as_ptr(), selection.len()) };
    assert_eq!(raw.status, LITEPARSE_STATUS_OK);
    let pages = slice(
        unsafe { liteparse_raw_text_pages(raw.handle, &mut count) },
        count,
    );
    let got: Vec<u32> = pages.iter().map(|page| page.page_number).collect();
    assert_eq!(got, expected);
    unsafe { liteparse_raw_text_free(raw.handle) };

    let extracted =
        unsafe { liteparse_document_extract(document.0, selection.as_ptr(), selection.len()) };
    assert_eq!(extracted.status, LITEPARSE_STATUS_OK);
    let pages = slice(
        unsafe { liteparse_extract_pages(extracted.handle, &mut count) },
        count,
    );
    let got: Vec<u32> = pages.iter().map(|page| page.page_number).collect();
    assert_eq!(got, expected);
    unsafe { liteparse_extract_free(extracted.handle) };

    let objects = unsafe {
        liteparse_document_page_objects(document.0, selection.as_ptr(), selection.len(), 0)
    };
    assert_eq!(objects.status, LITEPARSE_STATUS_OK);
    let pages = slice(
        unsafe { liteparse_page_objects_pages(objects.handle, &mut count) },
        count,
    );
    let got: Vec<u32> = pages.iter().map(|page| page.page_number).collect();
    assert_eq!(got, expected);
    unsafe { liteparse_page_objects_free(objects.handle) };

    for bad in [[0u32], [total + 1]] {
        let parsed = unsafe { liteparse_document_parse(document.0, bad.as_ptr(), 1) };
        assert_eq!(parsed.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        last_error_contains("out of range");
        let shots =
            unsafe { liteparse_document_screenshot(document.0, bad.as_ptr(), 1, 0.0, ptr::null()) };
        assert_eq!(shots.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        let raw = unsafe { liteparse_document_raw_text(document.0, bad.as_ptr(), 1) };
        assert_eq!(raw.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        let extracted = unsafe { liteparse_document_extract(document.0, bad.as_ptr(), 1) };
        assert_eq!(extracted.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        let objects = unsafe { liteparse_document_page_objects(document.0, bad.as_ptr(), 1, 0) };
        assert_eq!(objects.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
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
        let _ = liteparse_result_image_refs(result.0, 0, &mut len);
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

fn content_item(text: &[u8], y: f32) -> LiteParseTextItem {
    LiteParseTextItem {
        text: view(text),
        x: 72.0,
        y,
        width: 80.0,
        height: 12.0,
        ..LiteParseTextItem::default()
    }
}

fn content_page(number: u32, item_offset: usize) -> LiteParseContentPage {
    LiteParseContentPage {
        page_number: number,
        page_width: 612.0,
        page_height: 792.0,
        item_offset,
        item_count: 1,
        ..LiteParseContentPage::default()
    }
}

#[test]
fn raw_text_keeps_every_glyph_and_forwards_page_labels() {
    let parser = Parser::plain();
    let document = parser.open("page_labels.pdf");
    let raw = unsafe { liteparse_document_raw_text(document.0, ptr::null(), 0) };
    assert_eq!(
        raw.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );
    assert_eq!(unsafe { liteparse_raw_text_page_count(raw.handle) }, 4);

    let mut page_len = 0usize;
    let pages = slice(
        unsafe { liteparse_raw_text_pages(raw.handle, &mut page_len) },
        page_len,
    );
    assert_eq!(page_len, 4);
    let labels: Vec<String> = pages
        .iter()
        .map(|page| view_str(page.page_label))
        .collect();
    assert_eq!(labels, ["i", "ii", "1", "2"]);
    assert!(pages.iter().all(|page| page.has_geometry && page.page_width > 0.0));

    let mut item_len = 0usize;
    let items = slice(
        unsafe { liteparse_raw_text_items(raw.handle, 0, &mut item_len) },
        item_len,
    );
    assert!(!items.is_empty());
    let parsed = document.parse(&[]);
    let mut parsed_len = 0usize;
    let parsed_items = slice(
        unsafe { liteparse_result_text_items(parsed.0, 0, &mut parsed_len) },
        parsed_len,
    );
    assert!(
        items.len() >= parsed_items.len(),
        "raw items should not be coarser than projected text items"
    );
    for item in items {
        assert!((0.0..std::f32::consts::TAU).contains(&item.angle_radians));
        assert!(item.width >= 0.0 && item.height >= 0.0);
        if item.grounding_bounds.present {
            assert!(item.grounding_bounds.rect.width >= 0.0);
            assert!(item.grounding_bounds.rect.height >= 0.0);
        }
        if item.has_baseline_gap {
            assert!(item.baseline_gap.is_finite());
        }
        if item.glyph_names_len > 0 {
            assert!(item.has_glyph_names);
            assert!(!item.glyph_names.is_null());
        }
        if item.char_codes_len > 0 {
            assert!(!item.char_codes.is_null());
        }
        if !view_str(item.text).is_empty() || item.char_codes_len > 0 {
            continue;
        }
        panic!("empty raw item with no char codes");
    }

    let last = 4u32;
    let last_only = unsafe { liteparse_document_raw_text(document.0, &last, 1) };
    assert_eq!(last_only.status, LITEPARSE_STATUS_OK);
    assert_eq!(unsafe { liteparse_raw_text_page_count(last_only.handle) }, 1);
    let mut one = 0usize;
    let only = slice(
        unsafe { liteparse_raw_text_pages(last_only.handle, &mut one) },
        one,
    );
    assert_eq!(only[0].page_number, 4);
    assert_eq!(view_str(only[0].page_label), "2");
    unsafe { liteparse_raw_text_free(last_only.handle) };
    unsafe { liteparse_raw_text_free(raw.handle) };
}

#[test]
fn raw_text_includes_form_widget_appearance_text() {
    let document = Parser::plain().open("filled_acroform.pdf");
    let raw = unsafe { liteparse_document_raw_text(document.0, ptr::null(), 0) };
    assert_eq!(raw.status, LITEPARSE_STATUS_OK);
    let mut item_len = 0usize;
    let items = slice(
        unsafe { liteparse_raw_text_items(raw.handle, 0, &mut item_len) },
        item_len,
    );
    let text: String = items.iter().map(|item| view_str(item.text)).collect();
    assert!(text.contains("ACROFORM-CUSTOMER-7319"), "raw text: {text}");
    unsafe { liteparse_raw_text_free(raw.handle) };
}

#[test]
fn raw_text_honors_max_pages() {
    let parser = Parser::new(|config| config.max_pages = 1);
    let document = parser.open("page_labels.pdf");
    let raw = unsafe { liteparse_document_raw_text(document.0, ptr::null(), 0) };
    assert_eq!(raw.status, LITEPARSE_STATUS_OK);
    assert_eq!(unsafe { liteparse_raw_text_page_count(raw.handle) }, 1);
    let mut len = 0usize;
    let pages = slice(
        unsafe { liteparse_raw_text_pages(raw.handle, &mut len) },
        len,
    );
    assert_eq!(pages[0].page_number, 1);
    unsafe { liteparse_raw_text_free(raw.handle) };
}

#[test]
fn extract_forwards_page_labels_and_skips_projection() {
    let parser = Parser::plain();
    let document = parser.open("page_labels.pdf");
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(
        extracted.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );
    assert_eq!(unsafe { liteparse_extract_page_count(extracted.handle) }, 4);

    let mut page_len = 0usize;
    let pages = slice(
        unsafe { liteparse_extract_pages(extracted.handle, &mut page_len) },
        page_len,
    );
    let labels: Vec<String> = pages
        .iter()
        .map(|page| view_str(page.page_label))
        .collect();
    assert_eq!(labels, ["i", "ii", "1", "2"]);
    assert_eq!(
        view_str(unsafe { liteparse_extract_page_label(extracted.handle, 0) }),
        "i"
    );
    assert!(unsafe { liteparse_extract_page_geometry(extracted.handle, 0) }.present);

    let mut item_len = 0usize;
    let items = slice(
        unsafe { liteparse_extract_items(extracted.handle, &mut item_len) },
        item_len,
    );
    assert!(!items.is_empty());
    assert_eq!(
        item_len,
        pages.iter().map(|page| page.item_count).sum::<usize>()
    );
    let parsed = document.parse(&[]);
    let mut parsed_len = 0usize;
    let parsed_items = slice(
        unsafe { liteparse_result_text_items(parsed.0, 0, &mut parsed_len) },
        parsed_len,
    );
    assert!(
        pages[0].item_count >= parsed_items.len(),
        "extract items are pre-projection and should not be coarser than parse"
    );

    let last = 4u32;
    let last_only = unsafe { liteparse_document_extract(document.0, &last, 1) };
    assert_eq!(last_only.status, LITEPARSE_STATUS_OK);
    assert_eq!(unsafe { liteparse_extract_page_count(last_only.handle) }, 1);
    let mut one = 0usize;
    let only = slice(
        unsafe { liteparse_extract_pages(last_only.handle, &mut one) },
        one,
    );
    assert_eq!(only[0].page_number, 4);
    assert_eq!(view_str(only[0].page_label), "2");
    unsafe { liteparse_extract_free(last_only.handle) };
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn extract_honors_max_pages() {
    let parser = Parser::new(|config| config.max_pages = 1);
    let document = parser.open("page_labels.pdf");
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(extracted.status, LITEPARSE_STATUS_OK);
    assert_eq!(unsafe { liteparse_extract_page_count(extracted.handle) }, 1);
    let mut len = 0usize;
    let pages = slice(
        unsafe { liteparse_extract_pages(extracted.handle, &mut len) },
        len,
    );
    assert_eq!(pages[0].page_number, 1);
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn extract_as_content_round_trips_into_parse_content() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(
        extracted.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );
    let content = unsafe { liteparse_extract_as_content(extracted.handle) };
    assert_eq!(content.size_of_content, std::mem::size_of::<LiteParseContent>());
    assert!(content.pages_len > 0 && content.items_len > 0);
    let from_extract = parser.parse_content(&content);
    let from_parse = document.parse(&[]);
    let extract_text = view_str(unsafe { liteparse_result_page_text(from_extract.0, 0) });
    let parse_text = view_str(unsafe { liteparse_result_page_text(from_parse.0, 0) });
    assert!(!extract_text.is_empty());
    assert_eq!(extract_text, parse_text);
    let mut refs = 0usize;
    unsafe { liteparse_extract_image_refs(extracted.handle, 0, &mut refs) };
    let mut parsed_refs = 0usize;
    unsafe { liteparse_result_image_refs(from_parse.0, 0, &mut parsed_refs) };
    assert_eq!(refs, parsed_refs);
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn extract_annotations_match_parse_on_twin_links() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
    });
    let pdf = twin_links_pdf();
    let document = parser.open_bytes(&pdf);
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(extracted.status, LITEPARSE_STATUS_OK);
    let mut len = 0usize;
    let annotations = slice(
        unsafe { liteparse_extract_annotations(extracted.handle, 0, &mut len) },
        len,
    );
    assert_eq!(annotations.len(), 2);
    assert_ne!(
        annotations[0].object_number,
        annotations[1].object_number
    );
    for annotation in annotations {
        assert_eq!(view_str(annotation.subtype), "link");
        assert_eq!(view_str(annotation.uri), "https://example.invalid/twin");
    }
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn extract_geometry_matches_parse() {
    let parser = Parser::plain();
    let pdf = rotated_and_cropped_pdf();
    let document = parser.open_bytes(&pdf);
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(extracted.status, LITEPARSE_STATUS_OK);
    let result = document.parse(&[]);
    for index in 0..4usize {
        let from_extract = unsafe { liteparse_extract_page_geometry(extracted.handle, index) };
        let from_parse = unsafe { liteparse_result_page_geometry(result.0, index) };
        assert!(from_extract.present && from_parse.present);
        assert_eq!(
            from_extract.geometry.box_left,
            from_parse.geometry.box_left
        );
        assert_eq!(
            from_extract.geometry.rotation_quarter_turns,
            from_parse.geometry.rotation_quarter_turns
        );
        assert_eq!(
            from_extract.geometry.user_unit,
            from_parse.geometry.user_unit
        );
    }
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn extract_word_boxes_ride_as_content_when_requested() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
        config.bools_values |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
    });
    let document = parser.open("sample.pdf");
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(extracted.status, LITEPARSE_STATUS_OK);
    let mut page_len = 0usize;
    let pages = slice(
        unsafe { liteparse_extract_pages(extracted.handle, &mut page_len) },
        page_len,
    );
    let mut item_len = 0usize;
    let items = slice(
        unsafe { liteparse_extract_items(extracted.handle, &mut item_len) },
        item_len,
    );
    let page0 = &items[pages[0].item_offset..pages[0].item_offset + pages[0].item_count];
    let first_with_words = page0
        .iter()
        .position(|item| item.word_count > 0)
        .expect("emit_word_boxes should populate at least one item");
    let mut words = 0usize;
    let boxes = slice(
        unsafe { liteparse_extract_word_boxes(extracted.handle, 0, first_with_words, &mut words) },
        words,
    );
    assert_eq!(words, items[first_with_words].word_count);
    assert!(boxes.iter().any(|word| word.text.len > 0));
    let content = unsafe { liteparse_extract_as_content(extracted.handle) };
    assert!(content.words_len >= words);
    let projected = parser.parse_content(&content);
    assert!(!view_str(unsafe { liteparse_result_page_text(projected.0, 0) }).is_empty());
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn extract_forms_report_flatten_and_form_type() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_FORM_FIELDS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_FORM_FIELDS;
    });
    let document = parser.open("filled_acroform.pdf");
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(
        extracted.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );
    let mut len = 0usize;
    let fields = slice(
        unsafe { liteparse_extract_form_fields(extracted.handle, 0, &mut len) },
        len,
    );
    assert!(!fields.is_empty());
    assert!(unsafe { liteparse_extract_form_type(extracted.handle) }.present);
    let mut flattened = 0usize;
    unsafe { liteparse_extract_flattened_page_numbers(extracted.handle, &mut flattened) };
    assert_eq!(
        flattened > 0,
        unsafe { liteparse_extract_flattened_form_widgets(extracted.handle) }
    );
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn parse_content_applies_crop_box_and_skip_diagonal() {
    let parser = Parser::new(|config| {
        config.has_crop_box = true;
        config.crop_box = [0.0, 0.0, 0.5, 0.0];
        config.bools_set |= LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT;
        config.bools_values |= LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT;
    });
    let kept = content_item(b"keep", 80.0);
    let mut cropped = content_item(b"cropped", 500.0);
    cropped.y = 500.0;
    let mut diagonal = content_item(b"skew", 100.0);
    diagonal.rotation = 51.0;
    let items = [kept, cropped, diagonal];
    let page = LiteParseContentPage {
        page_number: 1,
        page_width: 612.0,
        page_height: 792.0,
        item_count: 3,
        ..LiteParseContentPage::default()
    };
    let mut content = liteparse_content_default();
    content.pages = &page;
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = items.len();
    let result = parser.parse_content(&content);
    let text = view_str(unsafe { liteparse_result_page_text(result.0, 0) });
    assert!(text.contains("keep"), "{text}");
    assert!(!text.contains("cropped"), "{text}");
    assert!(!text.contains("skew"), "{text}");
}

#[test]
fn parse_content_uses_struct_nodes_for_markdown_headings() {
    let parser = Parser::new(|config| config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN);
    let mut item = content_item(b"Tagged Heading", 72.0);
    item.mcid = 7;
    item.has_mcid = true;
    item.font_size = 12.0;
    item.has_font_size = true;
    let mcids = [7i32];
    let node = LiteParseStructNode {
        role: view(b"H1"),
        mcids: mcids.as_ptr(),
        mcids_len: 1,
        ..LiteParseStructNode::default()
    };
    let page = LiteParseContentPage {
        page_number: 1,
        page_width: 612.0,
        page_height: 792.0,
        item_count: 1,
        struct_count: 1,
        ..LiteParseContentPage::default()
    };
    let mut content = liteparse_content_default();
    content.pages = &page;
    content.pages_len = 1;
    content.items = &item;
    content.items_len = 1;
    content.struct_nodes = &node;
    content.struct_nodes_len = 1;
    let result = parser.parse_content(&content);
    let markdown = view_str(unsafe { liteparse_result_page_markdown(result.0, 0) });
    assert!(
        markdown.contains("# Tagged Heading"),
        "expected tagged heading, got {markdown:?}"
    );
}

#[test]
fn extract_as_content_keeps_mcid_without_text_metadata() {
    let parser = Parser::new(|config| config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN);
    let pdf = tagged_heading_pdf();
    let document = parser.open_bytes(&pdf);
    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(
        extracted.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );

    let mut item_len = 0usize;
    let items = slice(
        unsafe { liteparse_extract_items(extracted.handle, &mut item_len) },
        item_len,
    );
    let mut node_len = 0usize;
    let nodes = slice(
        unsafe { liteparse_extract_struct_nodes(extracted.handle, &mut node_len) },
        node_len,
    );
    assert!(
        items.iter().any(|item| item.has_mcid),
        "extract must keep mcid without EXTRACT_TEXT_METADATA"
    );
    assert!(
        nodes.iter().any(|node| view_str(node.role).eq_ignore_ascii_case("H1")),
        "expected an H1 struct node"
    );

    let content = unsafe { liteparse_extract_as_content(extracted.handle) };
    let result = parser.parse_content(&content);
    let markdown = view_str(unsafe { liteparse_result_page_markdown(result.0, 0) });
    assert!(
        markdown.contains("# Hello"),
        "tagged heading must survive extract → as_content, got {markdown:?}"
    );
    unsafe { liteparse_extract_free(extracted.handle) };
}

#[test]
fn parse_reports_pdf_page_labels() {
    let parser = Parser::plain();
    let document = parser.open("page_labels.pdf");
    let result = document.parse(&[]);
    let expected = ["i", "ii", "1", "2"];
    assert_eq!(
        unsafe { liteparse_result_page_count(result.0) },
        expected.len()
    );
    for (index, label) in expected.into_iter().enumerate() {
        assert_eq!(
            view_str(unsafe { liteparse_result_page_label(result.0, index) }),
            label,
            "page {index}"
        );
        assert_eq!(
            unsafe { liteparse_result_page_number(result.0, index) },
            index as u32 + 1
        );
    }
}

#[test]
fn parse_content_projects_caller_text_without_opening_a_document() {
    let parser = Parser::plain();
    let items = [content_item(b"hello", 100.0)];
    let pages = [content_page(1, 0)];
    let mut content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = pages.len();
    content.items = items.as_ptr();
    content.items_len = items.len();
    let result = parser.parse_content(&content);
    assert_eq!(unsafe { liteparse_result_page_count(result.0) }, 1);
    assert!(view_str(unsafe { liteparse_result_page_text(result.0, 0) }).contains("hello"));
    assert!(view_str(unsafe { liteparse_result_text(result.0) }).contains("hello"));
    assert_eq!(
        view_str(unsafe { liteparse_result_page_label(result.0, 0) }),
        ""
    );
}

#[test]
fn parse_content_forwards_a_supplied_page_label() {
    let parser = Parser::plain();
    let items = [content_item(b"hello", 100.0)];
    let mut page = content_page(1, 0);
    page.page_label = view(b"iv");
    let pages = [page];
    let mut content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = pages.len();
    content.items = items.as_ptr();
    content.items_len = items.len();
    let result = parser.parse_content(&content);
    assert_eq!(
        view_str(unsafe { liteparse_result_page_label(result.0, 0) }),
        "iv"
    );
}

#[test]
fn parse_content_renders_supplied_blocks_instead_of_classifying() {
    let parser = Parser::new(|config| {
        config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
    });
    let items = [content_item(b"ignored-body", 120.0)];
    let mut page = content_page(1, 0);
    page.block_count = 1;
    let pages = [page];
    let blocks = [LiteParseLayoutBlock {
        kind: view(b"heading"),
        text: view(b"Title"),
        level: 1,
        has_level: true,
        ..LiteParseLayoutBlock::default()
    }];
    let mut content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    content.blocks = blocks.as_ptr();
    content.blocks_len = 1;
    let result = parser.parse_content(&content);
    assert_eq!(
        view_str(unsafe { liteparse_result_page_markdown(result.0, 0) }),
        "# Title"
    );
    assert_eq!(
        view_str(unsafe { liteparse_result_text(result.0) }),
        "# Title"
    );
    assert!(view_str(unsafe { liteparse_result_page_text(result.0, 0) }).contains("ignored-body"));

    let mut len = 0usize;
    let packed = slice(
        unsafe { liteparse_result_blocks(result.0, 0, &mut len) },
        len,
    );
    assert_eq!(len, 1);
    assert_eq!(view_str(packed[0].kind), "heading");
    assert_eq!(view_str(packed[0].text), "Title");
}

#[test]
fn parse_content_max_pages_truncates_and_drops_document_blocks() {
    let parser = Parser::new(|config| {
        config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        config.max_pages = 1;
    });
    let items = [content_item(b"one", 100.0), content_item(b"two", 100.0)];
    let pages = [
        LiteParseContentPage {
            block_offset: 0,
            block_count: 1,
            ..content_page(1, 0)
        },
        LiteParseContentPage {
            block_offset: 1,
            block_count: 1,
            ..content_page(2, 1)
        },
    ];
    let blocks = [
        LiteParseLayoutBlock {
            kind: view(b"heading"),
            text: view(b"one"),
            level: 1,
            has_level: true,
            ..LiteParseLayoutBlock::default()
        },
        LiteParseLayoutBlock {
            kind: view(b"heading"),
            text: view(b"two"),
            level: 1,
            has_level: true,
            ..LiteParseLayoutBlock::default()
        },
        LiteParseLayoutBlock {
            kind: view(b"paragraph"),
            text: view(b"ALL"),
            ..LiteParseLayoutBlock::default()
        },
    ];
    let mut content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = 2;
    content.items = items.as_ptr();
    content.items_len = 2;
    content.blocks = blocks.as_ptr();
    content.blocks_len = 3;
    content.document_block_offset = 2;
    content.document_block_count = 1;
    let result = parser.parse_content(&content);
    assert_eq!(unsafe { liteparse_result_page_count(result.0) }, 1);
    let text = view_str(unsafe { liteparse_result_text(result.0) });
    assert!(text.contains("# one"));
    assert!(!text.contains("ALL"));
    assert!(!text.contains("# two"));
}

#[test]
fn parse_content_rejects_bad_ranges_and_unknown_kinds() {
    let parser = Parser::plain();
    let items = [content_item(b"hello", 100.0)];
    let pages = [LiteParseContentPage {
        item_count: 8,
        ..content_page(1, 0)
    }];
    let mut content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    let parsed = unsafe { liteparse_parser_parse_content(parser.0, &content) };
    assert_eq!(parsed.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("items");

    let pages = [LiteParseContentPage {
        block_count: 1,
        ..content_page(1, 0)
    }];
    let blocks = [LiteParseLayoutBlock {
        kind: view(b"not-a-block"),
        ..LiteParseLayoutBlock::default()
    }];
    content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    content.blocks = blocks.as_ptr();
    content.blocks_len = 1;
    let parsed = unsafe { liteparse_parser_parse_content(parser.0, &content) };
    assert_eq!(parsed.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("unknown kind");

    let mut bad = liteparse_content_default();
    bad.size_of_content = 1;
    let parsed = unsafe { liteparse_parser_parse_content(parser.0, &bad) };
    assert_eq!(parsed.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("size_of_content");
}

#[test]
fn parse_content_round_trips_merged_table_spans() {
    let parser = Parser::new(|config| {
        config.bools_set |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
        config.bools_values |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
    });
    let items = [content_item(b"A", 100.0)];
    let pages = [LiteParseContentPage {
        block_count: 1,
        ..content_page(1, 0)
    }];
    let cells = [LiteParseLayoutCell {
        text: view(b"A"),
        colspan: 2,
        rowspan: 3,
        ..LiteParseLayoutCell::default()
    }];
    let rows = [LiteParseLayoutRow {
        cell_offset: 0,
        cell_count: 1,
    }];
    let blocks = [LiteParseLayoutBlock {
        kind: view(b"merged_table"),
        first_row: 0,
        row_count: 1,
        header_rows: 1,
        has_header_rows: true,
        ..LiteParseLayoutBlock::default()
    }];
    let mut content = liteparse_content_default();
    content.pages = pages.as_ptr();
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    content.blocks = blocks.as_ptr();
    content.blocks_len = 1;
    content.cells = cells.as_ptr();
    content.cells_len = 1;
    content.rows = rows.as_ptr();
    content.rows_len = 1;
    let result = parser.parse_content(&content);

    let mut len = 0usize;
    let packed = slice(
        unsafe { liteparse_result_blocks(result.0, 0, &mut len) },
        len,
    );
    assert_eq!(len, 1);
    assert_eq!(view_str(packed[0].kind), "merged_table");
    assert!(packed[0].has_header_rows);
    assert_eq!(packed[0].header_rows, 1);
    assert_eq!(packed[0].row_count, 1);

    let cells = slice(
        unsafe { liteparse_result_block_cells(result.0, 0, &mut len) },
        len,
    );
    assert_eq!(len, 1);
    assert_eq!(view_str(cells[0].text), "A");
    assert_eq!(cells[0].colspan, 2);
    assert_eq!(cells[0].rowspan, 3);
}

#[test]
fn page_objects_walk_path_form_and_image_without_extract_policy() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&objects_pdf());
    let snapshot = unsafe { liteparse_document_page_objects(document.0, ptr::null(), 0, 0) };
    assert_eq!(
        snapshot.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );
    assert_eq!(unsafe { liteparse_page_objects_page_count(snapshot.handle) }, 1);

    let mut page_len = 0usize;
    let pages = slice(
        unsafe { liteparse_page_objects_pages(snapshot.handle, &mut page_len) },
        page_len,
    );
    assert_eq!(page_len, 1);
    assert_eq!(pages[0].page_number, 1);
    assert!(pages[0].has_geometry);
    assert_eq!(pages[0].object_count, 3);

    let mut object_len = 0usize;
    let objects = slice(
        unsafe { liteparse_page_objects_objects(snapshot.handle, &mut object_len) },
        object_len,
    );
    assert_eq!(object_len, 4);
    let tops = &objects[pages[0].object_offset..pages[0].object_offset + pages[0].object_count];
    assert_eq!(
        tops.iter().map(|object| object.kind).collect::<Vec<_>>(),
        [
            LITEPARSE_PAGE_OBJECT_PATH,
            LITEPARSE_PAGE_OBJECT_FORM,
            LITEPARSE_PAGE_OBJECT_IMAGE
        ]
    );

    let path = &tops[0];
    assert!(path.has_draw_mode && path.path_filled && !path.path_stroked);
    assert!(path.has_matrix);
    assert_eq!(
        (path.matrix.a, path.matrix.d, path.matrix.e, path.matrix.f),
        (1.0, 1.0, 10.0, 20.0)
    );
    assert!(path.has_bounds);
    assert_eq!(
        (
            path.bounds.left,
            path.bounds.bottom,
            path.bounds.right,
            path.bounds.top
        ),
        (10.0, 20.0, 60.0, 50.0)
    );
    let mut segment_len = 0usize;
    let segments = slice(
        unsafe { liteparse_page_objects_segments(snapshot.handle, &mut segment_len) },
        segment_len,
    );
    let path_segments = &segments[path.segment_offset..path.segment_offset + path.segment_count];
    assert!(path.segment_count >= 4);
    assert!(path_segments[0].has_kind && path_segments[0].has_point);
    assert_eq!(path_segments[0].kind, LITEPARSE_PATH_SEGMENT_MOVETO);
    assert_eq!((path_segments[0].x, path_segments[0].y), (0.0, 0.0));
    assert!(path_segments
        .iter()
        .skip(1)
        .all(|segment| segment.has_kind && segment.kind == LITEPARSE_PATH_SEGMENT_LINETO));
    assert!(path_segments.iter().any(|segment| segment.close));
    assert!(path.image_raw.ptr.is_null() && path.image_raw.len == 0);

    let form = &tops[1];
    assert_eq!(form.kind, LITEPARSE_PAGE_OBJECT_FORM);
    assert_eq!(form.child_count, 1);
    let inner = &objects[form.child_offset];
    assert_eq!(inner.kind, LITEPARSE_PAGE_OBJECT_PATH);
    assert!(inner.has_draw_mode && !inner.path_filled && inner.path_stroked);
    assert!(inner.has_stroke_width);
    assert_eq!(inner.stroke_width, 1.0);

    let image = &tops[2];
    assert_eq!(image.kind, LITEPARSE_PAGE_OBJECT_IMAGE);
    assert!(image.has_image_metadata);
    assert_eq!(
        (image.image_width, image.image_height, image.image_bits_per_pixel),
        (2, 2, 8)
    );
    assert_eq!(image.filter_count, 0);
    assert!(image.image_raw.ptr.is_null());
    assert!(image.image_decoded.ptr.is_null());
    assert!(image.image_bitmap.ptr.is_null());

    let extracted = unsafe { liteparse_document_extract(document.0, ptr::null(), 0) };
    assert_eq!(extracted.status, LITEPARSE_STATUS_OK);
    let mut graphic_len = 0usize;
    let graphics = slice(
        unsafe { liteparse_extract_graphics(extracted.handle, &mut graphic_len) },
        graphic_len,
    );
    assert!(graphics
        .iter()
        .any(|graphic| graphic.kind == LITEPARSE_GRAPHIC_RECT));
    assert!(graphics.iter().all(|graphic| {
        graphic.kind == LITEPARSE_GRAPHIC_STROKE || graphic.kind == LITEPARSE_GRAPHIC_RECT
    }));
    unsafe { liteparse_extract_free(extracted.handle) };
    unsafe { liteparse_page_objects_free(snapshot.handle) };
}

#[test]
fn page_objects_image_payloads_follow_flags() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&objects_pdf());
    let flags = LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW
        | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED
        | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP;
    let snapshot = unsafe { liteparse_document_page_objects(document.0, ptr::null(), 0, flags) };
    assert_eq!(
        snapshot.status,
        LITEPARSE_STATUS_OK,
        "{}",
        view_str(liteparse_last_error())
    );

    let mut object_len = 0usize;
    let objects = slice(
        unsafe { liteparse_page_objects_objects(snapshot.handle, &mut object_len) },
        object_len,
    );
    let image = objects
        .iter()
        .find(|object| object.kind == LITEPARSE_PAGE_OBJECT_IMAGE)
        .expect("image object");
    let raw = slice(image.image_raw.ptr, image.image_raw.len);
    let decoded = slice(image.image_decoded.ptr, image.image_decoded.len);
    assert_eq!(raw, [0x00, 0xff, 0xff, 0x00]);
    assert_eq!(decoded, raw);
    assert_eq!(image.bitmap_format, LITEPARSE_BITMAP_FORMAT_GRAY);
    assert_eq!((image.bitmap_width, image.bitmap_height), (2, 2));
    assert!(image.bitmap_stride >= 2);
    let bitmap = slice(image.image_bitmap.ptr, image.image_bitmap.len);
    let stride = image.bitmap_stride as usize;
    assert_eq!(&bitmap[..2], &[0x00, 0xff]);
    assert_eq!(&bitmap[stride..stride + 2], &[0xff, 0x00]);

    let rejected = unsafe { liteparse_document_page_objects(document.0, ptr::null(), 0, 1 << 10) };
    assert_eq!(rejected.status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("unknown bits");
    unsafe { liteparse_page_objects_free(snapshot.handle) };
}
