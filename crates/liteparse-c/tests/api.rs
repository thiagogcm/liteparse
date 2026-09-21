use std::ffi::c_void;
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
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            contents.len(),
            unsafe { std::str::from_utf8_unchecked(contents) }
        )
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
    let content =
        b"q 1 0 0 1 10 20 cm 0 0 50 30 re f Q q /Fx1 Do Q q 40 0 0 20 100 50 cm /Im1 Do Q";
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
    String::from_utf8(arr(view.ptr, view.len).to_vec()).expect("utf-8")
}

fn arr<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() {
        assert_eq!(len, 0, "null array with non-zero length");
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// A byte view is null exactly when it is empty.
fn check_view(view: LiteParseByteView) {
    assert_eq!(
        view.ptr.is_null(),
        view.len == 0,
        "byte view null/len mismatch"
    );
}

fn range<T>(items: &[T], offset: u32, count: u32) -> &[T] {
    let start = offset as usize;
    let end = start + count as usize;
    assert!(
        end <= items.len(),
        "range {start}..{end} outside {}",
        items.len()
    );
    &items[start..end]
}

fn last_error() -> String {
    view_str(liteparse_last_error())
}

fn last_error_contains(fragment: &str) {
    let message = last_error();
    assert!(message.contains(fragment), "last error was: {message}");
}

fn config(tweak: impl FnOnce(&mut LiteParseConfig)) -> LiteParseConfig {
    let mut config = std::mem::MaybeUninit::<LiteParseConfig>::uninit();
    unsafe { liteparse_config_init(config.as_mut_ptr()) };
    let mut config = unsafe { config.assume_init() };
    config.bools_set |= LITEPARSE_FLAG_QUIET | LITEPARSE_FLAG_OCR_ENABLED;
    config.bools_values |= LITEPARSE_FLAG_QUIET;
    tweak(&mut config);
    config
}

/// Run a creation function with an out pointer and wrap the handle.
fn call<H, W>(
    create: impl FnOnce(*mut *mut H) -> LiteParseStatus,
    wrap: impl FnOnce(*mut H) -> W,
) -> Result<W, LiteParseStatus> {
    let mut handle = ptr::null_mut();
    match create(&mut handle) {
        LITEPARSE_STATUS_OK => Ok(wrap(handle)),
        status => {
            assert!(handle.is_null(), "failed creation must leave a null handle");
            Err(status)
        }
    }
}

#[derive(Debug)]
struct Parser(*mut LiteParseParser);

impl Parser {
    fn try_new(config: &LiteParseConfig) -> Result<Self, LiteParseStatus> {
        call(|out| unsafe { liteparse_parser_new(config, out) }, Self)
    }

    fn new(tweak: impl FnOnce(&mut LiteParseConfig)) -> Self {
        Self::try_new(&config(tweak)).unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }

    fn plain() -> Self {
        Self::new(|_| {})
    }

    fn open(&self, name: &str) -> Document {
        let path = fixture(name);
        call(
            |out| unsafe { liteparse_document_open_path(self.0, view(path.as_bytes()), out) },
            Document,
        )
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }

    fn open_bytes(&self, bytes: &[u8]) -> Document {
        call(
            |out| unsafe {
                liteparse_document_open_bytes(self.0, bytes.as_ptr(), bytes.len(), out)
            },
            Document,
        )
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }

    fn parse_content(&self, content: &LiteParseContent) -> Result<Res, LiteParseStatus> {
        call(
            |out| unsafe { liteparse_parser_parse_content(self.0, content, out) },
            Res,
        )
    }
}

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe { liteparse_parser_free(self.0) };
    }
}

#[derive(Debug)]
struct Document(*mut LiteParseDocument);

impl Document {
    fn info(&self) -> &LiteParseDocumentInfo {
        unsafe { liteparse_document_info(self.0).as_ref() }.expect("info")
    }

    fn try_parse(&self, pages: &[u32]) -> Result<Res, LiteParseStatus> {
        call(
            |out| unsafe { liteparse_document_parse(self.0, pages.as_ptr(), pages.len(), out) },
            Res,
        )
    }

    fn parse(&self, pages: &[u32]) -> Res {
        self.try_parse(pages)
            .unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }

    fn try_extract(&self, pages: &[u32]) -> Result<Res, LiteParseStatus> {
        call(
            |out| unsafe { liteparse_document_extract(self.0, pages.as_ptr(), pages.len(), out) },
            Res,
        )
    }

    fn extract(&self, pages: &[u32]) -> Res {
        self.try_extract(pages)
            .unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }

    fn screenshot(
        &self,
        pages: &[u32],
        dpi: f32,
        region: Option<LiteParseRenderRegion>,
    ) -> Result<Shots, LiteParseStatus> {
        call(
            |out| unsafe {
                liteparse_document_screenshot(
                    self.0,
                    pages.as_ptr(),
                    pages.len(),
                    dpi,
                    region.as_ref().map_or(ptr::null(), ptr::from_ref),
                    out,
                )
            },
            Shots,
        )
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        unsafe { liteparse_document_free(self.0) };
    }
}

#[derive(Debug)]
struct Res(*mut LiteParseResult);

impl Res {
    fn view(&self) -> &LiteParseResultView {
        unsafe { liteparse_result_view(self.0).as_ref() }.expect("view")
    }

    fn text(&self) -> String {
        view_str(self.view().text)
    }

    fn pages(&self) -> &[LiteParsePage] {
        let content = &self.view().content;
        arr(content.pages, content.pages_len)
    }
}

impl Drop for Res {
    fn drop(&mut self) {
        unsafe { liteparse_result_free(self.0) };
    }
}

#[derive(Debug)]
struct Shots(*mut LiteParseScreenshots);

impl Shots {
    fn view(&self) -> &LiteParseScreenshotsView {
        unsafe { liteparse_screenshots_view(self.0).as_ref() }.expect("view")
    }

    fn shots(&self) -> &[LiteParseScreenshot] {
        arr(self.view().screenshots, self.view().screenshots_len)
    }
}

impl Drop for Shots {
    fn drop(&mut self) {
        unsafe { liteparse_screenshots_free(self.0) };
    }
}

