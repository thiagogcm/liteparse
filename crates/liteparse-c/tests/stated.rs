//! What a document states about itself: page boxes, annotations, structure
//! elements, `/Info` entries and the XMP packet.

use std::ptr;

use liteparse_c::*;

// ---- helpers --------------------------------------------------------------------------------

fn assemble(objects: &[Vec<u8>]) -> Vec<u8> {
    assemble_with(objects, "/Root 1 0 R")
}

fn assemble_with(objects: &[Vec<u8>], trailer: &str) -> Vec<u8> {
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
            "trailer\n<< /Size {} {trailer} >>\nstartxref\n{xref}\n%%EOF",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

fn arr<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() {
        assert_eq!(len, 0, "null array with non-zero length");
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
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

fn pooled(pool: &[u8], value: LiteParseStr) -> Option<String> {
    if value.len == 0 {
        return None;
    }
    let start = value.offset as usize;
    let end = start + value.len as usize;
    assert!(
        end < pool.len(),
        "string {start}..{end} outside pool of {}",
        pool.len()
    );
    assert_eq!(pool[end], 0, "pooled strings are NUL-terminated");
    Some(String::from_utf8(pool[start..end].to_vec()).expect("utf-8"))
}

fn last_error() -> String {
    let mut out = LiteParseByteView::default();
    unsafe { liteparse_last_error(&mut out) };
    String::from_utf8(arr(out.ptr, out.len).to_vec()).expect("utf-8")
}

struct Parser(*mut LiteParseParser);

impl Parser {
    fn new(tweak: impl FnOnce(&mut LiteParseConfig)) -> Self {
        let mut config = std::mem::MaybeUninit::<LiteParseConfig>::uninit();
        unsafe { liteparse_config_init(config.as_mut_ptr()) };
        let mut config = unsafe { config.assume_init() };
        config.options &= !LITEPARSE_FLAG_OCR_ENABLED;
        tweak(&mut config);
        let mut handle = ptr::null_mut();
        let status = unsafe { liteparse_parser_new(&config, &mut handle) };
        assert_eq!(status, LITEPARSE_STATUS_OK, "{}", last_error());
        Self(handle)
    }

    fn open(&self, bytes: &[u8]) -> Document {
        let mut handle = ptr::null_mut();
        let status = unsafe {
            liteparse_document_open_bytes(self.0, bytes.as_ptr(), bytes.len(), &mut handle)
        };
        assert_eq!(status, LITEPARSE_STATUS_OK, "{}", last_error());
        Document(handle)
    }
}

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe { liteparse_parser_free(self.0) };
    }
}

fn open(bytes: &[u8]) -> (Parser, Document) {
    let parser = Parser::new(|_| {});
    let document = parser.open(bytes);
    (parser, document)
}

struct Document(*mut LiteParseDocument);

impl Document {
    fn try_stated(&self, pages: &[u32]) -> Result<Stated, LiteParseStatus> {
        let mut handle = ptr::null_mut();
        let status = unsafe {
            liteparse_document_stated_pages(self.0, pages.as_ptr(), pages.len(), &mut handle)
        };
        if status != LITEPARSE_STATUS_OK {
            assert!(handle.is_null());
            return Err(status);
        }
        Ok(Stated(handle))
    }

    fn stated(&self, pages: &[u32]) -> Stated {
        self.try_stated(pages)
            .unwrap_or_else(|status| panic!("{status}: {}", last_error()))
    }

    fn revisions(&self) -> &LiteParseStatedRevisions {
        let mut out = ptr::null();
        let status = unsafe { liteparse_document_stated_revisions(self.0, &mut out) };
        assert_eq!(status, LITEPARSE_STATUS_OK, "{}", last_error());
        unsafe { out.as_ref() }.expect("revisions")
    }

