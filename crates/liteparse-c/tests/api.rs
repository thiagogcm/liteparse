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

fn blank_pages_pdf(width: u32, height: u32, count: u32) -> Vec<u8> {
    let kids = (3..count + 3)
        .map(|number| format!("{number} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {count} >>").into_bytes(),
    ];
    for _ in 0..count {
        objects.push(
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] >>")
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

/// Two tagged pages that draw MCIDs 0 and 1 each, joined to a list body that
/// spans them. The body owns page 1's MCID 1 as a bare kid under its own
/// `/Pg` and page 2's MCID 0 through an MCR with its own `/Pg`, and parents
/// the paragraph that owns page 2's MCID 1; page 1's MCID 0 is an unrelated
/// paragraph's. The body sits in both pages' trees, so a reader that ignores
/// each kid's page claims both of its numbers on both pages, where the other
/// number belongs to someone else.
fn cross_page_structure_pdf() -> Vec<u8> {
    let page1 = b"BT /F1 12 Tf 72 700 Td /P << /MCID 0 >> BDC (Opening paragraph.) Tj EMC ET\n\
                  BT /F1 12 Tf 72 600 Td /LBody << /MCID 1 >> BDC (Body starts here.) Tj EMC ET";
    let page2 = b"BT /F1 12 Tf 72 700 Td /LBody << /MCID 0 >> BDC (Body ends here.) Tj EMC ET\n\
                  BT /F1 12 Tf 72 600 Td /P << /MCID 1 >> BDC (Nested paragraph.) Tj EMC ET";
    let page = |contents: u32, parents: u32| {
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {contents} 0 R \
             /Resources << /Font << /F1 12 0 R >> >> /StructParents {parents} >>"
        )
        .into_bytes()
    };
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 7 0 R >>"
            .to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        page(5, 0),
        page(6, 1),
        stream("", page1),
        stream("", page2),
        b"<< /Type /StructTreeRoot /K [8 0 R] /ParentTree 13 0 R >>".to_vec(),
        b"<< /Type /StructElem /S /Document /P 7 0 R /K [9 0 R 10 0 R] >>".to_vec(),
        b"<< /Type /StructElem /S /P /P 8 0 R /Pg 3 0 R /K 0 >>".to_vec(),
        b"<< /Type /StructElem /S /LBody /P 8 0 R /Pg 3 0 R \
           /K [1 << /Type /MCR /Pg 4 0 R /MCID 0 >> 11 0 R] >>"
            .to_vec(),
        b"<< /Type /StructElem /S /P /P 10 0 R /Pg 4 0 R /K 1 >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        b"<< /Nums [0 [9 0 R 10 0 R] 1 [10 0 R 11 0 R]] >>".to_vec(),
    ])
}

fn outlined_two_page_pdf() -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R /Resources << /Font << /F1 8 0 R >> >> >>".to_vec(),
        stream("", b"BT /F1 18 Tf 72 700 Td (Outlined Title) Tj ET"),
        b"<< /Type /Outlines /First 7 0 R /Last 7 0 R /Count 1 >>".to_vec(),
        b"<< /Title (Outlined Title) /Parent 6 0 R /Dest [4 0 R /Fit] >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
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

/// Three pages of text; the second's crop box misses its media box, so its
/// visible box is empty and it has no area.
fn zero_area_page_pdf() -> Vec<u8> {
    let page = |crop: &str, contents: u32| {
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]{crop} \
             /Resources << /Font << /F1 6 0 R >> >> /Contents {contents} 0 R >>"
        )
        .into_bytes()
    };
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>".to_vec(),
        page("", 7),
        page(" /CropBox [700 800 900 1000]", 8),
        page("", 9),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        stream("", b"BT /F1 24 Tf 72 700 Td (Visible page one.) Tj ET"),
        stream("", b"BT /F1 24 Tf 72 700 Td (Hidden page two.) Tj ET"),
        stream("", b"BT /F1 24 Tf 72 700 Td (Visible page three.) Tj ET"),
    ])
}

/// Three pages whose second holds a filled text widget, with or without the
/// `/AcroForm` that should list its field: without it the widget is
/// orphaned, and liteparse repairs the catalog to read the field.
fn widget_on_second_page_pdf(acroform: bool) -> Vec<u8> {
    let catalog = if acroform {
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [6 0 R] >> >>"
    } else {
        "<< /Type /Catalog /Pages 2 0 R >>"
    };
    assemble(&[
        catalog.as_bytes().to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [6 0 R] >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_vec(),
        b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (customer_name) /V (Ada Lovelace) \
           /Rect [72 700 300 720] /F 4 /P 4 0 R >>"
            .to_vec(),
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

fn many_page_objects_pdf(count: u32) -> Vec<u8> {
    let content = b"0 0 1 1 re S\n".repeat(count as usize);
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >>".to_vec(),
        stream("", &content),
    ])
}

fn nested_form_objects_pdf(levels: u32) -> Vec<u8> {
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /XObject << /F 5 0 R >> >> >>".to_vec(),
        stream("", b"/F Do"),
    ];
    for depth in 0..levels {
        let child = depth + 6;
        let (resources, content) = if depth + 1 < levels {
            (
                format!("/Resources << /XObject << /F {child} 0 R >> >>"),
                b"/F Do".as_slice(),
            )
        } else {
            (String::new(), b"0 0 1 1 re S".as_slice())
        };
        objects.push(stream(
            &format!("/Type /XObject /Subtype /Form /BBox [0 0 100 100] {resources}"),
            content,
        ));
    }
    assemble(&objects)
}

fn image_with_unsupported_filter_pdf() -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R >> >> >>".to_vec(),
        stream("", b"q 40 0 0 20 100 50 cm /Im1 Do Q"),
        stream(
            "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /UnsupportedDecode",
            &[0x00, 0xff, 0xff, 0x00],
        ),
    ])
}