/// Every offset/count in a content view must land inside its array.
fn check_content_ranges(content: &LiteParseContent) {
    let pages = arr(content.pages, content.pages_len);
    let items = arr(content.items, content.items_len);
    let words = arr(content.words, content.words_len);
    let char_codes = arr(content.char_codes, content.char_codes_len);
    let graphics = arr(content.graphics, content.graphics_len);
    let struct_nodes = arr(content.struct_nodes, content.struct_nodes_len);
    let mcids = arr(content.mcids, content.mcids_len);
    let image_refs = arr(content.image_refs, content.image_refs_len);
    let annotations = arr(content.annotations, content.annotations_len);
    let quadpoints = arr(content.quadpoints, content.quadpoints_len);
    let form_fields = arr(content.form_fields, content.form_fields_len);
    let strings = arr(content.strings, content.strings_len);
    let structure_nodes = arr(content.structure_nodes, content.structure_nodes_len);
    let attributes = arr(
        content.structure_attributes,
        content.structure_attributes_len,
    );
    let blocks = arr(content.blocks, content.blocks_len);
    let cells = arr(content.cells, content.cells_len);
    let rows = arr(content.rows, content.rows_len);
    let shapes = arr(content.vector_shapes, content.vector_shapes_len);
    let lines = arr(content.vector_lines, content.vector_lines_len);
    let _ = arr(content.outline, content.outline_len);
    let _ = arr(content.page_errors, content.page_errors_len);
    let _ = arr(content.images, content.images_len);
    range(
        blocks,
        content.document_block_offset,
        content.document_block_count,
    );

    for item in items {
        check_view(item.text);
        check_view(item.font_name);
        check_view(item.link);
        range(words, item.word_offset, item.word_count);
        range(char_codes, item.char_code_offset, item.char_code_count);
    }
    for string in strings {
        check_view(*string);
    }
    for node in struct_nodes {
        range(mcids, node.mcid_offset, node.mcid_count);
    }
    for annotation in annotations {
        range(
            quadpoints,
            annotation.quadpoint_offset,
            annotation.quadpoint_count,
        );
    }
    for field in form_fields {
        range(strings, field.option_offset, field.option_count);
        range(
            strings,
            field.selected_option_offset,
            field.selected_option_count,
        );
    }
    for (index, node) in structure_nodes.iter().enumerate() {
        range(mcids, node.mcid_offset, node.mcid_count);
        range(attributes, node.attribute_offset, node.attribute_count);
        range(annotations, node.annotation_offset, node.annotation_count);
        assert!(node.parent_index == LITEPARSE_NO_PARENT || (node.parent_index as usize) < index);
    }
    for block in blocks {
        range(strings, block.line_offset, block.line_count);
        range(cells, block.header_cell_offset, block.header_cell_count);
        for row in range(rows, block.row_offset, block.row_count) {
            range(cells, row.cell_offset, row.cell_count);
        }
    }
    // Page ranges into the shared arrays never overlap structure-node or
    // block ranges into the same arrays.
    let overlaps =
        |a: (u32, u32), b: (u32, u32)| a.1 > 0 && b.1 > 0 && a.0 < b.0 + b.1 && b.0 < a.0 + a.1;
    for page in pages {
        let page_annotations = (page.annotation_offset, page.annotation_count);
        for node in range(
            structure_nodes,
            page.structure_node_offset,
            page.structure_node_count,
        ) {
            assert!(!overlaps(
                page_annotations,
                (node.annotation_offset, node.annotation_count)
            ));
            for struct_node in range(
                struct_nodes,
                page.struct_node_offset,
                page.struct_node_count,
            ) {
                assert!(!overlaps(
                    (struct_node.mcid_offset, struct_node.mcid_count),
                    (node.mcid_offset, node.mcid_count)
                ));
            }
        }
        for field in range(form_fields, page.form_field_offset, page.form_field_count) {
            for block in range(blocks, page.block_offset, page.block_count) {
                assert!(!overlaps(
                    (field.option_offset, field.option_count),
                    (block.line_offset, block.line_count)
                ));
            }
        }
    }
    for page in pages {
        check_view(page.label);
        check_view(page.text);
        check_view(page.markdown);
        range(items, page.item_offset, page.item_count);
        range(graphics, page.graphic_offset, page.graphic_count);
        range(
            struct_nodes,
            page.struct_node_offset,
            page.struct_node_count,
        );
        range(image_refs, page.image_ref_offset, page.image_ref_count);
        range(annotations, page.annotation_offset, page.annotation_count);
        range(form_fields, page.form_field_offset, page.form_field_count);
        range(
            structure_nodes,
            page.structure_node_offset,
            page.structure_node_count,
        );
        range(blocks, page.block_offset, page.block_count);
        range(shapes, page.vector_shape_offset, page.vector_shape_count);
        range(lines, page.vector_line_offset, page.vector_line_count);
    }
}

fn check_result_ranges(view: &LiteParseResultView) {
    check_content_ranges(&view.content);
    check_view(view.text);
    check_view(view.creator);
    check_view(view.producer);
    let content = &view.content;
    let pages = arr(content.pages, content.pages_len);
    let words = arr(content.words, content.words_len);
    let char_codes = arr(content.char_codes, content.char_codes_len);
    let figures = arr(view.figures, view.figures_len);
    let frames = arr(view.item_frames, view.item_frames_len);
    let lines = arr(view.projected_lines, view.projected_lines_len);
    let spans = arr(view.projected_spans, view.projected_spans_len);
    let paths = arr(view.region_paths, view.region_paths_len);
    let regions = arr(view.regions, view.regions_len);
    let region_children = arr(view.region_children, view.region_children_len);
    let shots = arr(view.screenshots, view.screenshots_len);
    let rects = arr(view.screenshot_rects, view.screenshot_rects_len);
    let _ = arr(view.xfa_packets, view.xfa_packets_len);
    let _ = arr(view.flattened_page_numbers, view.flattened_page_numbers_len);
    for page in pages {
        range(figures, page.figure_offset, page.figure_count);
        range(frames, page.item_frame_offset, page.item_frame_count);
        range(lines, page.projected_line_offset, page.projected_line_count);
        range(regions, page.region_offset, page.region_count);
    }
    for line in lines {
        range(spans, line.span_offset, line.span_count);
        range(paths, line.region_path_offset, line.region_path_count);
    }
    for span in spans {
        range(words, span.word_offset, span.word_count);
        range(char_codes, span.char_code_offset, span.char_code_count);
    }
    for (index, region) in regions.iter().enumerate() {
        for child in range(region_children, region.child_offset, region.child_count) {
            assert_eq!(regions[*child as usize].parent_index as usize, index);
        }
        assert!(
            region.parent_index == LITEPARSE_NO_PARENT || (region.parent_index as usize) < index
        );
    }
    for shot in shots {
        range(rects, shot.rect_offset, shot.rect_count);
    }
}

// ---------------------------------------------------------------------------

#[test]
fn version_and_last_error() {
    assert_eq!(view_str(liteparse_version()), env!("CARGO_PKG_VERSION"));
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_parser_new(ptr::null(), &mut handle) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert!(handle.is_null());
    last_error_contains("config must not be null");
    assert_eq!(
        unsafe { liteparse_parser_new(&config(|_| {}), ptr::null_mut()) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("out pointer");
}

#[test]
fn null_handles_degrade_to_null_views() {
    unsafe {
        assert!(liteparse_document_info(ptr::null()).is_null());
        assert!(liteparse_result_view(ptr::null()).is_null());
        assert!(liteparse_search_matches_view(ptr::null()).is_null());
        assert!(liteparse_screenshots_view(ptr::null()).is_null());
        assert!(liteparse_complexity_view(ptr::null()).is_null());
        assert!(liteparse_raw_text_view(ptr::null()).is_null());
        assert!(liteparse_page_objects_view(ptr::null()).is_null());
        let mut json = view(b"stale");
        assert_eq!(
            liteparse_result_to_json(ptr::null(), &mut json),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        assert!(json.ptr.is_null() && json.len == 0);
        let mut handle = ptr::null_mut();
        assert_eq!(
            liteparse_document_parse(ptr::null(), ptr::null(), 0, &mut handle),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        liteparse_parser_free(ptr::null_mut());
        liteparse_document_free(ptr::null_mut());
        liteparse_result_free(ptr::null_mut());
        liteparse_search_matches_free(ptr::null_mut());
        liteparse_screenshots_free(ptr::null_mut());
        liteparse_complexity_free(ptr::null_mut());
        liteparse_raw_text_free(ptr::null_mut());
        liteparse_page_objects_free(ptr::null_mut());
        liteparse_config_init(ptr::null_mut());
        liteparse_content_init(ptr::null_mut());
    }
}

#[test]
fn parser_new_validates_config() {
    let reject = |status: LiteParseStatus, fragment: &str, tweak: &dyn Fn(&mut LiteParseConfig)| {
        assert_eq!(Parser::try_new(&config(tweak)).unwrap_err(), status);
        last_error_contains(fragment);
    };
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "dpi", &|c| c.dpi = -1.0);
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "crop_box", &|c| {
        c.flags |= LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX;
        c.crop_box = [0.6, 0.0, 0.6, 0.0];
    });
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "unknown bits", &|c| {
        c.flags = 1 << 7
    });
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "image_output_dir", &|c| {
        c.image_output_dir = view(b"/tmp");
    });
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "output format", &|c| {
        c.output_format = 9
    });
    reject(LITEPARSE_STATUS_INVALID_ARGUMENT, "size_of_config", &|c| {
        c.size_of_config -= 1
    });
    reject(LITEPARSE_STATUS_INVALID_ARGUMENT, "UTF-8", &|c| {
        c.font_db_dir = view(&[0xff, 0xfe])
    });
    reject(LITEPARSE_STATUS_INVALID_ARGUMENT, "length zero", &|c| {
        c.font_db_dir = LiteParseByteView {
            ptr: ptr::null(),
            len: 3,
        };
    });
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "45", &|c| {
        static BAD: [LiteParsePageOrientationCorrection; 1] =
            [LiteParsePageOrientationCorrection { page: 1, angle: 45 }];
        c.orientation_corrections = BAD.as_ptr();
        c.orientation_corrections_len = 1;
    });
    // An identity crop box is accepted and only drops nothing.
    let cropped = Parser::new(|c| c.flags |= LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX);
    assert!(!cropped.open("sample.pdf").parse(&[]).text().is_empty());
}