    /// The catalog's `/Lang` as the document's metadata states it.
    fn language(&self) -> Option<String> {
        let mut out = ptr::null();
        let status = unsafe { liteparse_document_metadata(self.0, &mut out) };
        assert_eq!(status, LITEPARSE_STATUS_OK, "{}", last_error());
        let metadata = unsafe { out.as_ref() }.expect("metadata");
        pooled(
            arr(metadata.pool, metadata.pool_len),
            metadata.meta.language,
        )
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        unsafe { liteparse_document_free(self.0) };
    }
}

struct Stated(*mut LiteParseStatedPages);

impl Stated {
    fn view(&self) -> &LiteParseStatedPagesView {
        unsafe { liteparse_stated_pages_view(self.0).as_ref() }.expect("view")
    }

    fn pool(&self) -> &[u8] {
        let arenas = &self.view().arenas;
        arr(arenas.pool, arenas.pool_len)
    }

    fn pages(&self) -> &[LiteParseStatedPage] {
        arr(self.view().pages, self.view().pages_len)
    }

    fn text(&self, value: LiteParseStr) -> Option<String> {
        pooled(self.pool(), value)
    }

    fn annotations(&self, page: usize) -> Vec<Annotation> {
        let view = self.view();
        let record = &self.pages()[page];
        let all = arr(view.annotations, view.annotations_len);
        let quads = arr(view.quads, view.quads_len);
        range(all, record.annotation_offset, record.annotation_count)
            .iter()
            .map(|a| Annotation {
                subtype: self.text(a.subtype).expect("subtype"),
                contents: self.text(a.contents),
                author: self.text(a.author),
                name: self.text(a.name),
                modified: self.text(a.modified),
                created: self.text(a.created),
                uri: self.text(a.uri),
                rect: (a.flags & LITEPARSE_STATED_ANNOTATION_FLAG_HAS_RECT != 0).then_some(a.rect),
                quads: range(quads, a.quad_offset, a.quad_count).to_vec(),
                pdf_flags: (a.flags & LITEPARSE_STATED_ANNOTATION_FLAG_HAS_PDF_FLAGS != 0)
                    .then_some(a.pdf_flags),
                target_page: a.target_page,
            })
            .collect()
    }

    fn elements(&self, page: usize) -> Vec<Element> {
        let view = self.view();
        let record = &self.pages()[page];
        let all = arr(view.elements, view.elements_len);
        let mcids = arr(view.mcids, view.mcids_len);
        range(all, record.element_offset, record.element_count)
            .iter()
            .map(|e| Element {
                element_type: self.text(e.element_type),
                id: self.text(e.id),
                lang: self.text(e.lang),
                alt: self.text(e.alt_text),
                actual: self.text(e.actual_text),
                parent: e.parent,
                mcids: range(mcids, e.mcid_offset, e.mcid_count).to_vec(),
            })
            .collect()
    }
}

impl Drop for Stated {
    fn drop(&mut self) {
        unsafe { liteparse_stated_pages_free(self.0) };
    }
}

#[derive(Debug)]
struct Annotation {
    subtype: String,
    contents: Option<String>,
    author: Option<String>,
    name: Option<String>,
    modified: Option<String>,
    created: Option<String>,
    uri: Option<String>,
    rect: Option<LiteParseUserRect>,
    quads: Vec<LiteParseUserQuad>,
    pdf_flags: Option<u32>,
    target_page: u32,
}

#[derive(Debug)]
struct Element {
    element_type: Option<String>,
    id: Option<String>,
    lang: Option<String>,
    alt: Option<String>,
    actual: Option<String>,
    parent: u32,
    mcids: Vec<i32>,
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> LiteParseUserRect {
    LiteParseUserRect { x0, y0, x1, y1 }
}

// ---- page boxes -----------------------------------------------------------------------------

fn boxes_pdf() -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        // The page tree states a media box and a crop box, which PDFium does not
        // read for a page that inherits them.
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 \
          /MediaBox [0 0 612 792] /CropBox [10 20 600 780] >>"
            .to_vec(),
        // 3: states nothing, inherits both.
        b"<< /Type /Page /Parent 2 0 R >>".to_vec(),
        // 4: its own media box and the three print boxes, a user unit; inherits the crop box.
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /BleedBox [5 5 295 395] \
          /TrimBox [10 10 290 390] /ArtBox [20 20 280 380] /UserUnit 2 >>"
            .to_vec(),
        // 5: corners in any order, a fractional crop box of its own.
        b"<< /Type /Page /Parent 2 0 R /MediaBox [612 792 0 0] /CropBox [100.5 200.25 400 700] >>"
            .to_vec(),
    ])
}