fn empty_image_in_form_pdf() -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /XObject << /Fx1 5 0 R >> >> >>".to_vec(),
        stream("", b"q /Fx1 Do Q"),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /Im1 6 0 R >> >>",
            b"q 40 0 0 20 10 10 cm /Im1 Do Q",
        ),
        stream(
            "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8",
            b"",
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
/// Anything that carries a string pool.
trait Pooled {
    fn pool(&self) -> &[u8];
}

impl Pooled for LiteParseArenas {
    fn pool(&self) -> &[u8] {
        arr(self.pool, self.pool_len)
    }
}

impl Pooled for LiteParseContentArrays {
    fn pool(&self) -> &[u8] {
        self.arenas.pool()
    }
}

impl Pooled for LiteParseResultView {
    fn pool(&self) -> &[u8] {
        self.content.pool()
    }
}

impl Pooled for LiteParseRawTextView {
    fn pool(&self) -> &[u8] {
        self.arenas.pool()
    }
}

impl Pooled for LiteParsePageObjectsView {
    fn pool(&self) -> &[u8] {
        self.arenas.pool()
    }
}

impl Pooled for LiteParseDocumentInfo {
    fn pool(&self) -> &[u8] {
        arr(self.pool, self.pool_len)
    }
}

/// Read a pooled string, checking its range and NUL terminator.
fn pooled(owner: &impl Pooled, value: LiteParseStr) -> String {
    let pool = owner.pool();
    let start = value.offset as usize;
    let end = start + value.len as usize;
    assert!(
        end < pool.len(),
        "string {start}..{end} outside pool of {}",
        pool.len()
    );
    assert_eq!(pool[end], 0, "pooled strings are NUL-terminated");
    String::from_utf8(pool[start..end].to_vec()).expect("utf-8")
}

fn check_str(owner: &impl Pooled, value: LiteParseStr) {
    pooled(owner, value);
}

/// Builds the caller-side pool for `liteparse_parser_parse_content`. It has
/// no leading NUL and no terminators: input only needs the ranges.
#[derive(Default)]
struct TestPool(Vec<u8>);

impl TestPool {
    fn s(&mut self, text: &[u8]) -> LiteParseStr {
        let offset = self.0.len() as u32;
        self.0.extend_from_slice(text);
        LiteParseStr {
            offset,
            len: text.len() as u32,
        }
    }

    fn install(&self, content: &mut LiteParseContentArrays) {
        content.arenas.pool = self.0.as_ptr();
        content.arenas.pool_len = self.0.len();
    }
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
    let mut out = LiteParseByteView::default();
    unsafe { liteparse_last_error(&mut out) };
    view_str(out)
}

fn last_error_contains(fragment: &str) {
    let message = last_error();
    assert!(message.contains(fragment), "last error was: {message}");
}

fn config(tweak: impl FnOnce(&mut LiteParseConfig)) -> LiteParseConfig {
    let mut config = std::mem::MaybeUninit::<LiteParseConfig>::uninit();
    unsafe { liteparse_config_init(config.as_mut_ptr()) };
    let mut config = unsafe { config.assume_init() };
    config.options &= !LITEPARSE_FLAG_OCR_ENABLED;
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
            |out| unsafe { liteparse_document_open_path(self.0, path.as_ptr(), path.len(), out) },
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

    fn parse_content(&self, content: &LiteParseContentArrays) -> Result<Res, LiteParseStatus> {
        let mut input = std::mem::MaybeUninit::<LiteParseContentInput>::uninit();
        unsafe { liteparse_content_init(input.as_mut_ptr()) };
        let input = LiteParseContentInput {
            content: *content,
            ..unsafe { input.assume_init() }
        };
        call(
            |out| unsafe { liteparse_parser_parse_content(self.0, &input, out) },
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

    fn page_objects(&self, flags: u32) -> Result<PageObjects, LiteParseStatus> {
        call(
            |out| unsafe { liteparse_document_page_objects(self.0, ptr::null(), 0, flags, out) },
            PageObjects,
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
        pooled(self.view(), self.view().text)
    }

    fn pages(&self) -> &[LiteParsePage] {
        let content = &self.view().content;
        arr(content.pages, content.pages_len)
    }

    fn outputs(&self) -> &[LiteParsePageOutput] {
        arr(self.view().page_outputs, self.view().page_outputs_len)
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

#[derive(Debug)]
struct PageObjects(*mut LiteParsePageObjects);

impl PageObjects {
    fn view(&self) -> &LiteParsePageObjectsView {
        unsafe { liteparse_page_objects_view(self.0).as_ref() }.expect("view")
    }
}

impl Drop for PageObjects {
    fn drop(&mut self) {
        unsafe { liteparse_page_objects_free(self.0) };
    }
}

const ALL_IMAGE_PAYLOADS: u32 = LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_RAW
    | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_DECODED
    | LITEPARSE_PAGE_OBJECT_INCLUDE_IMAGE_BITMAP;

const UNAVAILABLE_PAYLOADS: u32 = LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_RAW_UNAVAILABLE
    | LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_DECODED_UNAVAILABLE
    | LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_BITMAP_UNAVAILABLE;

/// Every offset/count in a content view must land inside its array.
fn check_content_ranges(content: &LiteParseContentArrays) {
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
    for entry in arr(content.outline, content.outline_len) {
        check_str(content, entry.title);
    }
    for error in arr(content.arenas.page_errors, content.arenas.page_errors_len) {
        check_str(content, error.message);
    }
    for image in arr(content.images, content.images_len) {
        check_str(content, image.id);
        check_str(content, image.format);
        assert!(
            (image.bytes.offset as usize + image.bytes.len as usize) <= content.arenas.binary_len
        );
    }

    for item in items {
        check_str(content, item.text);
        check_str(content, item.font_name);
        check_str(content, item.link);
        range(words, item.word_offset, item.word_count);
        range(char_codes, item.char_code_offset, item.char_code_count);
    }
    for string in strings {
        check_str(content, *string);
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
        check_str(content, page.label);
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
    check_str(view, view.text);
    check_str(view, view.creator);
    check_str(view, view.producer);
    for line in arr(view.projected_lines, view.projected_lines_len) {
        check_str(view, line.text);
        check_str(view, line.dominant_font_name);
    }
    let content = &view.content;
    let pages = arr(content.pages, content.pages_len);
    let words = arr(content.words, content.words_len);
    let char_codes = arr(content.char_codes, content.char_codes_len);
    let outputs = arr(view.page_outputs, view.page_outputs_len);
    assert!(outputs.is_empty() || outputs.len() == pages.len());
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
    let numbers: Vec<u32> = pages.iter().map(|page| page.page_number).collect();
    for signal in [
        arr(view.flattened_page_numbers, view.flattened_page_numbers_len),
        arr(view.repaired_page_numbers, view.repaired_page_numbers_len),
    ] {
        assert!(signal.is_sorted());
        assert!(signal.iter().all(|number| numbers.contains(number)));
    }
    assert!(
        view.repaired_page_numbers_len == 0
            || view.flags & LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM != 0
    );
    for output in outputs {
        check_str(view, output.text);
        check_str(view, output.markdown);
        range(figures, output.figure_offset, output.figure_count);
        range(frames, output.item_frame_offset, output.item_frame_count);
        range(
            lines,
            output.projected_line_offset,
            output.projected_line_count,
        );
        range(regions, output.region_offset, output.region_count);
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
        assert!(
            (shot.png.offset as usize + shot.png.len as usize) <= view.content.arenas.binary_len
        );
        range(rects, shot.rect_offset, shot.rect_count);
    }
}

// ---------------------------------------------------------------------------

#[test]
fn version_and_last_error() {
    let mut version = LiteParseByteView::default();
    unsafe { liteparse_version(&mut version) };
    assert_eq!(view_str(version), env!("CARGO_PKG_VERSION"));
    assert_eq!(
        liteparse_sizeof(LITEPARSE_TYPE_CONFIG),
        size_of::<LiteParseConfig>()
    );
    assert_eq!(liteparse_sizeof(LITEPARSE_TYPE_STR), 8);
    assert_eq!(liteparse_sizeof(9999), 0);
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
        assert!(liteparse_screenshots_view(ptr::null()).is_null());
        assert!(liteparse_complexity_view(ptr::null()).is_null());
        assert!(liteparse_raw_text_view(ptr::null()).is_null());
        assert!(liteparse_page_objects_view(ptr::null()).is_null());
        let mut handle = ptr::null_mut();
        assert_eq!(
            liteparse_document_parse(ptr::null(), ptr::null(), 0, &mut handle),
            LITEPARSE_STATUS_INVALID_ARGUMENT
        );
        liteparse_parser_free(ptr::null_mut());
        liteparse_document_free(ptr::null_mut());
        liteparse_result_free(ptr::null_mut());
        liteparse_screenshots_free(ptr::null_mut());
        liteparse_complexity_free(ptr::null_mut());
        liteparse_raw_text_free(ptr::null_mut());
        liteparse_page_objects_free(ptr::null_mut());
        liteparse_config_init(ptr::null_mut());
        liteparse_content_init(ptr::null_mut());
    }
}

#[test]
fn fallible_outputs_reject_null_pointers_without_doing_work() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    assert_eq!(
        unsafe { liteparse_document_complexity(document.0, ptr::null(), 0, ptr::null_mut()) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("out pointer");
    assert_eq!(
        unsafe { liteparse_document_parse(document.0, ptr::null(), 0, ptr::null_mut()) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    // Void helpers continue to accept null.
    unsafe {
        liteparse_version(ptr::null_mut());
        liteparse_last_error(ptr::null_mut());
        liteparse_config_init(ptr::null_mut());
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
        c.options |= LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX;
        c.crop_box = [0.6, 0.0, 0.6, 0.0];
    });
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "unknown bits", &|c| {
        c.options = 1u64 << 62
    });
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "output format", &|c| {
        c.output_format = 9
    });
    reject(LITEPARSE_STATUS_INVALID_ARGUMENT, "struct_size", &|c| {
        c.struct_size -= 1
    });
    reject(LITEPARSE_STATUS_INVALID_ARGUMENT, "UTF-8", &|c| {
        c.font_db_dir = view(&[0xff, 0xfe])
    });
    reject(
        LITEPARSE_STATUS_INVALID_ARGUMENT,
        "must not be null when",
        &|c| {
            c.font_db_dir = LiteParseByteView {
                ptr: ptr::null(),
                len: 3,
            };
        },
    );
    reject(LITEPARSE_STATUS_INVALID_CONFIG, "45", &|c| {
        static BAD: [LiteParsePageOrientationCorrection; 1] =
            [LiteParsePageOrientationCorrection { page: 1, angle: 45 }];
        c.orientation_corrections = BAD.as_ptr();
        c.orientation_corrections_len = 1;
    });
    // An identity crop box is accepted and only drops nothing.
    let cropped = Parser::new(|c| c.options |= LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX);
    assert!(!cropped.open("sample.pdf").parse(&[]).text().is_empty());
}

#[test]
fn max_pages_zero_parses_no_pages() {
    let parser = Parser::new(|c| c.max_pages = 0);
    let document = parser.open("sample.pdf");
    let parsed = document.parse(&[]);
    assert_eq!(parsed.pages().len(), 0);
    assert_eq!(
        parsed.view().content.total_pages,
        document.info().total_pages
    );
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
fn oversized_page_selection_is_rejected_before_reading_or_copying() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let invalid_pages = std::ptr::NonNull::<u32>::dangling().as_ptr();
    let oversized = LITEPARSE_MAX_SELECTED_PAGES as usize + 1;
    let mut result = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_parse(document.0, invalid_pages, oversized, &mut result) },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(result.is_null());
    last_error_contains("selection of");

    assert_eq!(
        unsafe { liteparse_document_extract(document.0, invalid_pages, oversized, &mut result) },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(result.is_null());

    let mut shots = ptr::null_mut();
    assert_eq!(
        unsafe {
            liteparse_document_screenshot(
                document.0,
                invalid_pages,
                oversized,
                0.0,
                ptr::null(),
                &mut shots,
            )
        },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(shots.is_null());

    let mut complexity = ptr::null_mut();
    assert_eq!(
        unsafe {
            liteparse_document_complexity(document.0, invalid_pages, oversized, &mut complexity)
        },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(complexity.is_null());

    let mut raw_text = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_raw_text(document.0, invalid_pages, oversized, &mut raw_text) },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(raw_text.is_null());

    let mut objects = ptr::null_mut();
    assert_eq!(
        unsafe {
            liteparse_document_page_objects(document.0, invalid_pages, oversized, 0, &mut objects)
        },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(objects.is_null());

    // A null pointer with nonzero length remains an invalid argument.
    assert_eq!(
        unsafe { liteparse_document_parse(document.0, ptr::null(), oversized, &mut result) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );

    let within_limit = vec![1u32; LITEPARSE_MAX_SELECTED_PAGES as usize];
    let parsed = document.try_parse(&within_limit);
    assert!(parsed.is_ok(), "{}", last_error());
    assert_eq!(parsed.unwrap().pages().len(), 1);
}

#[test]
fn continue_on_page_error_never_swallows_argument_errors() {
    let parser = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
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
        c.options |= LITEPARSE_FLAG_EMIT_WORD_BOXES
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
    assert_eq!(view.content.total_pages, 4);
    let pages = parsed.pages();
    assert_eq!(pages.len(), 4);
    let labels: Vec<String> = pages.iter().map(|p| pooled(view, p.label)).collect();
    assert_eq!(labels, ["i", "ii", "1", "2"]);
    let info = document.info();
    assert_eq!(info.total_pages, 4);
    for entry in arr(info.outline, info.outline_len) {
        check_str(info, entry.title);
    }
    assert_eq!(parsed.outputs().len(), pages.len());
    for (page, output) in pages.iter().zip(parsed.outputs()) {
        assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_GEOMETRY, 0);
        assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY, 0);
        assert_eq!(page.complexity.page_number, page.page_number);
        assert_eq!(page.flags & LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS, 0);
        assert_eq!(page.flags & LITEPARSE_PAGE_FLAG_HAS_BLOCKS, 0);
        assert!(page.width > 0.0 && page.height > 0.0);
        assert!(!pooled(view, output.text).is_empty());
        assert!(pooled(view, output.markdown).is_empty());
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
    assert!(!pooled(view, word.text).is_empty());
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
        c.options |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS
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
fn screenshots_render_whole_pages_and_cropped_regions() {
    let parser = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS;
    });
    let document = parser.open("sample.pdf");
    let full = document.screenshot(&[], 0.0, None).unwrap();
    assert_eq!(full.shots().len() as u32, document.info().total_pages);
    assert_eq!(
        &blob(
            full.view().arenas.binary,
            full.view().arenas.binary_len,
            full.shots()[0].png
        )[..4],
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
fn region_screenshots_are_charged_for_the_region_alone() {
    let region = LiteParseRenderRegion {
        x: 100.0,
        y: 200.0,
        width: 10.0,
        height: 20.0,
    };
    let plain = Parser::new(|config| config.dpi = 72.0);
    let document = plain.open_bytes(&blank_pdf(10_000, 10_000));
    let shots = document.screenshot(&[1], 0.0, Some(region)).unwrap();
    let shot = arr(shots.view().screenshots, shots.view().screenshots_len)[0];
    assert_eq!((shot.width, shot.height), (10, 20));
    assert_ne!(shot.flags & LITEPARSE_SCREENSHOT_FLAG_SOLID_FILL, 0);
    assert_eq!(
        document.screenshot(&[1], 0.0, None).err().unwrap(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("per-page limit");

    // Rectangle detection reads the whole page, so it is charged in full.
    let detecting = Parser::new(|config| {
        config.dpi = 72.0;
        config.options |=
            LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS | LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
    });
    let document = detecting.open_bytes(&blank_pdf(10_000, 10_000));
    assert_eq!(
        document.screenshot(&[1], 0.0, Some(region)).err().unwrap(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("per-page limit");
}

#[test]
fn screenshot_work_budget_applies_to_batch_and_parse_paths() {
    let parser = Parser::new(|config| {
        config.dpi = 72.0;
        config.options |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
        config.options |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
    });
    let huge = parser.open_bytes(&blank_pdf(10_000, 10_000));
    assert_eq!(
        huge.try_parse(&[]).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("per-page limit");

    let batch = parser.open_bytes(&blank_pages_pdf(5_000, 5_000, 3));
    assert_eq!(
        batch.screenshot(&[], 0.0, None).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("operation limit");
    assert_eq!(
        batch.try_parse(&[]).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("operation limit");
}

#[test]
fn effective_dpi_reports_the_long_edge_cap_on_both_render_paths() {
    let parser = Parser::new(|c| {
        c.dpi = 400.0;
        c.options |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS;
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

/// A job handle freed on drop unless finished.
struct Job(*mut LiteParseJob);

impl Drop for Job {
    fn drop(&mut self) {
        unsafe { liteparse_job_free(self.0) };
    }
}

impl Job {
    fn render(
        &mut self,
        pages: &[u32],
        format: u32,
    ) -> Result<LiteParseOcrRasterView, LiteParseStatus> {
        let mut view = ptr::null();
        let status = unsafe {
            liteparse_job_render_ocr(self.0, pages.as_ptr(), pages.len(), 0, format, &mut view)
        };
        if status != LITEPARSE_STATUS_OK {
            assert!(view.is_null());
            return Err(status);
        }
        Ok(unsafe { *view })
    }

    fn merge(&mut self, recognition: &Recognition) -> LiteParseStatus {
        unsafe {
            liteparse_job_merge_ocr(
                self.0,
                recognition.inputs.as_ptr(),
                recognition.inputs.len(),
                recognition.words.as_ptr(),
                recognition.words.len(),
                recognition.pool.0.as_ptr(),
                recognition.pool.0.len(),
            )
        }
    }

    fn finish(mut self) -> Result<Res, LiteParseStatus> {
        let result = call(|out| unsafe { liteparse_job_finish(&mut self.0, out) }, Res);
        assert!(self.0.is_null(), "finish consumes the job");
        result
    }
}

impl Document {
    fn begin(&self, pages: &[u32]) -> Job {
        call(
            |out| unsafe { liteparse_document_begin(self.0, pages.as_ptr(), pages.len(), out) },
            Job,
        )
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }
}

/// Host recognition for one round: one word per raster, or one error.
#[derive(Default)]
struct Recognition {
    pool: TestPool,
    words: Vec<LiteParseOcrWord>,
    inputs: Vec<LiteParseOcrPageInput>,
}

impl Recognition {
    fn words(view: &LiteParseOcrRasterView, text: &str) -> Self {
        let mut recognition = Self::default();
        for raster in arr(view.rasters, view.rasters_len) {
            let text = recognition.text(text);
            recognition.inputs.push(LiteParseOcrPageInput {
                page_number: raster.page_number,
                word_offset: recognition.words.len() as u32,
                word_count: 1,
                error: LiteParseStr::default(),
            });
            recognition.words.push(LiteParseOcrWord {
                text,
                x2: 40.0,
                y2: 10.0,
                confidence: 0.9,
                ..LiteParseOcrWord::default()
            });
        }
        recognition
    }

    fn errors(view: &LiteParseOcrRasterView, message: &str) -> Self {
        let mut recognition = Self::default();
        for raster in arr(view.rasters, view.rasters_len) {
            let error = recognition.text(message);
            recognition.inputs.push(LiteParseOcrPageInput {
                page_number: raster.page_number,
                error,
                ..LiteParseOcrPageInput::default()
            });
        }
        recognition
    }

    fn text(&mut self, text: &str) -> LiteParseStr {
        self.pool.s(text.as_bytes())
    }
}

#[test]
fn staged_ocr_words_land_in_the_result() {
    let parser = Parser::plain();
    let document = parser.open("receipt.png");
    assert_ne!(document.info().flags & LITEPARSE_DOCUMENT_FLAG_CONVERTED, 0);
    let mut job = document.begin(&[]);
    drop(document);
    let mut rounds = 0;
    loop {
        let view = job
            .render(&[], LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE)
            .unwrap();
        if view.rasters_len == 0 {
            break;
        }
        rounds += 1;
        for raster in arr(view.rasters, view.rasters_len) {
            assert_eq!(raster.pixel_format, LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE);
            assert_eq!(raster.pixels.len, raster.width * raster.height);
            assert!(
                raster.pixels.offset as usize + raster.pixels.len as usize
                    <= view.arenas.binary_len
            );
            assert_eq!(raster.flags & LITEPARSE_OCR_RASTER_FLAG_HAS_NATIVE_TEXT, 0);
            assert!(raster.dpi > 0.0);
        }
        assert_eq!(
            job.merge(&Recognition::words(&view, "zzocrzz")),
            LITEPARSE_STATUS_OK
        );
    }
    assert!(rounds >= 1);
    let result = job.finish().unwrap();
    assert!(
        result.text().contains("zzocrzz"),
        "text was: {}",
        result.text()
    );
}

/// A recognized word carries its own word box exactly when extraction gives
/// native text one: under `LITEPARSE_FLAG_EMIT_WORD_BOXES` (or Markdown).
#[test]
fn staged_ocr_items_carry_word_boxes_when_extraction_does() {
    for with_boxes in [false, true] {
        let parser = if with_boxes {
            Parser::new(|c| c.options |= LITEPARSE_FLAG_EMIT_WORD_BOXES)
        } else {
            Parser::plain()
        };
        let document = parser.open("receipt.png");
        let mut job = document.begin(&[]);
        drop(document);
        loop {
            let view = job
                .render(&[], LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE)
                .unwrap();
            if view.rasters_len == 0 {
                break;
            }
            assert_eq!(
                job.merge(&Recognition::words(&view, "zzocrzz")),
                LITEPARSE_STATUS_OK
            );
        }
        let result = job.finish().unwrap();
        let view = result.view();
        let items = arr(view.content.items, view.content.items_len);
        let recognized = items
            .iter()
            .find(|item| pooled(view, item.text) == "zzocrzz")
            .expect("the recognized word is a text item");
        if with_boxes {
            assert_eq!(recognized.word_count, 1);
            let words = arr(view.content.words, view.content.words_len);
            let word = &range(words, recognized.word_offset, recognized.word_count)[0];
            assert_eq!(pooled(view, word.text), "zzocrzz");
            assert!(word.width > 0.0 && word.height > 0.0);
        } else {
            assert_eq!(recognized.word_count, 0);
        }
    }
}

#[test]
fn staged_ocr_validates_every_input_before_merging() {
    let parser = Parser::plain();
    let document = parser.open("receipt.png");
    let mut job = document.begin(&[]);
    assert_eq!(
        job.merge(&Recognition::default()),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("no rendered OCR round");
    let view = job.render(&[], LITEPARSE_OCR_PIXEL_FORMAT_RGB).unwrap();
    assert!(view.rasters_len >= 1);
    let valid = Recognition::words(&view, "zzocrzz");
    let reject = |job: &mut Job, bad: &Recognition, fragment: &str| {
        assert_eq!(job.merge(bad), LITEPARSE_STATUS_INVALID_ARGUMENT);
        last_error_contains(fragment);
    };

    let mut unknown_page = Recognition::words(&view, "x");
    unknown_page.inputs[0].page_number = 999;
    reject(&mut job, &unknown_page, "was not rendered");
    let mut repeated = Recognition::words(&view, "x");
    repeated.inputs.push(repeated.inputs[0]);
    reject(&mut job, &repeated, "repeats");
    let mut out_of_range = Recognition::words(&view, "x");
    out_of_range.inputs[0].word_count = 9;
    reject(&mut job, &out_of_range, "words range");
    let mut error_and_words = Recognition::words(&view, "x");
    error_and_words.inputs[0].error = LiteParseStr { offset: 0, len: 1 };
    reject(&mut job, &error_and_words, "both an error and words");
    let mut bad_box = Recognition::words(&view, "x");
    bad_box.words[0].x2 = f32::NAN;
    reject(&mut job, &bad_box, "bbox");
    let mut bad_text = Recognition::words(&view, "x");
    bad_text.words[0].text.len = 99;
    reject(&mut job, &bad_text, "string pool");

    // Rejected merges left the round in place.
    assert_eq!(job.merge(&valid), LITEPARSE_STATUS_OK);
    assert_eq!(job.merge(&valid), LITEPARSE_STATUS_INVALID_ARGUMENT);
    assert_eq!(
        job.render(&[], 7).err().unwrap(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        job.render(&[99], LITEPARSE_OCR_PIXEL_FORMAT_RGB)
            .err()
            .unwrap(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    let mut view = ptr::null();
    assert_eq!(
        unsafe { liteparse_job_render_ocr(job.0, ptr::null(), 0, 0, 0, ptr::null_mut()) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        unsafe { liteparse_job_render_ocr(ptr::null_mut(), ptr::null(), 0, 0, 0, &mut view) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
}

#[test]
fn staged_ocr_errors_are_page_errors_unless_fatal() {
    let tolerant =
        Parser::new(|c| c.options &= !LITEPARSE_FLAG_OCR_FAILURE_FATAL).open("receipt.png");
    let mut job = tolerant.begin(&[]);
    let view = job.render(&[], LITEPARSE_OCR_PIXEL_FORMAT_RGB).unwrap();
    assert_eq!(
        job.merge(&Recognition::errors(&view, "engine exploded")),
        LITEPARSE_STATUS_OK
    );
    let result = job.finish().unwrap();
    let content = &result.view().content;
    let errors = arr(content.arenas.page_errors, content.arenas.page_errors_len);
    assert_eq!(errors.len(), 1);
    assert_eq!(pooled(result.view(), errors[0].message), "engine exploded");

    let fatal = Parser::new(|c| c.options |= LITEPARSE_FLAG_OCR_FAILURE_FATAL);
    let document = fatal.open("receipt.png");
    let mut job = document.begin(&[]);
    let view = job.render(&[], LITEPARSE_OCR_PIXEL_FORMAT_RGB).unwrap();
    assert_eq!(
        job.merge(&Recognition::errors(&view, "engine exploded")),
        LITEPARSE_STATUS_OCR_ERROR
    );
    last_error_contains("engine exploded");
    assert_eq!(
        job.render(&[], LITEPARSE_OCR_PIXEL_FORMAT_RGB)
            .err()
            .unwrap(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("fatal OCR merge");
    assert_eq!(job.finish().unwrap_err(), LITEPARSE_STATUS_INVALID_ARGUMENT);
}

#[test]
fn explicit_ocr_selection_renders_native_text_pages() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let mut job = document.begin(&[]);
    let view = job.render(&[1], LITEPARSE_OCR_PIXEL_FORMAT_RGB).unwrap();
    let rasters = arr(view.rasters, view.rasters_len);
    assert_eq!(rasters.len(), 1);
    assert_eq!(rasters[0].page_number, 1);
    assert_ne!(
        rasters[0].flags & LITEPARSE_OCR_RASTER_FLAG_HAS_NATIVE_TEXT,
        0
    );
    assert_eq!(
        rasters[0].pixels.len,
        rasters[0].width * rasters[0].height * 3
    );
    let rects = arr(view.image_rects, view.image_rects_len);
    range(
        rects,
        rasters[0].image_rect_offset,
        rasters[0].image_rect_count,
    );
    let finished = job.finish().unwrap();
    assert_eq!(finished.text(), document.parse(&[]).text());
}

#[test]
fn finish_requires_a_job() {
    let mut job = ptr::null_mut();
    let mut result = std::ptr::dangling_mut::<LiteParseResult>();
    assert_eq!(
        unsafe { liteparse_job_finish(&mut job, &mut result) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert!(result.is_null());
    assert_eq!(
        unsafe { liteparse_job_finish(ptr::null_mut(), &mut result) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    unsafe { liteparse_job_free(ptr::null_mut()) };
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
        pages
            .iter()
            .map(|p| pooled(view, p.label))
            .collect::<Vec<_>>(),
        ["i", "ii", "1", "2"]
    );
    for name in glyph_names {
        check_str(view, *name);
    }
    for page in pages {
        assert_ne!(page.flags & LITEPARSE_RAW_PAGE_FLAG_HAS_GEOMETRY, 0);
        let page_items = range(items, page.item_offset, page.item_count);
        assert!(!page_items.is_empty());
        for item in page_items {
            let codes = range(char_codes, item.char_code_offset, item.char_code_count);
            assert!(!codes.is_empty());
            assert!(!pooled(view, item.text).is_empty());
            check_str(view, item.font_name);
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
    assert_eq!(
        (pages.len(), pooled(view, pages[0].label).as_str()),
        (1, "1")
    );
    unsafe { liteparse_raw_text_free(handle) };
}

#[test]
fn complexity_view_covers_every_page() {
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
    assert_eq!(view.arenas.page_errors_len, 0);
    unsafe { liteparse_complexity_free(handle) };
}

#[test]
fn extract_round_trips_into_parse_content() {
    let parser = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_EMIT_WORD_BOXES;
    });
    let document = parser.open("page_labels.pdf");
    let extracted = document.extract(&[]);
    let view = extracted.view();
    check_result_ranges(view);
    assert_ne!(view.flags & LITEPARSE_RESULT_FLAG_EXTRACT_ONLY, 0);
    assert_eq!(view.text.len, 0);
    assert_eq!(view.projected_lines_len, 0);
    let pages = extracted.pages();
    assert_eq!(pages.len(), 4);
    assert_eq!(pooled(view, pages[1].label), "ii");
    assert!(extracted.outputs().is_empty());
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
            .map(|p| pooled(reprojected.view(), p.label))
            .collect::<Vec<_>>(),
        ["i", "ii", "1", "2"]
    );
}

#[test]
fn selected_extract_preserves_source_outline_and_geometry_through_projection() {
    let parser = Parser::new(|config| config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN);
    let document = parser.open_bytes(&outlined_two_page_pdf());
    assert_eq!(document.info().total_pages, 2);
    let extracted = document.extract(&[2]);
    let view = extracted.view();
    assert_eq!(view.content.total_pages, 2);
    assert_eq!(extracted.pages().len(), 1);
    assert_eq!(extracted.pages()[0].page_number, 2);
    let outline = arr(view.content.outline, view.content.outline_len);
    assert_eq!(outline.len(), 1);
    assert_eq!(pooled(view, outline[0].title), "Outlined Title");
    assert_eq!(outline[0].page_index, 1);

    let projected = parser.parse_content(&view.content).unwrap();
    let projected_view = projected.view();
    let page = &projected.pages()[0];
    assert_eq!(projected_view.content.total_pages, 2);
    assert_eq!(projected_view.content.outline_len, 1);
    assert_ne!(page.flags & LITEPARSE_PAGE_FLAG_HAS_GEOMETRY, 0);
    assert_eq!(page.geometry.box_top, extracted.pages()[0].geometry.box_top);
    assert_eq!(
        page.geometry.user_unit,
        extracted.pages()[0].geometry.user_unit
    );
    assert_eq!(
        pooled(
            projected_view,
            arr(projected_view.content.outline, 1)[0].title
        ),
        "Outlined Title"
    );
    assert!(projected.text().contains("Outlined Title"));
}

#[test]
fn parse_content_retains_unblocked_sidecars_and_validates_source_facts() {
    let parser = Parser::plain();
    let mut pool = TestPool::default();
    let item = content_item(pool.s(b"body"), 100.0);
    let mut page = content_page(2, 0);
    page.flags = LITEPARSE_PAGE_FLAG_HAS_GEOMETRY | LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY;
    page.geometry = LiteParsePageGeometry {
        box_left: 0.0,
        box_bottom: 0.0,
        box_right: 612.0,
        box_top: 792.0,
        user_unit: 1.0,
        rotation_quarter_turns: 0,
    };
    page.complexity.page_number = 2;
    page.complexity.text_coverage = 0.25;
    let image_bytes = [7u8, 9];
    let image = LiteParseImage {
        id: pool.s(b"img-a"),
        format: pool.s(b"png"),
        bytes: LiteParseBlobRef {
            offset: 0,
            len: image_bytes.len() as u32,
        },
        page: 2,
        width: 2,
        height: 1,
        ..Default::default()
    };
    let page_error = LiteParsePageError {
        page_number: 1,
        message: pool.s(b"page failed"),
    };
    let mut content = empty_content();
    pool.install(&mut content);
    content.arenas.binary = image_bytes.as_ptr();
    content.arenas.binary_len = image_bytes.len();
    content.total_pages = 3;
    content.pages = &page;
    content.pages_len = 1;
    content.items = &item;
    content.items_len = 1;
    content.images = &image;
    content.images_len = 1;
    content.arenas.page_errors = &page_error;
    content.arenas.page_errors_len = 1;

    let projected = parser.parse_content(&content).unwrap();
    let view = projected.view();
    assert_eq!(view.content.total_pages, 3);
    assert_eq!(view.content.images_len, 1);
    assert_eq!(view.content.arenas.page_errors_len, 1);
    assert_eq!(
        blob(
            view.content.arenas.binary,
            view.content.arenas.binary_len,
            arr(view.content.images, 1)[0].bytes
        ),
        image_bytes
    );
    assert_eq!(
        pooled(view, arr(view.content.arenas.page_errors, 1)[0].message),
        "page failed"
    );
    assert_eq!(projected.pages()[0].complexity.text_coverage, 0.25);
    assert_eq!(projected.pages()[0].geometry.box_right, 612.0);

    content.arenas.binary_len = 1;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("images[0].bytes");
    content.arenas.binary_len = image_bytes.len();

    content.total_pages = 1;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("content.total_pages");
    content.total_pages = 3;
    let mut bad_page = page;
    bad_page.geometry.user_unit = 0.0;
    content.pages = &bad_page;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("geometry");
}

#[test]
fn content_input_budgets_reject_oversized_arrays_and_payloads_before_copying() {
    let parser = Parser::plain();
    let mut content = empty_content();
    content.items = std::ptr::dangling();
    content.items_len = usize::MAX;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("items_len");

    content.items = ptr::null();
    content.items_len = 0;
    let image = LiteParseImage {
        bytes: LiteParseBlobRef {
            offset: 0,
            len: LITEPARSE_MAX_SINGLE_BINARY_BYTES as u32 + 1,
        },
        ..Default::default()
    };
    content.images = &image;
    content.images_len = 1;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("images[0].bytes");
}

#[test]
fn content_input_budget_counts_reused_page_ranges_before_copying() {
    let parser = Parser::plain();
    let mut pool = TestPool::default();
    let text = vec![b'a'; 1024 * 1024];
    let item = content_item(pool.s(&text), 100.0);
    let pages: Vec<_> = (1..=300).map(|number| content_page(number, 0)).collect();
    let mut content = empty_content();
    pool.install(&mut content);
    content.pages = pages.as_ptr();
    content.pages_len = pages.len();
    content.total_pages = content.pages_len as u32;
    content.items = &item;
    content.items_len = 1;

    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("pages[");
}

#[test]
fn content_input_budget_counts_reused_outline_and_table_cells() {
    let parser = Parser::plain();
    let mut pool = TestPool::default();
    let title = pool.s(&vec![b'a'; 1024 * 1024]);
    let outline = vec![
        LiteParseOutlineEntry {
            title,
            ..Default::default()
        };
        300
    ];
    let mut content = empty_content();
    pool.install(&mut content);
    content.total_pages = 1;
    content.outline = outline.as_ptr();
    content.outline_len = outline.len();
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("outline[");

    content.outline = ptr::null();
    content.outline_len = 0;
    let mut page = content_page(1, 0);
    page.item_count = 0;
    page.block_count = 1;
    let block = LiteParseLayoutBlock {
        row_count: 300,
        ..Default::default()
    };
    let row = LiteParseLayoutRow {
        cell_count: 1,
        ..Default::default()
    };
    let rows = vec![row; 300];
    let cell = LiteParseLayoutCell {
        text: title,
        ..Default::default()
    };
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = content.pages_len as u32;
    content.blocks = &block;
    content.blocks_len = 1;
    content.rows = rows.as_ptr();
    content.rows_len = rows.len();
    content.cells = &cell;
    content.cells_len = 1;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("pages[0]");
}

#[test]
fn content_input_budget_ignores_unflagged_optional_page_ranges() {
    let parser = Parser::plain();
    let mut page = content_page(1, 0);
    page.item_count = 0;
    page.annotation_offset = u32::MAX;
    page.annotation_count = 1;
    page.form_field_offset = u32::MAX;
    page.form_field_count = 1;
    page.structure_node_offset = u32::MAX;
    page.structure_node_count = 1;
    let mut content = empty_content();
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = content.pages_len as u32;
    assert!(parser.parse_content(&content).is_ok());
}

#[test]
fn parse_content_canonicalizes_duplicate_image_references() {
    let parser = Parser::new(|config| {
        config.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        config.image_mode = LITEPARSE_IMAGE_MODE_PLACEHOLDER;
    });
    let mut pool = TestPool::default();
    let item = content_item(pool.s(b"Caption"), 150.0);
    let mut page = content_page(1, 0);
    page.image_ref_count = 1;
    let reference = LiteParseImageRef {
        id: pool.s(b"dup"),
        format: pool.s(b"png"),
        bbox: LiteParseRect {
            x: 30.0,
            y: 60.0,
            width: 80.0,
            height: 40.0,
        },
        ..Default::default()
    };
    let images = [
        LiteParseImage {
            id: pool.s(b"canonical"),
            name: pool.s(b"canonical.png"),
            format: pool.s(b"png"),
            page: 1,
            ..Default::default()
        },
        LiteParseImage {
            id: pool.s(b"dup"),
            name: pool.s(b"img_dup.png"),
            format: pool.s(b"png"),
            duplicate_of: pool.s(b"canonical"),
            page: 1,
            ..Default::default()
        },
    ];
    let mut content = empty_content();
    pool.install(&mut content);
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = content.pages_len as u32;
    content.items = &item;
    content.items_len = 1;
    content.image_refs = &reference;
    content.image_refs_len = 1;
    content.images = images.as_ptr();
    content.images_len = 2;
    let projected = parser.parse_content(&content).unwrap();
    assert_eq!(projected.view().content.images_len, 2);
    assert!(
        projected.text().contains("![](canonical.png)"),
        "{}",
        projected.text()
    );
    assert!(!projected.text().contains("img_dup.png"));
}

#[test]
fn parse_content_plain_text_does_not_rewrite_image_syntax() {
    let parser = Parser::plain();
    let mut pool = TestPool::default();
    let item = content_item(pool.s(b"literal ![](img_dup.png)"), 100.0);
    let page = content_page(1, 0);
    let images = [
        LiteParseImage {
            id: pool.s(b"canonical"),
            name: pool.s(b"canonical.png"),
            format: pool.s(b"png"),
            page: 1,
            ..Default::default()
        },
        LiteParseImage {
            id: pool.s(b"dup"),
            format: pool.s(b"png"),
            duplicate_of: pool.s(b"canonical"),
            page: 1,
            ..Default::default()
        },
    ];
    let mut content = empty_content();
    pool.install(&mut content);
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = content.pages_len as u32;
    content.items = &item;
    content.items_len = 1;
    content.images = images.as_ptr();
    content.images_len = images.len();

    let projected = parser.parse_content(&content).unwrap();
    assert!(projected.text().contains("![](img_dup.png)"));
    assert!(!projected.text().contains("canonical.png"));
}

fn content_item(text: LiteParseStr, y: f32) -> LiteParseTextItem {
    LiteParseTextItem {
        text,
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

fn blob<'a>(binary: *const u8, binary_len: usize, value: LiteParseBlobRef) -> &'a [u8] {
    let all = arr(binary, binary_len);
    range(all, value.offset, value.len)
}

fn empty_content() -> LiteParseContentArrays {
    LiteParseContentArrays::default()
}

#[test]
fn parse_content_projects_caller_text_without_opening_a_document() {
    let parser = Parser::plain();
    let mut pool = TestPool::default();
    let items = [content_item(pool.s(b"hello content"), 100.0)];
    let mut page = content_page(1, 0);
    page.label = pool.s(b"A-1");
    let mut content = empty_content();
    pool.install(&mut content);
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = content.pages_len as u32;
    content.items = items.as_ptr();
    content.items_len = 1;
    let parsed = parser.parse_content(&content).unwrap();
    check_result_ranges(parsed.view());
    assert!(parsed.text().contains("hello content"));
    assert_eq!(pooled(parsed.view(), parsed.pages()[0].label), "A-1");

    // A string range outside the pool is rejected, not read.
    let mut bad = content_item(
        LiteParseStr {
            offset: 1_000,
            len: 4,
        },
        100.0,
    );
    bad.flags = 0;
    let items = [bad];
    content.items = items.as_ptr();
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("string pool");
}

#[test]
fn parse_content_rejects_bad_ranges_kinds_and_sizes() {
    let parser = Parser::plain();
    let mut pool = TestPool::default();
    let x = pool.s(b"x");
    let items = [content_item(x, 100.0)];
    let block = LiteParseLayoutBlock {
        kind: 99,
        ..Default::default()
    };
    let attempt = |page: LiteParsePage| {
        let mut content = empty_content();
        pool.install(&mut content);
        content.total_pages = 1;
        content.pages = &page;
        content.pages_len = 1;
        content.items = items.as_ptr();
        content.items_len = 1;
        content.blocks = &block;
        content.blocks_len = 1;
        parser.parse_content(&content).unwrap_err()
    };

    assert_eq!(
        attempt(content_page(1, 5)),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("pages[0].items");

    let mut page = content_page(1, 0);
    page.block_count = 1;
    assert_eq!(attempt(page), LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("unknown kind");

    let mut input = std::mem::MaybeUninit::<LiteParseContentInput>::uninit();
    unsafe { liteparse_content_init(input.as_mut_ptr()) };
    let mut input = unsafe { input.assume_init() };
    input.struct_size -= 8;
    let mut out = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_parser_parse_content(parser.0, &input, &mut out) },
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    assert!(out.is_null());
    last_error_contains("struct_size");

    let mut page = content_page(1, 0);
    page.flags = 1 << 20;
    assert_eq!(attempt(page), LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("unknown bits");

    let mut page = content_page(1, 0);
    page.flags = LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY;
    page.complexity.text_coverage = f32::NAN;
    assert_eq!(attempt(page), LITEPARSE_STATUS_INVALID_ARGUMENT);
    last_error_contains("complexity must be finite");

    let reject_item = |item: LiteParseTextItem, fragment: &str| {
        let items = [item];
        let page = content_page(1, 0);
        let mut content = empty_content();
        pool.install(&mut content);
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
    let mut nan = content_item(x, 100.0);
    nan.flags = LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE;
    nan.font_size = f32::NAN;
    reject_item(nan, "font metrics");
    let mut unknown = content_item(x, 100.0);
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
        c.options |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
    });
    let mut pool = TestPool::default();
    let items = [content_item(pool.s(b"Body text"), 300.0)];
    let quadpoints = [LiteParseRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    }];
    let annotations = [
        LiteParseAnnotation {
            subtype: pool.s(b"link"),
            uri: pool.s(b"https://example.invalid"),
            rect: quadpoints[0],
            quadpoint_offset: 0,
            quadpoint_count: 1,
            flags: LITEPARSE_ANNOTATION_FLAG_HAS_RECT,
            ..Default::default()
        },
        LiteParseAnnotation {
            subtype: pool.s(b"highlight"),
            ..Default::default()
        },
    ];
    let strings = [pool.s(b"opt-a"), pool.s(b"opt-b"), pool.s(b"opt-b")];
    let form_fields = [LiteParseFormField {
        id: pool.s(b"f1"),
        field_type: pool.s(b"combobox"),
        page: 1,
        option_offset: 0,
        option_count: 2,
        selected_option_offset: 2,
        selected_option_count: 1,
        flags: LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED | LITEPARSE_FORM_FIELD_FLAG_CHECKED,
        ..Default::default()
    }];
    let attributes = [LiteParseStructureAttribute {
        name: pool.s(b"O"),
        string: pool.s(b"Layout"),
        kind: LITEPARSE_STRUCTURE_ATTR_STRING,
        number: 0.0,
    }];
    let mcids = [3i32, 4];
    // Document > [Sect > [P], P]
    let structure = [
        LiteParseStructureNode {
            element_type: pool.s(b"Document"),
            parent_index: LITEPARSE_NO_PARENT,
            ..Default::default()
        },
        LiteParseStructureNode {
            element_type: pool.s(b"Sect"),
            parent_index: 0,
            depth: 1,
            attribute_count: 1,
            annotation_offset: 1,
            annotation_count: 1,
            ..Default::default()
        },
        LiteParseStructureNode {
            element_type: pool.s(b"P"),
            parent_index: 1,
            depth: 2,
            mcid_count: 2,
            ..Default::default()
        },
        LiteParseStructureNode {
            element_type: pool.s(b"P"),
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
    pool.install(&mut content);
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = content.pages_len as u32;
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
    assert_eq!(
        pooled(view, page_annotations[0].uri),
        "https://example.invalid"
    );
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
            .map(|s| pooled(view, *s))
            .collect::<Vec<_>>(),
        ["opt-a", "opt-b"]
    );
    assert_eq!(
        pooled(
            view,
            range(out_strings, fields[0].selected_option_offset, 1)[0]
        ),
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
    let types: Vec<String> = nodes.iter().map(|n| pooled(view, n.element_type)).collect();
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
    assert_eq!(pooled(view, attribute.string), "Layout");
    assert_eq!(nodes[1].annotation_count, 1);
    assert_eq!(
        pooled(
            view,
            range(out_annotations, nodes[1].annotation_offset, 1)[0].subtype
        ),
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
        c.options |= LITEPARSE_FLAG_EXTRACT_BLOCKS;
    });
    let mut pool = TestPool::default();
    let items = [content_item(pool.s(b"Title"), 100.0)];
    let cells = [
        LiteParseLayoutCell {
            text: pool.s(b"wide"),
            colspan: 2,
            rowspan: 3,
            ..Default::default()
        },
        LiteParseLayoutCell {
            text: pool.s(b"b"),
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
            text: pool.s(b"Title"),
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
    pool.install(&mut content);
    content.pages = &page;
    content.pages_len = 1;
    content.total_pages = 1;
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
    assert_eq!(pooled(view, out_blocks[0].text), "Title");
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
    assert_eq!(pooled(view, out_cells[0].text), "wide");
    let markdown = pooled(view, parsed.outputs()[0].markdown);
    assert!(markdown.contains("## Title"), "markdown was: {markdown}");

    // max_pages truncates the page list and drops document-level blocks.
    let one = Parser::new(|c| c.max_pages = 1);
    let items = [
        content_item(pool.s(b"one"), 100.0),
        content_item(pool.s(b"two"), 100.0),
    ];
    let pages = [content_page(1, 0), content_page(2, 1)];
    let mut content = empty_content();
    pool.install(&mut content);
    content.pages = pages.as_ptr();
    content.pages_len = 2;
    content.total_pages = 2;
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
    assert_eq!(truncated.view().content.total_pages, 2);
    assert_eq!(truncated.pages().len(), 1);
}

#[test]
fn forms_and_metadata_pack_on_parse_and_extract() {
    let parser = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_EXTRACT_FORM_FIELDS | LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA;
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
        for field in fields {
            assert!(!pooled(view, field.id).is_empty());
            check_str(view, field.name);
            check_str(view, field.value);
        }
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
        c.options |= LITEPARSE_FLAG_EXTRACT_ANNOTATIONS;
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
            assert_eq!(pooled(view, annotation.subtype), "link");
            assert_eq!(pooled(view, annotation.uri), "https://example.invalid/twin");
            assert_ne!(annotation.flags & LITEPARSE_ANNOTATION_FLAG_HAS_RECT, 0);
        }
    }
}

#[test]
fn structure_tree_and_mcids_pack_with_absolute_parents() {
    let parser = Parser::new(|c| {
        c.output_format = LITEPARSE_OUTPUT_FORMAT_MARKDOWN;
        c.options |= LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE;
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
    assert!(struct_nodes.iter().any(|n| pooled(view, n.role) == "H1"));
    let mcids = arr(view.content.mcids, view.content.mcids_len);
    let h1 = struct_nodes
        .iter()
        .find(|n| pooled(view, n.role) == "H1")
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
    assert!(nodes.iter().any(|n| pooled(view, n.element_type) == "H1"));

    // Struct nodes re-enter parse_content for heading classification.
    let reparsed = parser.parse_content(&view.content).unwrap();
    assert!(
        reparsed.text().contains("# Hello"),
        "text was: {}",
        reparsed.text()
    );
}

#[test]
fn structure_mcids_stay_on_their_own_page_when_an_element_spans_pages() {
    let parser = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE;
    });
    let document = parser.open_bytes(&cross_page_structure_pdf());
    let staged = document.begin(&[]).finish().unwrap();
    for result in [document.extract(&[]), document.parse(&[]), staged] {
        assert_page_scoped_structure(&result);
    }
}

/// Each page of a [`cross_page_structure_pdf`] result carries only its own
/// MCIDs, in both the structure tree and the struct nodes, and the body's
/// page-1 box covers only its own content.
fn assert_page_scoped_structure(extracted: &Res) {
    let view = extracted.view();
    check_result_ranges(view);
    let mcids = arr(view.content.mcids, view.content.mcids_len);
    let structure_nodes = arr(
        view.content.structure_nodes,
        view.content.structure_nodes_len,
    );
    let struct_nodes = arr(view.content.struct_nodes, view.content.struct_nodes_len);
    let owned = |tag: LiteParseStr, offset: u32, count: u32| {
        (pooled(view, tag), range(mcids, offset, count).to_vec())
    };

    // Pre-order (tag, MCIDs) per page: the body is in both trees, with one id on each.
    let expected: [&[(&str, &[i32])]; 2] = [
        &[("Document", &[]), ("P", &[0]), ("LBody", &[1])],
        &[("Document", &[]), ("LBody", &[0]), ("P", &[1])],
    ];
    let pages = extracted.pages();
    assert_eq!(pages.len(), 2);
    for (page, expected) in pages.iter().zip(expected) {
        let expected = expected
            .iter()
            .map(|(tag, ids)| (tag.to_string(), ids.to_vec()))
            .collect::<Vec<_>>();
        let tree = range(
            structure_nodes,
            page.structure_node_offset,
            page.structure_node_count,
        )
        .iter()
        .map(|n| owned(n.element_type, n.mcid_offset, n.mcid_count))
        .collect::<Vec<_>>();
        assert_eq!(
            tree, expected,
            "structure tree of page {}",
            page.page_number
        );
        let hints = range(
            struct_nodes,
            page.struct_node_offset,
            page.struct_node_count,
        )
        .iter()
        .map(|n| owned(n.role, n.mcid_offset, n.mcid_count))
        .collect::<Vec<_>>();
        assert_eq!(hints, expected, "struct nodes of page {}", page.page_number);
    }

    // The body's page-1 box is its own line's: its page-2 MCID 0 names the
    // opening paragraph on page 1, and must not stretch the box up to it.
    let first = &pages[0];
    let body = range(
        struct_nodes,
        first.struct_node_offset,
        first.struct_node_count,
    )
    .iter()
    .find(|n| pooled(view, n.role) == "LBody")
    .unwrap();
    assert_ne!(body.flags & LITEPARSE_STRUCT_NODE_FLAG_HAS_BBOX, 0);
    let opening = range(
        arr(view.content.items, view.content.items_len),
        first.item_offset,
        first.item_count,
    )
    .iter()
    .find(|i| pooled(view, i.text).starts_with("Opening"))
    .unwrap();
    assert!(
        body.bbox.y > opening.y + opening.height,
        "body box {:?} reaches the opening line at y {}..{}",
        body.bbox,
        opening.y,
        opening.y + opening.height
    );
}

#[test]
fn page_objects_walk_path_form_and_image_without_extract_policy() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&objects_pdf());
    let page_objects = document
        .page_objects(0)
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()));
    let view = page_objects.view();
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
        image.image_raw.len == 0 && image.image_decoded.len == 0 && image.image_bitmap.len == 0
    );
}

#[test]
fn page_object_budget_fails_atomically_even_in_tolerant_mode() {
    let parser = Parser::new(|config| {
        config.options |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
    });
    let document = parser.open_bytes(&many_page_objects_pdf(LITEPARSE_MAX_OBJECTS + 1));
    let mut out = std::ptr::dangling_mut::<LiteParsePageObjects>();
    assert_eq!(
        unsafe { liteparse_document_page_objects(document.0, ptr::null(), 0, 0, &mut out) },
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    assert!(out.is_null());
    last_error_contains("count exceeds the operation limit");
}

#[test]
fn nested_form_object_budget_rejects_the_thirty_third_form() {
    let parser = Parser::plain();
    let within_limit =
        parser.open_bytes(&nested_form_objects_pdf(LITEPARSE_MAX_OBJECT_NESTING_DEPTH));
    within_limit.page_objects(0).unwrap();
    let document = parser.open_bytes(&nested_form_objects_pdf(
        LITEPARSE_MAX_OBJECT_NESTING_DEPTH + 1,
    ));
    assert_eq!(
        document.page_objects(0).unwrap_err(),
        LITEPARSE_STATUS_RESOURCE_LIMIT
    );
    last_error_contains("nesting exceeds");
}

#[test]
fn page_objects_image_payloads_follow_flags() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&objects_pdf());
    let flags = ALL_IMAGE_PAYLOADS;
    let page_objects = document.page_objects(flags).unwrap();
    let view = page_objects.view();
    let image = arr(view.objects, view.objects_len)
        .iter()
        .find(|o| o.kind == LITEPARSE_PAGE_OBJECT_IMAGE)
        .expect("image object");
    let raw = blob(view.arenas.binary, view.arenas.binary_len, image.image_raw);
    assert_eq!(raw, [0x00, 0xff, 0xff, 0x00]);
    assert_eq!(
        blob(
            view.arenas.binary,
            view.arenas.binary_len,
            image.image_decoded
        ),
        raw
    );
    assert_eq!(image.bitmap_format, LITEPARSE_BITMAP_FORMAT_GRAY);
    assert_eq!((image.bitmap_width, image.bitmap_height), (2, 2));
    let bitmap = blob(
        view.arenas.binary,
        view.arenas.binary_len,
        image.image_bitmap,
    );
    let stride = image.bitmap_stride as usize;
    assert_eq!(&bitmap[..2], &[0x00, 0xff]);
    assert_eq!(&bitmap[stride..stride + 2], &[0xff, 0x00]);
    drop(page_objects);

    assert_eq!(
        document.page_objects(1 << 10).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("unknown bits");
}

#[test]
fn page_objects_report_requested_image_payload_failures() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&image_with_unsupported_filter_pdf());
    let flags = ALL_IMAGE_PAYLOADS;
    let page_objects = document
        .page_objects(flags)
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()));
    let view = page_objects.view();
    let objects = arr(view.objects, view.objects_len);
    let image_index = objects
        .iter()
        .position(|o| o.kind == LITEPARSE_PAGE_OBJECT_IMAGE)
        .unwrap();
    let image = &objects[image_index];
    assert_ne!(
        image.flags & LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_BITMAP_UNAVAILABLE,
        0
    );
    assert_eq!(
        image.flags & LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_RAW_UNAVAILABLE,
        0
    );
    assert_eq!(
        image.flags & LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_DECODED_UNAVAILABLE,
        0
    );
    assert_eq!(
        blob(view.arenas.binary, view.arenas.binary_len, image.image_raw),
        [0x00, 0xff, 0xff, 0x00]
    );
    assert_eq!(
        blob(
            view.arenas.binary,
            view.arenas.binary_len,
            image.image_decoded
        ),
        [0x00, 0xff, 0xff, 0x00]
    );
    assert!(image.image_bitmap.len == 0);
}

#[test]
fn page_objects_report_empty_image_streams_inside_forms() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&empty_image_in_form_pdf());
    let flags = ALL_IMAGE_PAYLOADS;
    let page_objects = document
        .page_objects(flags)
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()));
    let view = page_objects.view();
    let objects = arr(view.objects, view.objects_len);
    let image_index = objects
        .iter()
        .position(|o| o.kind == LITEPARSE_PAGE_OBJECT_IMAGE)
        .unwrap();
    let image = &objects[image_index];
    assert_eq!(objects[0].kind, LITEPARSE_PAGE_OBJECT_FORM);
    assert_eq!(objects[0].child_offset as usize, image_index);
    assert_eq!(objects[0].flags & UNAVAILABLE_PAYLOADS, 0);
    assert_ne!(
        image.flags & LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_RAW_UNAVAILABLE,
        0
    );
    assert_ne!(
        image.flags & LITEPARSE_PAGE_OBJECT_FLAG_IMAGE_DECODED_UNAVAILABLE,
        0
    );
    assert_eq!(image.flags & UNAVAILABLE_PAYLOADS, UNAVAILABLE_PAYLOADS);
    drop(page_objects);

    let page_objects = document.page_objects(0).unwrap();
    let view = page_objects.view();
    assert!(
        arr(view.objects, view.objects_len)
            .iter()
            .all(|object| object.flags & UNAVAILABLE_PAYLOADS == 0)
    );
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
fn records_are_pointer_free_and_fixed_width() {
    // No `size_t` or pointers in text records, so their layout is the same
    // on every target and a copied-out array stays readable.
    assert_eq!(size_of::<LiteParseStr>(), 8);
    assert_eq!(align_of::<LiteParseStr>(), 4);
    assert_eq!(size_of::<LiteParseRect>(), 16);
    assert_eq!(size_of::<LiteParseLayoutRow>(), 8);
    assert_eq!(size_of::<LiteParseProjectedRegion>(), 32);
    assert_eq!(size_of::<LiteParsePageComplexity>(), 68);
    assert_eq!(
        size_of::<LiteParseTextItem>(),
        3 * 8 + 11 * 4 + 3 * 4 + 7 * 4
    );
    assert_eq!(align_of::<LiteParseTextItem>(), 4);
    assert_eq!(align_of::<LiteParsePage>(), 4);
    assert_eq!(align_of::<LiteParseProjectedLine>(), 4);
    assert_eq!(align_of::<LiteParseRawTextItem>(), 8, "f64 baseline_gap");
    // Input byte views are native-width; packed binary references are fixed-width.
    assert_eq!(size_of::<LiteParseByteView>(), 2 * size_of::<usize>());
    assert_eq!(size_of::<LiteParseBlobRef>(), 8);
    assert_eq!(align_of::<LiteParseImage>(), 4);
    assert_eq!(align_of::<LiteParseScreenshot>(), 4);
    // Every selector reports the size the compiler sees.
    assert_eq!(
        liteparse_sizeof(LITEPARSE_TYPE_TEXT_ITEM),
        size_of::<LiteParseTextItem>()
    );
    assert_eq!(
        liteparse_sizeof(LITEPARSE_TYPE_RESULT_VIEW),
        size_of::<LiteParseResultView>()
    );
    assert_eq!(
        liteparse_sizeof(LITEPARSE_TYPE_RAW_TEXT_VIEW),
        size_of::<LiteParseRawTextView>()
    );
}

#[test]
fn abi_descriptor_and_stage_defaults_match_compiled_layouts() {
    let mut descriptor = std::mem::MaybeUninit::<LiteParseAbiDescriptor>::uninit();
    unsafe { liteparse_abi_descriptor(descriptor.as_mut_ptr()) };
    let descriptor = unsafe { descriptor.assume_init() };
    assert_eq!(descriptor.pointer_width, usize::BITS);
    assert_eq!(
        descriptor.endianness,
        u32::from(cfg!(target_endian = "big"))
    );
    assert_eq!(
        descriptor.capabilities,
        if cfg!(feature = "tesseract") {
            LITEPARSE_CAPABILITY_TESSERACT
        } else {
            0
        }
    );
    assert_eq!(descriptor.max_selected_pages, LITEPARSE_MAX_SELECTED_PAGES);
    assert_eq!(
        descriptor.max_content_input_bytes,
        LITEPARSE_MAX_CONTENT_INPUT_BYTES
    );
    assert_eq!(
        descriptor.max_single_binary_bytes,
        LITEPARSE_MAX_SINGLE_BINARY_BYTES
    );
    assert_eq!(
        descriptor.max_content_binary_bytes,
        LITEPARSE_MAX_CONTENT_BINARY_BYTES
    );
    assert_eq!(
        descriptor.max_raster_pixels_per_page,
        LITEPARSE_MAX_RASTER_PIXELS_PER_PAGE
    );
    assert_eq!(
        descriptor.max_raster_bytes_per_operation,
        LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION
    );
    assert_eq!(descriptor.max_objects, LITEPARSE_MAX_OBJECTS);
    assert_eq!(
        descriptor.max_object_nesting_depth,
        LITEPARSE_MAX_OBJECT_NESTING_DEPTH
    );
    assert_eq!(
        liteparse_sizeof(LITEPARSE_TYPE_ABI_DESCRIPTOR),
        size_of::<LiteParseAbiDescriptor>()
    );
    assert_eq!(
        descriptor.layout_fingerprint,
        u64::from_str_radix(
            include_str!("../include/liteparse_abi.h")
                .split("LITEPARSE_ABI_FINGERPRINT 0x")
                .nth(1)
                .and_then(|rest| rest.split("ULL").next())
                .expect("published fingerprint"),
            16
        )
        .unwrap()
    );
}

#[test]
fn binary_arena_is_relocatable_after_free() {
    let parser = Parser::plain();
    let document = parser.open("sample.pdf");
    let shots = document.screenshot(&[], 72.0, None).unwrap();
    let view = shots.view();
    let binary = arr(view.arenas.binary, view.arenas.binary_len).to_vec();
    let records = shots.shots().to_vec();
    drop(shots);
    drop(document);
    drop(parser);
    assert_eq!(
        &range(&binary, records[0].png.offset, records[0].png.len)[..4],
        b"\x89PNG"
    );
}

#[test]
fn a_result_can_be_copied_out_and_read_after_the_handle_is_freed() {
    // The consumption pattern the pool exists for: bulk-copy the arrays and
    // the pool, free the handle, keep reading.
    let parser = Parser::plain();
    let parsed = parser.open("page_labels.pdf").parse(&[]);
    let view = parsed.view();
    let pool = view.pool().to_vec();
    let pages = parsed.pages().to_vec();
    let items = arr(view.content.items, view.content.items_len).to_vec();
    let expected = parsed.text();
    let text = view.text;
    drop(parsed);
    drop(parser);

    let read = |s: LiteParseStr| {
        let start = s.offset as usize;
        let end = start + s.len as usize;
        assert_eq!(pool[end], 0);
        std::str::from_utf8(&pool[start..end]).unwrap().to_owned()
    };
    assert_eq!(read(text), expected);
    assert_eq!(read(pages[1].label), "ii");
    let first = &items[pages[0].item_offset as usize];
    assert!(!read(first.text).is_empty());
    assert!(expected.contains(&read(first.text)));
    // Empty strings are `{0, 0}` and read as an empty C string.
    assert_eq!(pool[0], 0);
    assert_eq!(read(LiteParseStr::default()), "");
}

/// Two pages whose second `/Kids` entry is not a page dictionary, so PDFium
/// fails to load page 2.
fn broken_second_page_pdf() -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec(),
        b"42".to_vec(),
    ])
}

#[test]
fn tolerant_inspection_records_skipped_pages() {
    let tolerant = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR
            | LITEPARSE_FLAG_EXTRACT_SCREENSHOTS
            | LITEPARSE_FLAG_INCLUDE_COMPLEXITY
    });
    let document = tolerant.open_bytes(&broken_second_page_pdf());
    let check = |arenas: &LiteParseArenas| {
        let errors = arr(arenas.page_errors, arenas.page_errors_len);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].page_number, 2);
        assert!(!pooled(arenas, errors[0].message).is_empty());
    };

    let mut raw = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_raw_text(document.0, ptr::null(), 0, &mut raw) },
        LITEPARSE_STATUS_OK
    );
    let view = unsafe { liteparse_raw_text_view(raw).as_ref() }.unwrap();
    assert_eq!(view.pages_len, 1);
    check(&view.arenas);
    unsafe { liteparse_raw_text_free(raw) };

    let objects = document.page_objects(0).unwrap();
    assert_eq!(objects.view().pages_len, 1);
    check(&objects.view().arenas);

    let shots = document.screenshot(&[], 0.0, None).unwrap();
    assert_eq!(shots.view().screenshots_len, 1);
    check(&shots.view().arenas);

    let parsed = document.parse(&[]);
    assert_eq!(parsed.pages().len(), 1);
    check(&parsed.view().content.arenas);

    let mut complexity = ptr::null_mut();
    assert_eq!(
        unsafe { liteparse_document_complexity(document.0, ptr::null(), 0, &mut complexity) },
        LITEPARSE_STATUS_OK
    );
    let view = unsafe { liteparse_complexity_view(complexity).as_ref() }.unwrap();
    assert_eq!(view.pages_len, 1);
    check(&view.arenas);
    unsafe { liteparse_complexity_free(complexity) };

    let strict = Parser::plain().open_bytes(&broken_second_page_pdf());
    assert_eq!(
        strict.page_objects(0).unwrap_err(),
        LITEPARSE_STATUS_PARSE_ERROR
    );
}

// ---------------------------------------------------------------------------
// Zero-area pages

fn page_errors(view: &LiteParseArenas) -> Vec<(u32, String)> {
    arr(view.page_errors, view.page_errors_len)
        .iter()
        .map(|error| (error.page_number, pooled(view, error.message)))
        .collect()
}

#[test]
fn zero_area_pages_are_extracted_and_parsed_like_any_other() {
    let parser = Parser::plain();
    let document = parser.open_bytes(&zero_area_page_pdf());
    let parsed = document.parse(&[]);
    let extracted = document.extract(&[]);
    for result in [&parsed, &extracted] {
        check_result_ranges(result.view());
        let pages = result.pages();
        assert_eq!(
            pages
                .iter()
                .map(|page| page.page_number)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let empty = &pages[1];
        assert_eq!((empty.width, empty.height), (0.0, 0.0));
        assert_eq!(empty.item_count, 0);
        assert_ne!(empty.flags & LITEPARSE_PAGE_FLAG_HAS_GEOMETRY, 0);
        assert_eq!(result.view().content.arenas.page_errors_len, 0);
    }
    assert!(!parsed.text().contains("Hidden"));
    assert!(parsed.text().contains("Visible page three."));

    // What extraction reports passes back unchanged.
    let reparsed = parser
        .parse_content(&extracted.view().content)
        .unwrap_or_else(|status| panic!("{status}: {}", last_error()));
    assert_eq!(reparsed.pages().len(), 3);
    assert_eq!(reparsed.pages()[1].width, 0.0);
    assert_eq!(reparsed.text(), parsed.text());
}

#[test]
fn parse_content_accepts_zero_but_not_negative_page_extents() {
    let parser = Parser::plain();
    let mut page = content_page(1, 0);
    page.item_count = 0;
    page.flags = LITEPARSE_PAGE_FLAG_HAS_GEOMETRY;
    page.geometry.user_unit = 1.0;
    (page.width, page.height) = (0.0, 0.0);
    let mut content = empty_content();
    content.total_pages = 1;
    content.pages = &page;
    content.pages_len = 1;
    let parsed = parser.parse_content(&content).unwrap();
    assert_eq!(parsed.pages()[0].width, 0.0);

    let mut bad = page;
    bad.height = -1.0;
    content.pages = &bad;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("non-negative");

    let mut bad = page;
    bad.geometry.box_left = 1.0;
    content.pages = &bad;
    assert_eq!(
        parser.parse_content(&content).unwrap_err(),
        LITEPARSE_STATUS_INVALID_ARGUMENT
    );
    last_error_contains("geometry");
}

#[test]
fn zero_area_pages_cannot_be_rendered() {
    let strict = Parser::plain().open_bytes(&zero_area_page_pdf());
    assert_eq!(
        strict.screenshot(&[2], 0.0, None).unwrap_err(),
        LITEPARSE_STATUS_PARSE_ERROR
    );
    last_error_contains("page 2 has no visible area to render");

    let tolerant = Parser::new(|c| c.options |= LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR)
        .open_bytes(&zero_area_page_pdf());
    let shots = tolerant.screenshot(&[], 0.0, None).unwrap();
    assert_eq!(
        shots
            .shots()
            .iter()
            .map(|shot| shot.page_number)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        page_errors(&shots.view().arenas),
        [(2, "page 2 has no visible area to render".to_owned())]
    );

    let parser = Parser::new(|c| {
        c.options |= LITEPARSE_FLAG_EXTRACT_SCREENSHOTS | LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR;
    });
    let parsed = parser.open_bytes(&zero_area_page_pdf()).parse(&[]);
    let view = parsed.view();
    check_result_ranges(view);
    assert_eq!(parsed.pages().len(), 3);
    assert_eq!(
        arr(view.screenshots, view.screenshots_len)
            .iter()
            .map(|shot| shot.page_number)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        page_errors(&view.content.arenas),
        [(2, "page 2 has no visible area to render".to_owned())]
    );
}

#[test]
fn ocr_never_renders_zero_area_pages() {
    // One raster per round makes a round end, and the next start, around the
    // area-less page; the default fits the whole document in one round.
    for workers in [1, 0] {
        let parser = Parser::new(|c| {
            if workers > 0 {
                c.num_workers = workers;
            }
        });
        let document = parser.open_bytes(&zero_area_page_pdf());
        let (rendered, pages) = render_ocr_rounds(&document, &[]);
        // Too little text on either visible page to skip OCR.
        assert_eq!(rendered, [1, 3], "workers {workers}");
        assert_eq!(pages, 3);
        let (rendered, _) = render_ocr_rounds(&document, &[2]);
        assert!(rendered.is_empty(), "rendered {rendered:?}");
    }
}

/// Run every OCR round of a job over `document` restricted to `selection`,
/// recognising nothing; the rendered page numbers and the result's page count.
fn render_ocr_rounds(document: &Document, selection: &[u32]) -> (Vec<u32>, usize) {
    let mut job = document.begin(&[]);
    let mut rendered = Vec::new();
    loop {
        let view = job
            .render(selection, LITEPARSE_OCR_PIXEL_FORMAT_RGB)
            .unwrap();
        assert_eq!(view.arenas.page_errors_len, 0);
        if view.rasters_len == 0 {
            break;
        }
        rendered.extend(
            arr(view.rasters, view.rasters_len)
                .iter()
                .map(|r| r.page_number),
        );
        assert_eq!(job.merge(&Recognition::default()), LITEPARSE_STATUS_OK);
    }
    let result = job.finish().unwrap();
    assert_eq!(result.view().content.arenas.page_errors_len, 0);
    (rendered, result.pages().len())
}

// ---------------------------------------------------------------------------
// Form recovery signals

fn form_fields_of<'a>(result: &'a Res, page: &LiteParsePage) -> &'a [LiteParseFormField] {
    let view = result.view();
    range(
        arr(view.content.form_fields, view.content.form_fields_len),
        page.form_field_offset,
        page.form_field_count,
    )
}

#[test]
fn acroform_repair_is_reported_with_the_pages_it_made_readable() {
    let parser = Parser::new(|c| c.options |= LITEPARSE_FLAG_EXTRACT_FORM_FIELDS);
    let document = parser.open_bytes(&widget_on_second_page_pdf(false));
    let job = document.begin(&[]).finish().unwrap();
    for result in [document.parse(&[]), document.extract(&[]), job] {
        let view = result.view();
        check_result_ranges(view);
        assert_ne!(view.flags & LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM, 0);
        assert_eq!(
            arr(view.repaired_page_numbers, view.repaired_page_numbers_len),
            [2]
        );
        // The repaired copy is what extraction read: it has an AcroForm.
        assert_eq!(view.form_type, LITEPARSE_FORM_TYPE_ACRO_FORM);
        let fields = form_fields_of(&result, &result.pages()[1]);
        assert_eq!(fields.len(), 1);
        assert_eq!(pooled(view, fields[0].value), "Ada Lovelace");
    }

    // The repair is the document's, whatever the selection holds.
    let first = document.parse(&[1]);
    assert_ne!(
        first.view().flags & LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM,
        0
    );
    assert_eq!(first.view().repaired_page_numbers_len, 0);
}

#[test]
fn acroform_repair_is_not_reported_when_nothing_was_repaired() {
    let healthy = Parser::new(|c| c.options |= LITEPARSE_FLAG_EXTRACT_FORM_FIELDS)
        .open_bytes(&widget_on_second_page_pdf(true));
    // Without form-field extraction the orphaned widget is never repaired.
    let unread = Parser::plain().open_bytes(&widget_on_second_page_pdf(false));
    for document in [healthy, unread] {
        for result in [document.parse(&[]), document.extract(&[])] {
            let view = result.view();
            assert_eq!(view.flags & LITEPARSE_RESULT_FLAG_REPAIRED_ACROFORM, 0);
            assert_eq!(view.repaired_page_numbers_len, 0);
        }
    }
}

#[test]
fn flattened_form_widgets_are_reported_on_parse_and_extract() {
    let document = Parser::plain().open("filled_acroform.pdf");
    let parsed = document.parse(&[]);
    let extracted = document.extract(&[]);
    let flattened = |result: &Res| {
        let view = result.view();
        (
            view.flags & LITEPARSE_RESULT_FLAG_FLATTENED_FORM_WIDGETS != 0,
            arr(view.flattened_page_numbers, view.flattened_page_numbers_len).to_vec(),
        )
    };
    let (flagged, pages) = flattened(&parsed);
    assert!(flagged && !pages.is_empty());
    assert_eq!(flattened(&parsed), flattened(&extracted));
}