#[test]
fn max_pages_zero_parses_no_pages() {
    let parser = Parser::new(|c| c.max_pages = 0);
    let document = parser.open("sample.pdf");
    let parsed = document.parse(&[]);
    assert_eq!(parsed.pages().len(), 0);
    assert_eq!(parsed.view().total_pages, document.info().total_pages);
    let one = Parser::new(|c| c.max_pages = 1);
    assert_eq!(one.open("page_labels.pdf").extract(&[]).pages().len(), 1);
}

#[test]
fn config_views_are_copied_at_parser_creation() {
    let dir = String::from("/definitely/missing/glyph-db");
    let parser = Parser::new(|c| c.font_db_dir = view(dir.as_bytes()));
    drop(dir);
    assert!(!parser.open("sample.pdf").parse(&[]).text().is_empty());
}

#[test]
fn page_selections_are_validated_everywhere() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let total = document.info().total_pages;
    assert!(total >= 1);
    let selection = [total, 1, total];

    let parsed = document.parse(&selection);
    let numbers: Vec<u32> = parsed.pages().iter().map(|p| p.page_number).collect();
    let expected: Vec<u32> = if total == 1 { vec![1] } else { vec![1, total] };
    assert_eq!(numbers, expected);
    let extracted = document.extract(&selection);
    assert_eq!(
        extracted
            .pages()
            .iter()
            .map(|p| p.page_number)
            .collect::<Vec<_>>(),
        expected
    );

    for bad in [[0u32], [total + 1]] {
        assert_eq!(
            document.try_parse(&bad).unwrap_err(),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        last_error_contains("out of range");
        assert_eq!(
            document.try_extract(&bad).unwrap_err(),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            document.screenshot(&bad, 0.0, None).unwrap_err(),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        unsafe {
            let mut complexity = ptr::null_mut();
            assert_eq!(
                liteparse_document_complexity(document.0, bad.as_ptr(), 1, &mut complexity),
                LITEPARSE_STATUS_INVALID_ARGUMENT
            );
            let mut raw = ptr::null_mut();
            assert_eq!(
                liteparse_document_raw_text(document.0, bad.as_ptr(), 1, &mut raw),
                LITEPARSE_STATUS_INVALID_ARGUMENT
            );
            let mut objects = ptr::null_mut();
            assert_eq!(
                liteparse_document_page_objects(document.0, bad.as_ptr(), 1, 0, &mut objects),
                LITEPARSE_STATUS_INVALID_ARGUMENT
            );
            assert!(complexity.is_null() && raw.is_null() && objects.is_null());
        }
    }
    // Null pages with a non-zero length is an argument error, never a crash.
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_parse(document.0, ptr::null(), 2, &mut handle) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
}

#[test]
fn continue_on_page_error_never_swallows_argument_errors() {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
        c.bools_values |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
    });
    let document = parser.open("sample.pdf");
    let outside = LiteParseRenderRegion {
        x: 0.0,
        y: 0.0,
        width: 99999.0,
        height: 1.0,
    };
    assert_eq!(
        document.screenshot(&[], 0.0, Some(outside)).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        document.try_parse(&[0]).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
}

#[test]
fn open_bytes_matches_open_path() {
    let parser = Parser::plain();
    let from_path = parser.open("sample.pdf").parse(&[]).text();
    let bytes = std::fs::read(fixture("sample.pdf")).expect("fixture");
    let document = parser.open_bytes(&bytes);
    drop(bytes);
    assert_eq!(document.parse(&[]).text(), from_path);
    assert_eq!(document.info().flags & LITEPARSE_DOCUMENT_FLAG_CONVERTED, 0);
}

#[test]
fn result_view_is_self_consistent() {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_EMIT_WORD_BOXES
            | LITEPARSE_FLAG_EXTRACT_TEXT_METADATA
            | LITEPARSE_FLAG_INCLUDE_COMPLEXITY
            | LITEPARSE_FLAG_EXTRACT_CONTENT_BOUNDS;
        c.bools_values |= LITEPARSE_FLAG_EMIT_WORD_BOXES
            | LITEPARSE_FLAG_EXTRACT_TEXT_METADATA
            | LITEPARSE_FLAG_INCLUDE_COMPLEXITY
            | LITEPARSE_FLAG_EXTRACT_CONTENT_BOUNDS;
    });
    let document = parser.open("page_labels.pdf");
    let parsed = document.parse(&[]);
    let view = parsed.view();
    check_result_ranges(view);
    assert_eq!(view.flags & LITEPARSE_RESULT_FLAG_EXTRACT_ONLY, 0);
    assert_ne!(view.flags & LITEPARSE_RESULT_FLAG_TEXT_METADATA, 0);
    assert_eq!(view.total_pages, 4);
    let pages = parsed.pages();
    assert_eq!(pages.len(), 4);
    let labels: Vec<String> = pages.iter().map(|p| view_str(p.label)).collect();
    assert_eq!(labels, ["i", "ii", "1", "2"]);
    for page in pages {
        assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_GEOMETRY, 0);
        assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY, 0);
        assert_eq!(page.complexity.page_number, page.page_number);
        assert_eq!(page.flags & LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS, 0);
        assert_eq!(page.flags & LITEPARSE_PAGE_FLAG_HAS_BLOCKS, 0);
        assert!(page.width > 0.0 && page.height > 0.0);
        assert!(!view_str(page.text).is_empty());
        assert!(view_str(page.markdown).is_empty());
    }
    let items = arr(view.content.items, view.content.items_len);
    assert!(!items.is_empty());
    assert!(
        items.iter().any(|item| item.word_count > 0),
        "word boxes requested"
    );
    assert!(
        items.iter().any(|item| item.char_code_count > 0),
        "char codes exported"
    );
    let words = arr(view.content.words, view.content.words_len);
    let first = items.iter().find(|item| item.word_count > 0).unwrap();
    let word = &range(words, first.word_offset, first.word_count)[0];
    assert!(!view_str(word.text).is_empty());
    assert!(view.projected_lines_len > 0 && view.regions_len > 0);

    // Text metadata off: the items still carry what the core holds.
    let plain = Parser::plain().open("page_labels.pdf").parse(&[1]);
    let plain_view = plain.view();
    assert_eq!(plain_view.flags & LITEPARSE_RESULT_FLAG_TEXT_METADATA, 0);
    check_result_ranges(plain_view);
    assert_eq!(plain_view.content.pages_len, 1);
}

#[test]
fn empty_optional_collections_are_null_with_flags() {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS
            | LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE
            | LITEPARSE_FLAG_EXTRACT_VECTOR_GRAPHICS
            | LITEPARSE_FLAG_EXTRACT_XFA_PACKETS;
        c.bools_values |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS
            | LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE
            | LITEPARSE_FLAG_EXTRACT_VECTOR_GRAPHICS
            | LITEPARSE_FLAG_EXTRACT_XFA_PACKETS;
    });
    let parsed = parser.open_bytes(&blank_pdf(200, 100)).parse(&[]);
    let view = parsed.view();
    check_result_ranges(view);
    let page = &parsed.pages()[0];
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS, 0);
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS, 0);
    assert_eq!(page.annotation_count, 0);
    assert!(view.content.annotations.is_null() && view.content.annotations_len == 0);
    assert!(view.content.outline.is_null() && view.content.outline_len == 0);
    assert_ne!(view.flags & LITEPARSE_RESULT_FLAG_HAS_XFA_PACKETS, 0);
    assert!(view.xfa_packets.is_null() && view.xfa_packets_len == 0);
    assert_eq!(view.flags & LITEPARSE_RESULT_FLAG_HAS_FORM_TYPE, 0);
    assert_eq!(view.flags & LITEPARSE_RESULT_FLAG_HAS_DOC_META, 0);
}