fn flags(page: &LiteParseStatedPage) -> [bool; 5] {
    [
        LITEPARSE_STATED_PAGE_FLAG_MEDIA_BOX,
        LITEPARSE_STATED_PAGE_FLAG_CROP_BOX,
        LITEPARSE_STATED_PAGE_FLAG_BLEED_BOX,
        LITEPARSE_STATED_PAGE_FLAG_TRIM_BOX,
        LITEPARSE_STATED_PAGE_FLAG_ART_BOX,
    ]
    .map(|bit| page.flags & bit != 0)
}

#[test]
fn page_boxes_are_the_ones_the_page_dictionary_states() {
    let (_parser, document) = open(&boxes_pdf());
    let stated = document.stated(&[]);
    let pages = stated.pages();
    assert_eq!(pages.len(), 3);

    assert_eq!(pages[0].page_number, 1);
    assert_eq!(flags(&pages[0]), [false; 5]);
    assert_eq!(pages[0].user_unit, 1.0);

    assert_eq!(flags(&pages[1]), [true, false, true, true, true]);
    assert_eq!(pages[1].media_box, rect(0.0, 0.0, 300.0, 400.0));
    assert_eq!(pages[1].bleed_box, rect(5.0, 5.0, 295.0, 395.0));
    assert_eq!(pages[1].trim_box, rect(10.0, 10.0, 290.0, 390.0));
    assert_eq!(pages[1].art_box, rect(20.0, 20.0, 280.0, 380.0));
    assert_eq!(pages[1].user_unit, 2.0);

    assert_eq!(flags(&pages[2]), [true, true, false, false, false]);
    assert_eq!(pages[2].media_box, rect(0.0, 0.0, 612.0, 792.0));
    assert_eq!(pages[2].crop_box, rect(100.5, 200.25, 400.0, 700.0));
}

#[test]
fn a_box_nobody_states_stays_absent() {
    let pdf = assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R >>".to_vec(),
    ]);
    let (_parser, document) = open(&pdf);
    let stated = document.stated(&[]);
    assert_eq!(flags(&stated.pages()[0]), [false; 5]);
}

#[test]
fn a_selection_states_its_pages_in_ascending_order() {
    let (_parser, document) = open(&boxes_pdf());
    let stated = document.stated(&[3, 1, 3]);
    let numbers: Vec<u32> = stated.pages().iter().map(|page| page.page_number).collect();
    assert_eq!(numbers, [1, 3]);
    assert_eq!(
        document.try_stated(&[4]).err(),
        Some(LITEPARSE_STATUS_INVALID_ARGUMENT)
    );
    let none = document.try_stated(&[]).map(|stated| stated.pages().len());
    assert_eq!(none, Ok(3));
}

// ---- annotations ----------------------------------------------------------------------------

fn annotations_pdf() -> Vec<u8> {
    let appearance = stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 100 20]",
        b"0 0 1 RG 0 0 100 20 re S",
    );
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 612 792] >>".to_vec(),
        // 3: page one, eleven /Annots entries, the last not a dictionary.
        b"<< /Type /Page /Parent 2 0 R /Annots [5 0 R 6 0 R 7 0 R 8 0 R 9 0 R 10 0 R 11 0 R \
          12 0 R 13 0 R 14 0 R 99] >>"
            .to_vec(),
        b"<< /Type /Page /Parent 2 0 R >>".to_vec(),
        // 5: a link to a URI, with a colour and flags.
        b"<< /Type /Annot /Subtype /Link /Rect [72 700 172 720] /F 4 /C [0 0 1] \
          /A << /S /URI /URI (https://example.com/a) >> >>"
            .to_vec(),
        // 6: a link with its own destination on page two.
        b"<< /Type /Annot /Subtype /Link /Rect [72 650 172 670] /Dest [4 0 R /Fit] >>".to_vec(),
        // 7: a link whose GoTo action leads to page two.
        b"<< /Type /Annot /Subtype /Link /Rect [172 600 72 620] \
          /A << /S /GoTo /D [4 0 R /XYZ 0 700 0] >> >>"
            .to_vec(),
        // 8: a highlight over two skewed quads.
        b"<< /Type /Annot /Subtype /Highlight /Rect [70 500 300 560] \
          /QuadPoints [72 540 200 550 72 520 200 530  100 510 240 520 100 500 240 510] \
          /C [1 1 0] /T (Ada) /NM (note-1) /M (D:20240102030405Z) \
          /CreationDate (D:20240101000000Z) /Contents (A remark) /F 28 /Popup 9 0 R >>"
            .to_vec(),
        // 9: its popup.
        b"<< /Type /Annot /Subtype /Popup /Rect [300 500 400 560] /Parent 8 0 R /Open true >>"
            .to_vec(),
        // 10: a square with an appearance stream and a one-component colour.
        b"<< /Type /Annot /Subtype /Square /Rect [72 400 172 420] /C [0.5] /AP << /N 15 0 R >> >>"
            .to_vec(),
        // 11: a note with an empty colour and no flags.
        b"<< /Type /Annot /Subtype /Text /Rect [72 300 90 318] /C [] /Contents (Note) >>".to_vec(),
        // 12: a subtype PDFium does not know.
        b"<< /Type /Annot /Subtype /Foo /Rect [72 200 90 218] >>".to_vec(),
        // 13: a widget.
        b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (field) /Rect [72 100 172 120] >>".to_vec(),
        // 14: a stamp with a four-component colour and no appearance stream.
        b"<< /Type /Annot /Subtype /Stamp /Rect [200 100 300 120] /C [0 0.5 0.25 0.1] >>".to_vec(),
        appearance,
    ])
}

#[test]
fn every_annotation_is_stated_in_array_order_with_its_subtype() {
    let (_parser, document) = open(&annotations_pdf());
    let stated = document.stated(&[]);
    let subtypes: Vec<String> = stated
        .annotations(0)
        .into_iter()
        .map(|annotation| annotation.subtype)
        .collect();
    // The entry that is not a dictionary states nothing; popups and widgets are annotations.
    assert_eq!(
        subtypes,
        [
            "Link",
            "Link",
            "Link",
            "Highlight",
            "Popup",
            "Square",
            "Text",
            "Foo",
            "Widget",
            "Stamp"
        ]
    );
    assert!(stated.annotations(1).is_empty());
}

#[test]
fn annotation_strings_flags_and_dates_are_as_written() {
    let (_parser, document) = open(&annotations_pdf());
    let stated = document.stated(&[]);
    let annotations = stated.annotations(0);

    let link = &annotations[0];
    assert_eq!(link.uri.as_deref(), Some("https://example.com/a"));
    assert_eq!(link.pdf_flags, Some(4));
    assert_eq!(link.rect, Some(rect(72.0, 700.0, 172.0, 720.0)));
    assert_eq!(link.contents, None);

    let highlight = &annotations[3];
    assert_eq!(highlight.contents.as_deref(), Some("A remark"));
    assert_eq!(highlight.author.as_deref(), Some("Ada"));
    assert_eq!(highlight.name.as_deref(), Some("note-1"));
    assert_eq!(highlight.modified.as_deref(), Some("D:20240102030405Z"));
    assert_eq!(highlight.created.as_deref(), Some("D:20240101000000Z"));
    assert_eq!(highlight.pdf_flags, Some(28));

    // A widget's /T is its field name, stated as written.
    assert_eq!(annotations[8].author.as_deref(), Some("field"));
    // No /F is no flags, which is not flags of zero.
    assert_eq!(annotations[6].pdf_flags, None);
    // The rectangle is normalised.
    assert_eq!(annotations[2].rect, Some(rect(72.0, 600.0, 172.0, 620.0)));
}