#[test]
fn json_is_cached_and_borrowed_from_the_handle() {
    let parser = Parser::plain();
    let parsed = parser.open("sample.pdf").parse(&[]);
    let mut first = LiteParseByteView::default();
    let mut second = LiteParseByteView::default();
    unsafe {
        assert_eq!(
            liteparse_result_to_json(parsed.0, &mut first),
            LITEPARSE_STATUS_OK
        );
        assert_eq!(
            liteparse_result_to_json(parsed.0, &mut second),
            LITEPARSE_STATUS_OK
        );
    }
    assert_eq!(first.ptr, second.ptr);
    assert!(view_str(first).contains("\"pages\""));
    let extracted = parser.open("sample.pdf").extract(&[]);
    assert_eq!(
        unsafe { liteparse_result_to_json(extracted.0, &mut first) },
        LITEPARSE_STATUS_SERIALIZATION_ERROR
    );
}

#[test]
fn search_matches_outlive_the_result() {
    let parser = Parser::plain();
    let parsed = parser.open("sample.pdf").parse(&[]);
    let items = arr(parsed.view().content.items, parsed.view().content.items_len);
    let phrase = view_str(items[0].text);
    let mut matches = ptr::null_mut();
    let status =
        unsafe { liteparse_result_search(parsed.0, 0, view(phrase.as_bytes()), 0, &mut matches) };
    assert_eq!(status, LITEPARSE_STATUS_OK, "{}", last_error());
    drop(parsed);
    let found = unsafe { liteparse_search_matches_view(matches).as_ref() }.unwrap();
    let hits = arr(found.items, found.items_len);
    assert!(!hits.is_empty());
    assert!(view_str(hits[0].text).contains(&phrase));
    for hit in hits {
        range(
            arr(found.words, found.words_len),
            hit.word_offset,
            hit.word_count,
        );
        range(
            arr(found.char_codes, found.char_codes_len),
            hit.char_code_offset,
            hit.char_code_count,
        );
    }
    unsafe { liteparse_search_matches_free(matches) };

    let parsed = parser.open("sample.pdf").parse(&[]);
    assert_eq!(
        unsafe { liteparse_result_search(parsed.0, 99, view(b"x"), 0, &mut matches) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        unsafe { liteparse_result_search(parsed.0, 0, view(b"x"), 1 << 5, &mut matches) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert!(matches.is_null());
}

#[test]
fn screenshots_render_whole_pages_and_cropped_regions() {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
        c.bools_values |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
    });
    let document = parser.open("sample.pdf");
    let full = document.screenshot(&[], 0.0, None).unwrap();
    assert_eq!(full.shots().len() as u32, document.info().total_pages);
    assert_eq!(
        &arr(full.shots()[0].png.ptr, full.shots()[0].png.len)[..4],
        b"\x89PNG"
    );
    let rects = arr(full.view().rects, full.view().rects_len);
    for shot in full.shots() {
        range(rects, shot.rect_offset, shot.rect_count);
    }

    let region = LiteParseRenderRegion {
        x: 10.3,
        y: 20.7,
        width: 100.2,
        height: 50.4,
    };
    let clipped = document.screenshot(&[], 144.0, Some(region)).unwrap();
    let expect = |pt: f32| (pt * 144.0 / 72.0).round() as u32;
    let shot = &clipped.shots()[0];
    assert_eq!(
        (shot.width, shot.height),
        (expect(region.width), expect(region.height))
    );
    let rects = arr(clipped.view().rects, clipped.view().rects_len);
    for rect in range(rects, shot.rect_offset, shot.rect_count) {
        assert!(rect.x >= 0.0 && rect.y >= 0.0);
        assert!(rect.x + rect.width <= region.width + 0.01);
        assert!(rect.y + rect.height <= region.height + 0.01);
    }
    assert_eq!(
        document.screenshot(&[], -3.0, None).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("dpi_override");
}

#[test]
fn effective_dpi_reports_the_long_edge_cap_on_both_render_paths() {
    let parser = Parser::new(|c| {
        c.dpi = 400.0;
        c.bools_set |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
        c.bools_values |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
    });
    let document = parser.open_bytes(&blank_pdf(7_200, 72));
    let rendered = document.screenshot(&[], 0.0, None).unwrap();
    assert_eq!(rendered.shots().len(), 1);
    assert_eq!(rendered.shots()[0].width, 30_000);
    assert_eq!(rendered.shots()[0].effective_dpi, 300.0);

    let parsed = document.parse(&[]);
    let shots = arr(parsed.view().screenshots, parsed.view().screenshots_len);
    assert_eq!(shots.len(), 1);
    assert_eq!(shots[0].width, 30_000);
    assert_eq!(shots[0].effective_dpi, 300.0);
    assert_ne!(shots[0].flags & LITEPARSE_SCREENSHOT_FLAG_SOLID_FILL, 0);
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
        let parser = Parser::new(|c| {
            c.bools_set |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
            if detect {
                c.bools_values |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
            }
        });
        let shots = parser
            .open("sample.pdf")
            .screenshot(&[1], 96.0, Some(region))
            .unwrap();
        arr(shots.shots()[0].png.ptr, shots.shots()[0].png.len).to_vec()
    };
    assert_eq!(png(true), png(false));
}

unsafe extern "C" fn recognize_words(
    calls: *mut c_void,
    image: *const LiteParseOcrImage,
    sink: *mut LiteParseOcrSink,
) -> u32 {
    let image = unsafe { &*image };
    assert_eq!(image.pixel_format, LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE);
    assert_eq!(
        image.pixels.len,
        image.width as usize * image.height as usize
    );
    assert_eq!(view_str(image.language), "eng");
    unsafe { *calls.cast::<u32>() += 1 };
    let words = [
        LiteParseOcrWord {
            text: view(b"zzocrzz"),
            x1: 0.0,
            y1: 0.0,
            x2: 40.0,
            y2: 10.0,
            confidence: 0.9,
            ..Default::default()
        },
        LiteParseOcrWord {
            text: view(b"zzrotate"),
            x1: 0.0,
            y1: 20.0,
            x2: 40.0,
            y2: 30.0,
            confidence: 0.8,
            polygon: [0.0, 20.0, 40.0, 20.0, 40.0, 30.0, 0.0, 30.0],
            flags: LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON,
        },
    ];
    let bad = LiteParseOcrWord {
        text: view(b"nope"),
        flags: 1 << 9,
        ..Default::default()
    };
    unsafe {
        assert_eq!(
            liteparse_ocr_sink_add(sink, &bad, 1),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            liteparse_ocr_sink_add(sink, words.as_ptr(), words.len()),
            LITEPARSE_STATUS_OK
        );
    }
    0
}

unsafe extern "C" fn recognize_failing(
    _user_data: *mut c_void,
    _image: *const LiteParseOcrImage,
    sink: *mut LiteParseOcrSink,
) -> u32 {
    unsafe { liteparse_ocr_sink_set_error(sink, view(b"engine exploded")) };
    7
}

fn ocr_parser(recognize: LiteParseOcrRecognizeFn, user_data: *mut c_void, flags: u32) -> Parser {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_OCR_FAILURE_FATAL;
        c.bools_values |= LITEPARSE_FLAG_OCR_ENABLED | LITEPARSE_FLAG_OCR_FAILURE_FATAL;
    });
    let status = unsafe {
        liteparse_parser_set_ocr_callback(parser.0, recognize, user_data, view(b"test-ocr"), flags)
    };
    assert_eq!(status, LITEPARSE_STATUS_OK);
    parser
}

#[test]
fn ocr_words_land_in_the_result_through_the_sink() {
    let mut calls = 0u32;
    let parser = ocr_parser(
        Some(recognize_words),
        ptr::from_mut(&mut calls).cast(),
        LITEPARSE_OCR_FLAG_PREFERS_GRAYSCALE,
    );
    let document = parser.open("receipt.png");
    assert_ne!(document.info().flags & LITEPARSE_DOCUMENT_FLAG_CONVERTED, 0);
    let text = document.parse(&[]).text();
    assert!(
        text.contains("zzocrzz") && text.contains("zzrotate"),
        "text was: {text}"
    );
    drop((document, parser));
    assert!(calls >= 1);
}

#[test]
fn ocr_callback_failure_surfaces_when_fatal() {
    let parser = ocr_parser(Some(recognize_failing), ptr::null_mut(), 0);
    let document = parser.open("receipt.png");
    assert_eq!(
        document.try_parse(&[]).unwrap_err(),
        LITEPARSE_STATUS_OCR_ERROR
    );
    last_error_contains("engine exploded");
    assert_eq!(
        unsafe {
            liteparse_parser_set_ocr_callback(parser.0, None, ptr::null_mut(), view(b""), 1 << 3)
        },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
}

#[test]
fn clearing_the_ocr_callback_restores_the_default_engine_choice() {
    let parser = ocr_parser(Some(recognize_failing), ptr::null_mut(), 0);
    let status =
        unsafe { liteparse_parser_set_ocr_callback(parser.0, None, ptr::null_mut(), view(b""), 0) };
    assert_eq!(status, LITEPARSE_STATUS_OK);
    // Built-in OCR availability varies, so only the removed callback is checked.
    if parser.open("sample.pdf").try_parse(&[]).is_err() {
        assert!(!last_error().contains("engine exploded"));
    }
}

#[test]
fn raw_text_keeps_every_glyph_and_forwards_page_labels() {
    let parser = Parser::plain();
    let document = parser.open("page_labels.pdf");
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_raw_text(document.0, ptr::null(), 0, &mut handle) },
        LITEPARSE_STATUS_OK
    );
    let view = unsafe { liteparse_raw_text_view(handle).as_ref() }.unwrap();
    let pages = arr(view.pages, view.pages_len);
    let items = arr(view.items, view.items_len);
    let char_codes = arr(view.char_codes, view.char_codes_len);
    let glyph_names = arr(view.glyph_names, view.glyph_names_len);
    assert_eq!(pages.len(), 4);
    assert_eq!(
        pages.iter().map(|p| view_str(p.label)).collect::<Vec<_>>(),
        ["i", "ii", "1", "2"]
    );
    for page in pages {
        assert_ne!(page.flags & LITEPARSE_RAW_PAGE_FLAG_HAS_GEOMETRY, 0);
        let page_items = range(items, page.item_offset, page.item_count);
        assert!(!page_items.is_empty());
        for item in page_items {
            let codes = range(char_codes, item.char_code_offset, item.char_code_count);
            assert!(!codes.is_empty());
            let names = range(glyph_names, item.glyph_name_offset, item.glyph_name_count);
            if item.flags & LITEPARSE_RAW_ITEM_FLAG_HAS_GLYPH_NAMES != 0 {
                assert_eq!(names.len(), codes.len());
            } else {
                assert!(names.is_empty());
            }
        }
    }
    unsafe { liteparse_raw_text_free(handle) };

    let selected = [3u32];
    assert_eq!(
        unsafe { liteparse_document_raw_text(document.0, selected.as_ptr(), 1, &mut handle) },
        LITEPARSE_STATUS_OK
    );
    let view = unsafe { liteparse_raw_text_view(handle).as_ref() }.unwrap();
    let pages = arr(view.pages, view.pages_len);
    assert_eq!((pages.len(), view_str(pages[0].label).as_str()), (1, "1"));
    unsafe { liteparse_raw_text_free(handle) };
}

#[test]
fn complexity_view_and_json_agree() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_complexity(document.0, ptr::null(), 0, &mut handle) },
        LITEPARSE_STATUS_OK
    );
    let view = unsafe { liteparse_complexity_view(handle).as_ref() }.unwrap();
    let pages = arr(view.pages, view.pages_len);
    assert_eq!(pages.len() as u32, document.info().total_pages);
    assert_eq!(pages[0].page_number, 1);
    let mut json = LiteParseByteView::default();
    assert_eq!(
        unsafe { liteparse_complexity_to_json(handle, &mut json) },
        LITEPARSE_STATUS_OK
    );
    assert!(view_str(json).contains("\"page_number\""));
    unsafe { liteparse_complexity_free(handle) };
}

#[test]
fn extract_round_trips_into_parse_content() {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
        c.bools_values |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
    });
    let document = parser.open("page_labels.pdf");
    let extracted = document.extract(&[]);
    let view = extracted.view();
    check_result_ranges(view);
    assert_ne!(view.flags & LITEPARSE_RESULT_FLAG_EXTRACT_ONLY, 0);
    assert!(view.text.ptr.is_null());
    assert_eq!(view.projected_lines_len, 0);
    let pages = extracted.pages();
    assert_eq!(pages.len(), 4);
    assert_eq!(view_str(pages[1].label), "ii");
    assert!(view_str(pages[1].text).is_empty());
    let items = arr(view.content.items, view.content.items_len);
    assert!(items.iter().any(|item| item.word_count > 0));
    assert_eq!(
        pages.iter().map(|p| p.item_count as usize).sum::<usize>(),
        items.len()
    );

    let parsed = document.parse(&[]);
    for (extracted_page, parsed_page) in pages.iter().zip(parsed.pages()) {
        assert_eq!(
            extracted_page.flags & LITEPARSE_PAGE_FLAG_HAS_GEOMETRY,
            LITEPARSE_PAGE_FLAG_HAS_GEOMETRY
        );
        assert_eq!(
            extracted_page.geometry.rotation_quarter_turns,
            parsed_page.geometry.rotation_quarter_turns
        );
        assert_eq!(
            extracted_page.geometry.box_right,
            parsed_page.geometry.box_right
        );
    }
    let reprojected = parser.parse_content(&view.content).unwrap();
    assert_eq!(reprojected.text(), parsed.text());
    assert_eq!(
        reprojected
            .pages()
            .iter()
            .map(|p| view_str(p.label))
            .collect::<Vec<_>>(),
        ["i", "ii", "1", "2"]
    );
}

fn content_item(text: &[u8], y: f32) -> LiteParseTextItem {
    LiteParseTextItem {
        text: view(text),
        x: 72.0,
        y,
        width: 80.0,
        height: 12.0,
        ..Default::default()
    }
}

fn content_page(number: u32, item_offset: u32) -> LiteParsePage {
    LiteParsePage {
        page_number: number,
        width: 612.0,
        height: 792.0,
        item_offset,
        item_count: 1,
        ..Default::default()
    }
}

fn empty_content() -> LiteParseContent {
    let mut content = std::mem::MaybeUninit::<LiteParseContent>::uninit();
    unsafe { liteparse_content_init(content.as_mut_ptr()) };
    unsafe { content.assume_init() }
}

#[test]
fn parse_content_projects_caller_text_without_opening_a_document() {
    let parser = Parser::plain();
    let items = [content_item(b"hello content", 100.0)];
    let mut page = content_page(1, 0);
    page.label = view(b"A-1");
    let mut content = empty_content();
    content.pages = &page;
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    let parsed = parser.parse_content(&content).unwrap();
    check_result_ranges(parsed.view());
    assert!(parsed.text().contains("hello content"));
    assert_eq!(view_str(parsed.pages()[0].label), "A-1");
}