#[test]
fn quad_corners_are_the_attachment_points_in_stated_order() {
    let (_parser, document) = open(&annotations_pdf());
    let stated = document.stated(&[]);
    let annotations = stated.annotations(0);
    let quad = |c: [f32; 8]| LiteParseUserQuad {
        x1: c[0],
        y1: c[1],
        x2: c[2],
        y2: c[3],
        x3: c[4],
        y3: c[5],
        x4: c[6],
        y4: c[7],
    };
    assert_eq!(
        annotations[3].quads,
        [
            quad([72.0, 540.0, 200.0, 550.0, 72.0, 520.0, 200.0, 530.0]),
            quad([100.0, 510.0, 240.0, 520.0, 100.0, 500.0, 240.0, 510.0]),
        ]
    );
    assert!(annotations[0].quads.is_empty());
}

#[test]
fn a_link_states_the_page_it_jumps_to() {
    let (_parser, document) = open(&annotations_pdf());
    let stated = document.stated(&[]);
    let targets: Vec<u32> = stated
        .annotations(0)
        .iter()
        .take(3)
        .map(|link| link.target_page)
        .collect();
    // A URI jumps nowhere; a /Dest and a GoTo action reach page two.
    assert_eq!(targets, [0, 2, 2]);
}

// ---- structure ------------------------------------------------------------------------------

fn structure_pdf() -> Vec<u8> {
    structure_pdf_marked("true")
}