#[test]
fn parse_content_rejects_bad_ranges_kinds_and_sizes() {
    let parser = Parser::plain();
    let items = [content_item(b"x", 100.0)];
    let block = LiteParseLayoutBlock {
        kind: 99,
        ..Default::default()
    };
    let attempt = |page: LiteParsePage, shrink: usize| {
        let mut content = empty_content();
        content.size_of_content -= shrink;
        content.pages = &page;
        content.pages_len = 1;
        content.items = items.as_ptr();
        content.items_len = 1;
        content.blocks = &block;
        content.blocks_len = 1;
        parser.parse_content(&content).unwrap_err()
    };

    assert_eq!(
        attempt(content_page(1, 5), 0),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("pages[0].items");

    let mut page = content_page(1, 0);
    page.block_count = 1;
    assert_eq!(attempt(page, 0), LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("unknown kind");

    assert_eq!(
        attempt(content_page(1, 0), 8),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("size_of_content");

    let mut page = content_page(1, 0);
    page.flags = 1 << 20;
    assert_eq!(attempt(page, 0), LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("unknown bits");

    let mut page = content_page(1, 0);
    page.flags = LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY;
    page.complexity.text_coverage = f32::NAN;
    assert_eq!(attempt(page, 0), LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("complexity must be finite");

    let reject_item = |item: LiteParseTextItem, fragment: &str| {
        let items = [item];
        let page = content_page(1, 0);
        let mut content = empty_content();
        content.pages = &page;
        content.pages_len = 1;
        content.items = items.as_ptr();
        content.items_len = 1;
        assert_eq!(
            parser.parse_content(&content).unwrap_err(),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        last_error_contains(fragment);
    };
    let mut nan = content_item(b"x", 100.0);
    nan.flags = LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE;
    nan.font_size = f32::NAN;
    reject_item(nan, "font metrics");
    let mut unknown = content_item(b"x", 100.0);
    unknown.flags = 1 << 30;
    reject_item(unknown, "unknown bits");

    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_parser_parse_content(parser.0, ptr::null(), &mut handle) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert!(handle.is_null());
}

#[test]
fn parse_content_accepts_and_packs_back_extras() {
    let parser = Parser::new(|c| {
        c.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        c.bools_set |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
        c.bools_values |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
    });
    let items = [content_item(b"Body text", 300.0)];
    let quadpoints = [LiteParseRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    }];
    let annotations = [
        LiteParseAnnotation {
            subtype: view(b"link"),
            uri: view(b"https://example.invalid"),
            rect: quadpoints[0],
            quadpoint_offset: 0,
            quadpoint_count: 1,
            flags: LITEPARSE_ANNOTATION_FLAG_HAS_RECT,
            ..Default::default()
        },
        LiteParseAnnotation {
            subtype: view(b"highlight"),
            ..Default::default()
        },
    ];
    let strings = [view(b"opt-a"), view(b"opt-b"), view(b"opt-b")];
    let form_fields = [LiteParseFormField {
        id: view(b"f1"),
        field_type: view(b"combobox"),
        page: 1,
        option_offset: 0,
        option_count: 2,
        selected_option_offset: 2,
        selected_option_count: 1,
        flags: LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED | LITEPARSE_FORM_FIELD_FLAG_CHECKED,
        ..Default::default()
    }];
    let attributes = [LiteParseStructureAttribute {
        name: view(b"O"),
        string: view(b"Layout"),
        kind: LITEPARSE_STRUCTURE_ATTR_STRING,
        number: 0.0,
    }];
    let mcids = [3i32, 4];
    // Document > [Sect > [P], P]
    let structure = [
        LiteParseStructureNode {
            element_type: view(b"Document"),
            parent_index: LITEPARSE_NO_PARENT,
            ..Default::default()
        },
        LiteParseStructureNode {
            element_type: view(b"Sect"),
            parent_index: 0,
            depth: 1,
            attribute_count: 1,
            annotation_offset: 1,
            annotation_count: 1,
            ..Default::default()
        },
        LiteParseStructureNode {
            element_type: view(b"P"),
            parent_index: 1,
            depth: 2,
            mcid_count: 2,
            ..Default::default()
        },
        LiteParseStructureNode {
            element_type: view(b"P"),
            parent_index: 0,
            depth: 1,
            ..Default::default()
        },
    ];
    let shapes = [LiteParseVectorShape {
        bbox: quadpoints[0],
        stroke_color: 0xff112233,
        flags: LITEPARSE_VECTOR_FLAG_STROKE | LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR,
        ..Default::default()
    }];
    let mut page = content_page(1, 0);
    page.flags = LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS
        | LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS
        | LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE
        | LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS
        | LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS;
    page.content_bounds = quadpoints[0];
    page.annotation_count = 1;
    page.form_field_count = 1;
    page.structure_node_count = 4;
    page.vector_shape_count = 1;
    let mut content = empty_content();
    content.pages = &page;
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    content.annotations = annotations.as_ptr();
    content.annotations_len = 2;
    content.quadpoints = quadpoints.as_ptr();
    content.quadpoints_len = 1;
    content.form_fields = form_fields.as_ptr();
    content.form_fields_len = 1;
    content.strings = strings.as_ptr();
    content.strings_len = 3;
    content.structure_nodes = structure.as_ptr();
    content.structure_nodes_len = 4;
    content.structure_attributes = attributes.as_ptr();
    content.structure_attributes_len = 1;
    content.mcids = mcids.as_ptr();
    content.mcids_len = 2;
    content.vector_shapes = shapes.as_ptr();
    content.vector_shapes_len = 1;

    let parsed = parser.parse_content(&content).unwrap();
    let view = parsed.view();
    check_result_ranges(view);
    let page = &parsed.pages()[0];
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS, 0);
    assert_eq!(page.content_bounds, quadpoints[0]);
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_BLOCKS, 0);
    assert!(page.block_count > 0, "classifier ran on the projected text");

    let out_annotations = arr(view.content.annotations, view.content.annotations_len);
    let page_annotations = range(
        out_annotations,
        page.annotation_offset,
        page.annotation_count,
    );
    assert_eq!(page_annotations.len(), 1);
    assert_eq!(view_str(page_annotations[0].uri), "https://example.invalid");
    assert_eq!(page_annotations[0].quadpoint_count, 1);
    let quads = arr(view.content.quadpoints, view.content.quadpoints_len);
    assert_eq!(
        range(quads, page_annotations[0].quadpoint_offset, 1)[0],
        quadpoints[0]
    );

    let fields = range(
        arr(view.content.form_fields, view.content.form_fields_len),
        page.form_field_offset,
        page.form_field_count,
    );
    let out_strings = arr(view.content.strings, view.content.strings_len);
    assert_eq!(fields.len(), 1);
    assert_ne!(fields[0].flags & LITEPARSE_FORM_FIELD_FLAG_CHECKED, 0);
    assert_eq!(
        range(out_strings, fields[0].option_offset, fields[0].option_count)
            .iter()
            .map(|s| view_str(*s))
            .collect::<Vec<_>>(),
        ["opt-a", "opt-b"]
    );
    assert_eq!(
        view_str(range(out_strings, fields[0].selected_option_offset, 1)[0]),
        "opt-b"
    );

    let nodes = range(
        arr(
            view.content.structure_nodes,
            view.content.structure_nodes_len,
        ),
        page.structure_node_offset,
        page.structure_node_count,
    );
    assert_eq!(nodes.len(), 4);
    let types: Vec<String> = nodes.iter().map(|n| view_str(n.element_type)).collect();
    assert_eq!(types, ["Document", "Sect", "P", "P"]);
    let parents: Vec<u32> = nodes.iter().map(|n| n.parent_index).collect();
    let base = page.structure_node_offset;
    assert_eq!(parents, [LITEPARSE_NO_PARENT, base, base + 1, base]);
    assert_eq!(nodes[1].attribute_count, 1);
    let attribute = range(
        arr(
            view.content.structure_attributes,
            view.content.structure_attributes_len,
        ),
        nodes[1].attribute_offset,
        1,
    )[0];
    assert_eq!(view_str(attribute.string), "Layout");
    assert_eq!(nodes[1].annotation_count, 1);
    assert_eq!(
        view_str(range(out_annotations, nodes[1].annotation_offset, 1)[0].subtype),
        "highlight"
    );
    assert_eq!(
        range(
            arr(view.content.mcids, view.content.mcids_len),
            nodes[2].mcid_offset,
            nodes[2].mcid_count
        ),
        [3, 4]
    );

    let out_shapes = range(
        arr(view.content.vector_shapes, view.content.vector_shapes_len),
        page.vector_shape_offset,
        page.vector_shape_count,
    );
    assert_eq!(out_shapes[0].stroke_color, 0xff112233);
    assert_ne!(
        out_shapes[0].flags & LITEPARSE_VECTOR_FLAG_HAS_STROKE_COLOR,
        0
    );
    assert_eq!(out_shapes[0].flags & LITEPARSE_VECTOR_FLAG_FILL, 0);
}

#[test]
fn parse_content_round_trips_supplied_blocks_and_merged_tables() {
    let parser = Parser::new(|c| {
        c.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        c.bools_set |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
        c.bools_values |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
    });
    let items = [content_item(b"Title", 100.0)];
    let cells = [
        LiteParseLayoutCell {
            text: view(b"wide"),
            colspan: 2,
            rowspan: 3,
            ..Default::default()
        },
        LiteParseLayoutCell {
            text: view(b"b"),
            ..Default::default()
        },
    ];
    let rows = [LiteParseLayoutRow {
        cell_offset: 0,
        cell_count: 2,
    }];
    let blocks = [
        LiteParseLayoutBlock {
            kind: LITEPARSE_BLOCK_HEADING,
            text: view(b"Title"),
            level: 2,
            flags: LITEPARSE_BLOCK_FLAG_HAS_LEVEL,
            ..Default::default()
        },
        LiteParseLayoutBlock {
            kind: LITEPARSE_BLOCK_MERGED_TABLE,
            row_offset: 0,
            row_count: 1,
            header_rows: 1,
            flags: LITEPARSE_BLOCK_FLAG_HAS_HEADER_ROWS,
            ..Default::default()
        },
    ];
    let mut page = content_page(1, 0);
    page.block_count = 2;
    let mut content = empty_content();
    content.pages = &page;
    content.pages_len = 1;
    content.items = items.as_ptr();
    content.items_len = 1;
    content.blocks = blocks.as_ptr();
    content.blocks_len = 2;
    content.cells = cells.as_ptr();
    content.cells_len = 2;
    content.rows = rows.as_ptr();
    content.rows_len = 1;

    let parsed = parser.parse_content(&content).unwrap();
    let view = parsed.view();
    check_result_ranges(view);
    let page = &parsed.pages()[0];
    let out_blocks = range(
        arr(view.content.blocks, view.content.blocks_len),
        page.block_offset,
        page.block_count,
    );
    assert_eq!(out_blocks.len(), 2);
    assert_eq!(out_blocks[0].kind, LITEPARSE_BLOCK_HEADING);
    assert_eq!(out_blocks[0].level, 2);
    assert_eq!(view_str(out_blocks[0].text), "Title");
    let table = &out_blocks[1];
    assert_eq!(table.kind, LITEPARSE_BLOCK_MERGED_TABLE);
    assert_eq!((table.header_rows, table.row_count), (1, 1));
    let out_rows = range(
        arr(view.content.rows, view.content.rows_len),
        table.row_offset,
        table.row_count,
    );
    let out_cells = range(
        arr(view.content.cells, view.content.cells_len),
        out_rows[0].cell_offset,
        out_rows[0].cell_count,
    );
    assert_eq!((out_cells[0].colspan, out_cells[0].rowspan), (2, 3));
    assert_eq!(view_str(out_cells[0].text), "wide");
    let markdown = view_str(page.markdown);
    assert!(markdown.contains("## Title"), "markdown was: {markdown}");

    // max_pages truncates the page list and drops document-level blocks.
    let one = Parser::new(|c| c.max_pages = 1);
    let items = [content_item(b"one", 100.0), content_item(b"two", 100.0)];
    let pages = [content_page(1, 0), content_page(2, 1)];
    let mut content = empty_content();
    content.pages = pages.as_ptr();
    content.pages_len = 2;
    content.items = items.as_ptr();
    content.items_len = 2;
    content.blocks = blocks.as_ptr();
    content.blocks_len = 2;
    content.cells = cells.as_ptr();
    content.cells_len = 2;
    content.rows = rows.as_ptr();
    content.rows_len = 1;
    content.document_block_offset = 0;
    content.document_block_count = 1;
    let truncated = one.parse_content(&content).unwrap();
    assert_eq!(truncated.pages().len(), 1);
    assert_eq!(truncated.view().content.document_block_count, 0);
}

#[test]
fn forms_and_metadata_pack_on_parse_and_extract() {
    let parser = Parser::new(|c| {
        c.bools_set |=
            LITEPARSE_FLAG_EXTRACT_FORM_FIELDS | LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA;
        c.bools_values |=
            LITEPARSE_FLAG_EXTRACT_FORM_FIELDS | LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA;
    });
    let document = parser.open("filled_acroform.pdf");
    for result in [document.parse(&[]), document.extract(&[])] {
        let view = result.view();
        check_result_ranges(view);
        assert_ne!(view.flags & LITEPARSE_RESULT_FLAG_HAS_FORM_TYPE, 0);
        assert_eq!(view.form_type, LITEPARSE_FORM_TYPE_ACRO_FORM);
        let page = &result.pages()[0];
        assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS, 0);
        let fields = range(
            arr(view.content.form_fields, view.content.form_fields_len),
            page.form_field_offset,
            page.form_field_count,
        );
        assert!(!fields.is_empty());
        assert!(fields.iter().all(|f| f.page == 1));
    }
    let parsed = document.parse(&[]);
    assert_ne!(parsed.view().flags & LITEPARSE_RESULT_FLAG_HAS_DOC_META, 0);
    assert_ne!(
        parsed.view().doc_meta.flags & LITEPARSE_DOC_META_FLAG_HAS_RAW_FILE_SIZE,
        0
    );
    assert!(parsed.view().doc_meta.raw_file_size > 0);
}

#[test]
fn annotations_forward_twin_links_on_parse_and_extract() {
    let parser = Parser::new(|c| {
        c.bools_set |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
        c.bools_values |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
    });
    let document = parser.open_bytes(&twin_links_pdf());
    for result in [document.parse(&[]), document.extract(&[])] {
        let view = result.view();
        let page = &result.pages()[0];
        let annotations = range(
            arr(view.content.annotations, view.content.annotations_len),
            page.annotation_offset,
            page.annotation_count,
        );
        assert_eq!(annotations.len(), 2);
        for annotation in annotations {
            assert_eq!(view_str(annotation.subtype), "link");
            assert_eq!(view_str(annotation.uri), "https://example.invalid/twin");
            assert_ne!(annotation.flags & LITEPARSE_ANNOTATION_FLAG_HAS_RECT, 0);
        }
    }
}

#[test]
fn structure_tree_and_mcids_pack_with_absolute_parents() {
    let parser = Parser::new(|c| {
        c.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        c.bools_set |= LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE;
        c.bools_values |= LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE;
    });
    let document = parser.open_bytes(&tagged_heading_pdf());
    let extracted = document.extract(&[]);
    let view = extracted.view();
    check_result_ranges(view);
    let page = &extracted.pages()[0];
    let struct_nodes = range(
        arr(view.content.struct_nodes, view.content.struct_nodes_len),
        page.struct_node_offset,
        page.struct_node_count,
    );
    assert!(struct_nodes.iter().any(|n| view_str(n.role) == "H1"));
    let mcids = arr(view.content.mcids, view.content.mcids_len);
    let h1 = struct_nodes
        .iter()
        .find(|n| view_str(n.role) == "H1")
        .unwrap();
    assert_eq!(range(mcids, h1.mcid_offset, h1.mcid_count), [0]);
    let items = range(
        arr(view.content.items, view.content.items_len),
        page.item_offset,
        page.item_count,
    );
    assert!(
        items
            .iter()
            .any(|i| i.flags & LITEPARSE_TEXT_ITEM_FLAG_HAS_MCID != 0 && i.mcid == 0)
    );
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE, 0);
    let nodes = range(
        arr(
            view.content.structure_nodes,
            view.content.structure_nodes_len,
        ),
        page.structure_node_offset,
        page.structure_node_count,
    );
    assert!(nodes.iter().any(|n| view_str(n.element_type) == "H1"));

    // Struct nodes re-enter parse_content for heading classification.
    let reparsed = parser.parse_content(&view.content).unwrap();
    assert!(
        reparsed.text().contains("# Hello"),
        "text was: {}",
        reparsed.text()
    );
}

#[test]
fn page_objects_walk_path_form_and_image_without_extract_policy() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&objects_pdf());
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_page_objects(document.0, ptr::null(), 0, 0, &mut handle) },
        LITEPARSE_STATUS_OK,
        "{}",
        last_error()
    );
    let view = unsafe { liteparse_page_objects_view(handle).as_ref() }.unwrap();
    let pages = arr(view.pages, view.pages_len);
    let objects = arr(view.objects, view.objects_len);
    let segments = arr(view.segments, view.segments_len);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].page_number, 1);
    assert_ne!(pages[0].flags & LITEPARSE_OBJECT_PAGE_FLAG_HAS_GEOMETRY, 0);
    assert_eq!(objects.len(), 4);
    let tops = range(objects, pages[0].object_offset, pages[0].object_count);
    assert_eq!(
        tops.iter().map(|o| o.kind).collect::<Vec<_>>(),
        [
            LITEPARSE_PAGE_OBJECT_PATH,
            LITEPARSE_PAGE_OBJECT_FORM,
            LITEPARSE_PAGE_OBJECT_IMAGE
        ]
    );
    let path = &tops[0];
    let flags = path.flags;
    assert_ne!(flags & LITEPARSE_PAGE_OBJECT_FLAG_HAS_DRAW_MODE, 0);
    assert_ne!(flags & LITEPARSE_PAGE_OBJECT_FLAG_PATH_FILLED, 0);
    assert_eq!(flags & LITEPARSE_PAGE_OBJECT_FLAG_PATH_STROKED, 0);
    assert_ne!(flags & LITEPARSE_PAGE_OBJECT_FLAG_HAS_MATRIX, 0);
    assert_eq!(
        (path.matrix.a, path.matrix.d, path.matrix.e, path.matrix.f),
        (1.0, 1.0, 10.0, 20.0)
    );
    assert_ne!(flags & LITEPARSE_PAGE_OBJECT_FLAG_HAS_BOUNDS, 0);
    assert_eq!(
        (
            path.bounds.left,
            path.bounds.bottom,
            path.bounds.right,
            path.bounds.top
        ),
        (10.0, 20.0, 60.0, 50.0)
    );
    let path_segments = range(segments, path.segment_offset, path.segment_count);
    assert!(path_segments.len() >= 4);
    assert_eq!(path_segments[0].kind, LITEPARSE_PATH_SEGMENT_MOVETO);
    assert_ne!(
        path_segments[0].flags & LITEPARSE_PATH_SEGMENT_FLAG_HAS_POINT,
        0
    );
    assert!(
        path_segments
            .iter()
            .skip(1)
            .all(|s| s.kind == LITEPARSE_PATH_SEGMENT_LINETO)
    );
    assert!(
        path_segments
            .iter()
            .any(|s| s.flags & LITEPARSE_PATH_SEGMENT_FLAG_CLOSE != 0)
    );

    let form = &tops[1];
    assert_eq!(form.child_count, 1);
    let inner = &range(objects, form.child_offset, form.child_count)[0];
    assert_eq!(inner.kind, LITEPARSE_PAGE_OBJECT_PATH);
    assert_ne!(inner.flags & LITEPARSE_PAGE_OBJECT_FLAG_PATH_STROKED, 0);
    assert_ne!(inner.flags & LITEPARSE_PAGE_OBJECT_FLAG_HAS_STROKE_WIDTH, 0);
    assert_eq!(inner.stroke_width, 1.0);

    let image = &tops[2];
    assert_ne!(
        image.flags & LITEPARSE_PAGE_OBJECT_FLAG_HAS_IMAGE_METADATA,
        0
    );
    assert_eq!(
        (
            image.image_width,
            image.image_height,
            image.image_bits_per_pixel
        ),
        (2, 2, 8)
    );
    assert_eq!(image.filter_count, 0);
    assert!(
        image.image_raw.ptr.is_null()
            && image.image_decoded.ptr.is_null()
            && image.image_bitmap.ptr.is_null()
    );
    unsafe { liteparse_page_objects_free(handle) };
}

#[test]
fn page_objects_image_payloads_follow_flags() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&objects_pdf());
    let flags = LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW
        | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED
        | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP;
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_page_objects(document.0, ptr::null(), 0, flags, &mut handle) },
        LITEPARSE_STATUS_OK
    );
    let view = unsafe { liteparse_page_objects_view(handle).as_ref() }.unwrap();
    let image = arr(view.objects, view.objects_len)
        .iter()
        .find(|o| o.kind == LITEPARSE_PAGE_OBJECT_IMAGE)
        .expect("image object");
    let raw = arr(image.image_raw.ptr, image.image_raw.len);
    assert_eq!(raw, [0x00, 0xff, 0xff, 0x00]);
    assert_eq!(arr(image.image_decoded.ptr, image.image_decoded.len), raw);
    assert_eq!(image.bitmap_format, LITEPARSE_BITMAP_FORMAT_GRAY);
    assert_eq!((image.bitmap_width, image.bitmap_height), (2, 2));
    let bitmap = arr(image.image_bitmap.ptr, image.image_bitmap.len);
    let stride = image.bitmap_stride as usize;
    assert_eq!(&bitmap[..2], &[0x00, 0xff]);
    assert_eq!(&bitmap[stride..stride + 2], &[0xff, 0x00]);
    unsafe { liteparse_page_objects_free(handle) };

    assert_eq!(
        unsafe {
            liteparse_document_page_objects(document.0, ptr::null(), 0, 1 << 10, &mut handle)
        },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("unknown bits");
}