/// The structure document with `/MarkInfo /Marked` set to `marked`.
fn structure_pdf_marked(marked: &str) -> Vec<u8> {
    let content = stream(
        "",
        b"BT /F1 12 Tf 72 700 Td /Para << /MCID 0 >> BDC (Hello) Tj EMC ET \
          BT /F1 12 Tf 72 600 Td /P << /MCID 1 >> BDC (World) Tj EMC ET",
    );
    let second = stream(
        "",
        b"BT /F1 12 Tf 72 700 Td /P << /MCID 0 >> BDC (Elsewhere) Tj EMC ET",
    );
    assemble(&[
        format!(
            "<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked {marked} >> \
             /StructTreeRoot 6 0 R /Lang (en) >>"
        )
        .into_bytes(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R \
          /Resources << /Font << /F1 7 0 R >> >> /StructParents 0 >>"
            .to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 12 0 R \
          /Resources << /Font << /F1 7 0 R >> >> /StructParents 1 >>"
            .to_vec(),
        content,
        b"<< /Type /StructTreeRoot /K [8 0 R] /ParentTree 11 0 R /RoleMap << /Para /P >> >>"
            .to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        b"<< /Type /StructElem /S /Document /P 6 0 R /Lang (en-US) /K [9 0 R 10 0 R 13 0 R] >>"
            .to_vec(),
        b"<< /Type /StructElem /S /Para /P 8 0 R /Pg 3 0 R /K 0 /ID (p1) /Alt (First) \
          /Lang (fr) >>"
            .to_vec(),
        b"<< /Type /StructElem /S /P /P 8 0 R /Pg 3 0 R /K 1 /ActualText (Second) >>".to_vec(),
        b"<< /Nums [0 [9 0 R 10 0 R] 1 [13 0 R]] >>".to_vec(),
        second,
        b"<< /Type /StructElem /S /P /P 8 0 R /Pg 4 0 R /K 0 >>".to_vec(),
    ])
}

#[test]
fn structure_elements_are_numbered_in_pre_order_per_page() {
    let (_parser, document) = open(&structure_pdf());
    let stated = document.stated(&[]);

    let first = stated.elements(0);
    let summary: Vec<(Option<&str>, u32, &[i32])> = first
        .iter()
        .map(|element| {
            (
                element.element_type.as_deref(),
                element.parent,
                element.mcids.as_slice(),
            )
        })
        .collect();
    // The type is the role the role map resolves /Para to.
    assert_eq!(
        summary,
        [
            (Some("Document"), 0, &[][..]),
            (Some("P"), 1, &[0][..]),
            (Some("P"), 1, &[1][..]),
        ]
    );

    // Page two's tree holds the document and the one paragraph on it; ordinals restart.
    let second = stated.elements(1);
    let summary: Vec<(Option<&str>, u32, &[i32])> = second
        .iter()
        .map(|element| {
            (
                element.element_type.as_deref(),
                element.parent,
                element.mcids.as_slice(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [(Some("Document"), 0, &[][..]), (Some("P"), 1, &[0][..])]
    );
}

#[test]
fn structure_language_id_and_text_are_the_elements_own() {
    let (_parser, document) = open(&structure_pdf());
    let elements = document.stated(&[1]).elements(0);
    assert_eq!(elements[0].lang.as_deref(), Some("en-US"));
    assert_eq!(elements[1].lang.as_deref(), Some("fr"));
    assert_eq!(elements[1].id.as_deref(), Some("p1"));
    assert_eq!(elements[1].alt.as_deref(), Some("First"));
    // /Lang is not inherited: the element states none of its own.
    assert_eq!(elements[2].lang, None);
    assert_eq!(elements[2].actual.as_deref(), Some("Second"));
    assert_eq!(elements[2].alt, None);
}

#[test]
fn a_tree_pdfium_reads_is_not_flagged_unread() {
    let (_parser, document) = open(&structure_pdf());
    let stated = document.stated(&[]);
    for page in stated.pages() {
        assert_eq!(page.flags & LITEPARSE_STATED_PAGE_FLAG_STRUCTURE_UNREAD, 0);
    }
}

#[test]
fn a_tree_in_a_document_not_marked_tagged_is_flagged_unread() {
    let (_parser, document) = open(&structure_pdf_marked("false"));
    let stated = document.stated(&[]);
    for (index, page) in stated.pages().iter().enumerate() {
        assert_ne!(page.flags & LITEPARSE_STATED_PAGE_FLAG_STRUCTURE_UNREAD, 0);
        assert!(stated.elements(index).is_empty());
    }
}

#[test]
fn an_untagged_document_is_flagged_unread_too() {
    // PDFium cannot tell a document with no tree from one whose catalog does
    // not mark its tree.
    let (_parser, document) = open(&boxes_pdf());
    let stated = document.stated(&[]);
    assert_ne!(
        stated.pages()[0].flags & LITEPARSE_STATED_PAGE_FLAG_STRUCTURE_UNREAD,
        0
    );
    assert!(stated.elements(0).is_empty());
}

// ---- view consistency -----------------------------------------------------------------------

#[test]
fn every_range_of_a_view_lies_inside_its_array() {
    for pdf in [annotations_pdf(), structure_pdf(), boxes_pdf()] {
        let (_parser, document) = open(&pdf);
        let stated = document.stated(&[]);
        let view = stated.view();
        for (index, page) in stated.pages().iter().enumerate() {
            range(
                arr(view.annotations, view.annotations_len),
                page.annotation_offset,
                page.annotation_count,
            );
            range(
                arr(view.elements, view.elements_len),
                page.element_offset,
                page.element_count,
            );
            stated.annotations(index);
            stated.elements(index);
        }
        for annotation in arr(view.annotations, view.annotations_len) {
            range(
                arr(view.quads, view.quads_len),
                annotation.quad_offset,
                annotation.quad_count,
            );
        }
    }
}

#[test]
fn null_handles_degrade_to_null_views() {
    assert!(unsafe { liteparse_stated_pages_view(ptr::null()) }.is_null());
    unsafe { liteparse_stated_pages_free(ptr::null_mut()) };
    let mut out = ptr::null_mut();
    let status = unsafe { liteparse_document_stated_pages(ptr::null(), ptr::null(), 0, &mut out) };
    assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    assert!(out.is_null());
}

// ---- language -------------------------------------------------------------------------------

#[test]
fn the_metadata_states_the_catalog_language_as_pdfium_reads_it() {
    let pdf = assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R /Lang (pt-BR) >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec(),
    ]);
    let (_parser, document) = open(&pdf);
    assert_eq!(document.language().as_deref(), Some("pt-BR"));
    let (_parser, untagged) = open(&boxes_pdf());
    assert_eq!(untagged.language(), None);
}

// ---- revisions ------------------------------------------------------------------------------

/// A one-page file with a signature field, then an incremental update that adds a text
/// annotation, each closed by its own trailer: the first revision's length and the whole file's.
fn revised_pdf() -> (Vec<u8>, usize) {
    let base = assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] /SigFlags 3 >> >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>".to_vec(),
        b"<< /FT /Sig /T (Signature1) /V 5 0 R >>".to_vec(),
        b"<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached \
          /ByteRange [0 10 20 30] /M (D:20240102030405Z) /Contents <00> >>"
            .to_vec(),
    ]);
    let first = base.len();
    let xref = base
        .windows(4)
        .rposition(|window| window == b"xref")
        .expect("the base has a table");
    let mut pdf = base.clone();
    pdf.extend_from_slice(b"\n");
    let note = pdf.len();
    pdf.extend_from_slice(
        b"6 0 obj\n<< /Type /Annot /Subtype /Text /Rect [10 10 20 20] >>\nendobj\n",
    );
    let table = pdf.len();
    pdf.extend_from_slice(
        format!(
            "xref\n6 1\n{note:010} 00000 n \ntrailer\n<< /Size 7 /Root 1 0 R /Prev {xref} >>\nstartxref\n{table}\n%%EOF\n"
        )
        .as_bytes(),
    );
    // The first revision ends after its marker and the line end the update begins with.
    (pdf, first + 1)
}

#[test]
fn the_ends_of_the_trailers_and_the_signatures_are_stated() {
    let (pdf, first) = revised_pdf();
    let (_parser, document) = open(&pdf);
    let revisions = document.revisions();
    assert_ne!(revisions.flags & LITEPARSE_STATED_REVISIONS_FLAG_ENDS, 0);
    assert_eq!(
        arr(revisions.ends, revisions.ends_len),
        &[first as u64, pdf.len() as u64]
    );
    let pool = arr(revisions.pool, revisions.pool_len);
    let signatures = arr(revisions.signatures, revisions.signatures_len);
    assert_eq!(signatures.len(), 1);
    let signature = signatures[0];
    assert_eq!(
        pooled(pool, signature.sub_filter).as_deref(),
        Some("adbe.pkcs7.detached")
    );
    assert_eq!(
        pooled(pool, signature.time).as_deref(),
        Some("D:20240102030405Z")
    );
    let ranges = arr(revisions.ranges, revisions.ranges_len);
    assert_eq!(
        range(ranges, signature.range_offset, signature.range_count),
        &[0, 10, 20, 30]
    );
}

#[test]
fn an_unsigned_file_written_once_has_one_end_and_no_signature() {
    let mut pdf = boxes_pdf();
    pdf.push(b'\n');
    let (_parser, document) = open(&pdf);
    let revisions = document.revisions();
    assert_eq!(arr(revisions.ends, revisions.ends_len), &[pdf.len() as u64]);
    assert_eq!(revisions.signatures_len, 0);
}

#[test]
fn a_last_marker_without_a_line_end_states_no_end() {
    let pdf = boxes_pdf();
    let (_parser, document) = open(&pdf);
    let revisions = document.revisions();
    assert_eq!(revisions.ends_len, 0);
}