#[test]
fn orientation_corrections_reach_every_direct_pdfium_path() {
    let corrections = [LiteParsePageOrientationCorrection { page: 1, angle: 90 }];
    let parser = Parser::new(|c| {
        c.orientation_corrections = corrections.as_ptr();
        c.orientation_corrections_len = 1;
    });
    let document = parser.open("sample_rotated_90cw.pdf");
    let upright = Parser::plain().open("sample.pdf");

    let corrected = document.parse(&[]);
    let reference = upright.parse(&[]);
    let (page, expected) = (&corrected.pages()[0], &reference.pages()[0]);
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_ROTATION, 0);
    assert_eq!(page.geometry.rotation_quarter_turns, 0);
    assert!((page.width - expected.width).abs() < 0.01);
    assert!((page.height - expected.height).abs() < 0.01);

    let extracted = document.extract(&[]);
    assert_eq!(extracted.pages()[0].geometry.rotation_quarter_turns, 0);

    let shots = document.screenshot(&[], 0.0, None).unwrap();
    let reference_shots = upright.screenshot(&[], 0.0, None).unwrap();
    assert_eq!(
        (shots.shots()[0].width, shots.shots()[0].height),
        (
            reference_shots.shots()[0].width,
            reference_shots.shots()[0].height
        )
    );

    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_raw_text(document.0, ptr::null(), 0, &mut handle) },
        LITEPARSE_STATUS_OK
    );
    let raw = unsafe { liteparse_raw_text_view(handle).as_ref() }.unwrap();
    assert_eq!(
        arr(raw.pages, raw.pages_len)[0]
            .geometry
            .rotation_quarter_turns,
        0
    );
    unsafe { liteparse_raw_text_free(handle) };
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
                    let mut handle = ptr::null_mut();
                    let status =
                        unsafe { liteparse_document_parse(shared.0, ptr::null(), 0, &mut handle) };
                    assert_eq!(status, LITEPARSE_STATUS_OK);
                    Res(handle).text()
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

#[test]
fn records_hold_no_pointers_besides_byte_views() {
    // Fixed-width records: no `size_t`, so sizes are the same on every
    // 64-bit target and independent of pointer width for pointer-free ones.
    assert_eq!(size_of::<LiteParseRect>(), 16);
    assert_eq!(size_of::<LiteParseLayoutRow>(), 8);
    assert_eq!(size_of::<LiteParseProjectedRegion>(), 32);
    assert_eq!(size_of::<LiteParsePageComplexity>(), 68);
    assert_eq!(size_of::<LiteParseByteView>(), 2 * size_of::<usize>());
    assert_eq!(align_of::<LiteParseTextItem>(), align_of::<usize>());
}
