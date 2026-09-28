//! Page-scoped marked-content ids for extracted structure.
//!
//! An MCID names a marked-content sequence only within one page's content
//! stream; a structure kid's page is its own `/Pg`, else its element's. The
//! tree PDFium builds for a page holds every element with content on the
//! page plus their ancestors, so an element whose content spans pages sits in
//! the tree of each. Core extraction reads an element's ids from `/K` without
//! the kids' pages, so on every such page it claims all of them, numbers
//! another element owns there included, and the struct node's box unions
//! that other content. `FPDF_StructElement_GetChildMarkedContentID` reads
//! only the kids PDFium resolved to the tree's page, so the ids are read
//! again through it and written over the core's, before anything packs or
//! projects them.

use liteparse::extract::ExtractedPages;
use liteparse::types::{Page as ExtractedPage, Rect, StructureTreeElement};
use liteparse_pdfium::{Document, Page, RectF, collect_mcid_bboxes, pdfium_sys, union_mcid_bboxes};

use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR};

/// One element of a page's structure tree, in the pre-order the core walks.
struct ScopedElement {
    /// The element's role, as extraction reports it.
    role: String,
    /// The ids of the element's content on the tree's page, in `/K` order.
    ids: Vec<i32>,
    /// How many of its children are elements of the tree.
    children: usize,
}

/// Replace the marked-content ids of every extracted page's struct nodes and
/// structure tree with the page's own, and each struct node's box with the
/// union of what those ids mark.
///
/// `document` is the one `extracted` came from. Extraction read structure
/// before flattening a page's form widgets into it, and flattening rewrites
/// the page in place, so flattened pages are read from `reopen`, a fresh
/// copy of the same input, opened at most once.
pub(crate) fn scope_marked_content_ids<'lib>(
    document: &Document<'lib>,
    extracted: &mut ExtractedPages,
    reopen: impl FnOnce() -> FfiResult<Document<'lib>>,
) -> FfiResult {
    let ExtractedPages {
        pages,
        flattened_page_numbers,
        ..
    } = extracted;
    let mut reopen = Some(reopen);
    let mut pristine = None;
    for page in pages {
        let tagged = !page.struct_nodes.is_empty()
            || page
                .structure_tree
                .as_ref()
                .is_some_and(|tree| !tree.roots.is_empty());
        if !tagged {
            continue;
        }
        let source = if flattened_page_numbers.contains(&(page.page_number as u32)) {
            if pristine.is_none() {
                let open = reopen.take().expect("a copy is opened at most once");
                pristine = Some(open()?);
            }
            pristine.as_ref().expect("opened above")
        } else {
            document
        };
        scope_page(source, page)?;
    }
    Ok(())
}

fn scope_page(document: &Document<'_>, page: &mut ExtractedPage) -> FfiResult {
    let number = page.page_number;
    let mismatch = || {
        FfiError::new(
            LITEPARSE_STATUS_PARSE_ERROR,
            format!("page {number}: the structure tree read differently than at extraction"),
        )
    };
    let pdf_page = document.page(number as i32 - 1)?;
    let elements = scoped_elements(&pdf_page);

    // The walks must meet node for node: same count, same roles in the same
    // order, same shape. Anything else is not the tree extraction read, and
    // writing ids by position onto it would be a guess.
    if page.struct_nodes.len() != elements.len()
        || page
            .struct_nodes
            .iter()
            .zip(&elements)
            .any(|(node, element)| node.role != element.role)
    {
        return Err(mismatch());
    }
    if !elements.is_empty() {
        // The view box extraction measured the page's struct nodes in.
        let view_box = pdf_page.view_box().unwrap_or(RectF {
            left: 0.0,
            top: pdf_page.height(),
            right: pdf_page.width(),
            bottom: 0.0,
        });
        let boxes = collect_mcid_bboxes(&pdf_page, &view_box);
        for (node, element) in page.struct_nodes.iter_mut().zip(&elements) {
            node.bbox = union_mcid_bboxes(&element.ids, &boxes).map(|b| Rect {
                x: b.left,
                y: b.top,
                width: b.right - b.left,
                height: b.bottom - b.top,
            });
            node.mcids.clone_from(&element.ids);
        }
    }

    if let Some(tree) = &mut page.structure_tree {
        let mut elements = elements.iter();
        for root in &mut tree.roots {
            scope_element(root, &mut elements).ok_or_else(mismatch)?;
        }
        if elements.next().is_some() {
            return Err(mismatch());
        }
    }
    Ok(())
}

/// Write `element`'s and its descendants' ids from `scoped`, which walks the
/// same tree in the same pre-order; `None` when the two disagree in shape.
fn scope_element<'a>(
    element: &mut StructureTreeElement,
    scoped: &mut impl Iterator<Item = &'a ScopedElement>,
) -> Option<()> {
    let own = scoped.next()?;
    if own.role != element.element_type || own.children != element.children.len() {
        return None;
    }
    element.marked_content_ids.clone_from(&own.ids);
    for child in &mut element.children {
        scope_element(child, scoped)?;
    }
    Some(())
}

/// Every element of `page`'s structure tree in pre-order, skipping the
/// children PDFium did not load for the page, as the core's walks do.
fn scoped_elements(page: &Page<'_, '_>) -> Vec<ScopedElement> {
    let pdfium = pdfium_sys::dynamic::pdfium();
    // SAFETY: `page` is a live page loaded under the PDFium lock its
    // lifetime carries; the tree is closed before returning, and elements
    // are only used while it is open.
    unsafe {
        let tree = (pdfium.FPDF_StructTree_GetForPage)(page.handle());
        if tree.is_null() {
            return Vec::new();
        }
        let mut elements = Vec::new();
        for index in 0..(pdfium.FPDF_StructTree_CountChildren)(tree) {
            let root = (pdfium.FPDF_StructTree_GetChildAtIndex)(tree, index);
            if !root.is_null() {
                walk(pdfium, root, &mut elements);
            }
        }
        (pdfium.FPDF_StructTree_Close)(tree);
        elements
    }
}

/// # Safety
///
/// `element` must belong to an open structure tree.
unsafe fn walk(
    pdfium: &pdfium_sys::dynamic::PdfiumBindings,
    element: pdfium_sys::FPDF_STRUCTELEMENT,
    out: &mut Vec<ScopedElement>,
) {
    let mut ids = Vec::new();
    let mut children = Vec::new();
    // SAFETY: the caller guarantees `element` is live.
    let role = unsafe { element_type(pdfium, element) };
    unsafe {
        for index in 0..(pdfium.FPDF_StructElement_CountChildren)(element) {
            let id = (pdfium.FPDF_StructElement_GetChildMarkedContentID)(element, index);
            if id >= 0 {
                ids.push(id);
            }
            let child = (pdfium.FPDF_StructElement_GetChildAtIndex)(element, index);
            if !child.is_null() {
                children.push(child);
            }
        }
    }
    out.push(ScopedElement {
        role,
        ids,
        children: children.len(),
    });
    for child in children {
        // SAFETY: a child of a live element of the same open tree.
        unsafe { walk(pdfium, child, out) };
    }
}

/// `element`'s role as `FPDF_StructElement_GetType` spells it, decoded the
/// way the core decodes it: UTF-16LE, trailing NULs dropped.
///
/// # Safety
///
/// `element` must belong to an open structure tree.
unsafe fn element_type(
    pdfium: &pdfium_sys::dynamic::PdfiumBindings,
    element: pdfium_sys::FPDF_STRUCTELEMENT,
) -> String {
    // SAFETY: the caller guarantees `element` is live. A null buffer asks
    // only for the length.
    let needed =
        unsafe { (pdfium.FPDF_StructElement_GetType)(element, std::ptr::null_mut(), 0) } as usize;
    if needed < 2 {
        return String::new();
    }
    let mut buffer = vec![0u16; needed / 2];
    // SAFETY: as above, and `buffer` holds the `needed` bytes PDFium asked for.
    let written = unsafe {
        (pdfium.FPDF_StructElement_GetType)(element, buffer.as_mut_ptr().cast(), needed as _)
    } as usize;
    if written < 2 {
        return String::new();
    }
    let mut end = (written / 2).min(buffer.len());
    while end > 0 && buffer[end - 1] == 0 {
        end -= 1;
    }
    String::from_utf16_lossy(&buffer[..end])
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeSet;

    use liteparse::stages::{self, ExtractRequest, ExtractionOutputOptions};
    use liteparse::types::PdfInput;
    use liteparse_pdfium::Library;

    use super::*;

    fn assemble(objects: &[&[u8]]) -> Vec<u8> {
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

    fn stream(data: &[u8]) -> Vec<u8> {
        let mut out = format!("<< /Length {} >>\nstream\n", data.len()).into_bytes();
        out.extend_from_slice(data);
        out.extend_from_slice(b"\nendstream");
        out
    }

    /// Two tagged pages drawing MCIDs 0 and 1 each, joined to a list body
    /// that owns page 1's MCID 1 and page 2's MCID 0, and parents the
    /// paragraph owning page 2's MCID 1. Page 1 holds a filled text field,
    /// so extraction flattens it; page 2 is cropped and turned a quarter, so
    /// its view box is not its media box.
    fn fixture() -> Vec<u8> {
        let page1 = stream(
            b"BT /F1 12 Tf 72 700 Td /P << /MCID 0 >> BDC (Opening paragraph.) Tj EMC ET\n\
              BT /F1 12 Tf 72 600 Td /LBody << /MCID 1 >> BDC (Body starts here.) Tj EMC ET",
        );
        let page2 = stream(
            b"BT /F1 12 Tf 72 700 Td /LBody << /MCID 0 >> BDC (Body ends here.) Tj EMC ET\n\
              BT /F1 12 Tf 90 600 Td /P << /MCID 1 >> BDC (Nested paragraph.) Tj EMC ET",
        );
        // The widget's own appearance paints its value, which is what makes
        // extraction flatten the page to read it.
        let appearance = {
            let content = b"/Tx BMC BT /Helv 12 Tf 2 5 Td (Ada Lovelace) Tj ET EMC";
            let mut out = format!(
                "<< /Type /XObject /Subtype /Form /BBox [0 0 200 20] \
                 /Resources << /Font << /Helv 12 0 R >> >> /Length {} >>\nstream\n",
                content.len()
            )
            .into_bytes();
            out.extend_from_slice(content);
            out.extend_from_slice(b"\nendstream");
            out
        };
        assemble(&[
            b"<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 7 0 R \
               /AcroForm << /Fields [14 0 R] /NeedAppearances true \
               /DR << /Font << /Helv 12 0 R >> >> /DA (/Helv 12 Tf 0 g) >> >>",
            b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R \
               /Resources << /Font << /F1 12 0 R >> >> /StructParents 0 /Annots [14 0 R] >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /CropBox [36 300 500 760] \
               /Rotate 90 /Contents 6 0 R /Resources << /Font << /F1 12 0 R >> >> \
               /StructParents 1 >>",
            &page1,
            &page2,
            b"<< /Type /StructTreeRoot /K [8 0 R] /ParentTree 13 0 R >>",
            b"<< /Type /StructElem /S /Document /P 7 0 R /K [9 0 R 10 0 R] >>",
            b"<< /Type /StructElem /S /P /P 8 0 R /Pg 3 0 R /K 0 >>",
            b"<< /Type /StructElem /S /LBody /P 8 0 R /Pg 3 0 R \
               /K [1 << /Type /MCR /Pg 4 0 R /MCID 0 >> 11 0 R] >>",
            b"<< /Type /StructElem /S /P /P 10 0 R /Pg 4 0 R /K 1 >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            b"<< /Nums [0 [9 0 R 10 0 R] 1 [10 0 R 11 0 R]] >>",
            b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /V (Ada Lovelace) \
               /DA (/Helv 12 Tf 0 g) /Rect [300 400 500 420] /F 4 /P 3 0 R /AP << /N 15 0 R >> >>",
            &appearance,
        ])
    }

    /// One tagged page, every element's content on it: a heading, a
    /// two-item list three levels deep and a closing paragraph, so the core
    /// reads every id right and the walks must meet node for node.
    fn nested_fixture() -> Vec<u8> {
        let content = stream(
            b"BT /F1 18 Tf 72 720 Td /H1 << /MCID 0 >> BDC (Title) Tj EMC ET\n\
              BT /F1 12 Tf 72 680 Td /Lbl << /MCID 1 >> BDC (1.) Tj EMC ET\n\
              BT /F1 12 Tf 92 680 Td /LBody << /MCID 2 >> BDC (First item) Tj EMC ET\n\
              BT /F1 12 Tf 72 660 Td /Lbl << /MCID 3 >> BDC (2.) Tj EMC ET\n\
              BT /F1 12 Tf 92 660 Td /LBody << /MCID 4 >> BDC (Second item) Tj EMC ET\n\
              BT /F1 12 Tf 72 620 Td /P << /MCID 5 >> BDC (Closing) Tj EMC ET",
        );
        assemble(&[
            b"<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 5 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
               /Resources << /Font << /F1 6 0 R >> >> /StructParents 0 >>",
            &content,
            b"<< /Type /StructTreeRoot /K [7 0 R] /ParentTree 18 0 R >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            b"<< /Type /StructElem /S /Document /P 5 0 R /K [8 0 R] >>",
            b"<< /Type /StructElem /S /Sect /P 7 0 R /Pg 3 0 R /K [9 0 R 10 0 R 17 0 R] >>",
            b"<< /Type /StructElem /S /H1 /P 8 0 R /Pg 3 0 R /K 0 >>",
            b"<< /Type /StructElem /S /L /P 8 0 R /Pg 3 0 R /K [11 0 R 12 0 R] >>",
            b"<< /Type /StructElem /S /LI /P 10 0 R /Pg 3 0 R /K [13 0 R 14 0 R] >>",
            b"<< /Type /StructElem /S /LI /P 10 0 R /Pg 3 0 R /K [15 0 R 16 0 R] >>",
            b"<< /Type /StructElem /S /Lbl /P 11 0 R /Pg 3 0 R /K 1 >>",
            b"<< /Type /StructElem /S /LBody /P 11 0 R /Pg 3 0 R /K 2 >>",
            b"<< /Type /StructElem /S /Lbl /P 12 0 R /Pg 3 0 R /K 3 >>",
            b"<< /Type /StructElem /S /LBody /P 12 0 R /Pg 3 0 R /K 4 >>",
            b"<< /Type /StructElem /S /P /P 8 0 R /Pg 3 0 R /K 5 >>",
            b"<< /Nums [0 [9 0 R 13 0 R 14 0 R 15 0 R 16 0 R 17 0 R]] >>",
        ])
    }

    /// Extract `pdf` with its structure tree, and scope the result.
    fn extract_and_scope(
        pdf: Vec<u8>,
        tamper: impl FnOnce(&mut ExtractedPages),
    ) -> (ExtractedPages, FfiResult<ExtractedPages>) {
        let lib = Library::try_init().unwrap();
        let input = PdfInput::Bytes(pdf);
        let document = stages::open(&lib, &input, None, &[]).unwrap();
        let core = stages::extract(&document, &request()).unwrap();
        let mut scoped = core.clone();
        tamper(&mut scoped);
        let outcome = scope_marked_content_ids(&document, &mut scoped, || {
            Ok(stages::open(&lib, &input, None, &[]).unwrap())
        })
        .map(|()| scoped);
        (core, outcome)
    }

    #[test]
    fn the_walk_meets_extraction_node_for_node() {
        let (core, scoped) = extract_and_scope(nested_fixture(), |_| {});
        let scoped = scoped.unwrap();
        let (before, after) = (&core.pages[0], &scoped.pages[0]);
        let roles = |page: &ExtractedPage| {
            page.struct_nodes
                .iter()
                .map(|node| (node.role.clone(), node.mcids.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(roles(after), roles(before));
        assert_eq!(
            roles(after)
                .iter()
                .map(|(role, _)| role.as_str())
                .collect::<Vec<_>>(),
            [
                "Document", "Sect", "H1", "L", "LI", "Lbl", "LBody", "LI", "Lbl", "LBody", "P"
            ]
        );
        let bits = |page: &ExtractedPage| {
            page.struct_nodes
                .iter()
                .map(|node| {
                    node.bbox
                        .as_ref()
                        .map(|r| [r.x, r.y, r.width, r.height].map(f32::to_bits))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(after), bits(before));
        let (mut tree_before, mut tree_after) = (Vec::new(), Vec::new());
        tree_ids(
            &before.structure_tree.as_ref().unwrap().roots,
            &mut tree_before,
        );
        tree_ids(
            &after.structure_tree.as_ref().unwrap().roots,
            &mut tree_after,
        );
        assert_eq!(tree_after, tree_before);
    }

    #[test]
    fn a_walk_that_disagrees_with_extraction_is_an_error() {
        type Tamper = fn(&mut ExtractedPages);
        let tampers: [(&str, Tamper); 5] = [
            ("a struct node fewer", |pages| {
                pages.pages[0].struct_nodes.pop();
            }),
            ("struct nodes out of order", |pages| {
                pages.pages[0].struct_nodes.swap(2, 3);
            }),
            ("a tree element fewer", |pages| {
                let tree = pages.pages[0].structure_tree.as_mut().unwrap();
                tree.roots[0].children[0].children.pop();
            }),
            ("tree children out of order", |pages| {
                let tree = pages.pages[0].structure_tree.as_mut().unwrap();
                tree.roots[0].children[0].children.swap(0, 1);
            }),
            ("a tree root more", |pages| {
                let tree = pages.pages[0].structure_tree.as_mut().unwrap();
                let extra = tree.roots[0].clone();
                tree.roots.push(extra);
            }),
        ];
        for (case, tamper) in tampers {
            let (_, outcome) = extract_and_scope(nested_fixture(), tamper);
            let error = outcome
                .err()
                .unwrap_or_else(|| panic!("{case}: scoped anyway"));
            assert_eq!(error.status, LITEPARSE_STATUS_PARSE_ERROR, "{case}");
            assert!(
                error
                    .message
                    .contains("read differently than at extraction"),
                "{case}: {}",
                error.message
            );
        }
    }

    fn request() -> ExtractRequest<'static> {
        ExtractRequest {
            output: ExtractionOutputOptions {
                extract_structure_tree: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn tree_ids(elements: &[StructureTreeElement], out: &mut Vec<Vec<i32>>) {
        for element in elements {
            out.push(element.marked_content_ids.clone());
            tree_ids(&element.children, out);
        }
    }

    #[test]
    fn ids_are_the_pages_own_and_boxes_follow_the_cores_math() {
        let lib = Library::try_init().unwrap();
        let input = PdfInput::Bytes(fixture());
        let document = stages::open(&lib, &input, None, &[]).unwrap();
        let core = stages::extract(&document, &request()).unwrap();
        assert_eq!(core.flattened_page_numbers, [1], "page 1 is flattened");

        let mut scoped = core.clone();
        let reopened = Cell::new(0);
        scope_marked_content_ids(&document, &mut scoped, || {
            reopened.set(reopened.get() + 1);
            Ok(stages::open(&lib, &input, None, &[]).unwrap())
        })
        .unwrap();
        assert_eq!(
            reopened.get(),
            1,
            "only the flattened page reads a fresh copy"
        );

        let expected: [&[&[i32]]; 2] = [&[&[], &[0], &[1]], &[&[], &[0], &[1]]];
        for ((page, before), expected) in scoped.pages.iter().zip(&core.pages).zip(expected) {
            let ids: Vec<_> = page.struct_nodes.iter().map(|n| n.mcids.clone()).collect();
            assert_eq!(ids, expected, "struct nodes of page {}", page.page_number);
            let mut tree = Vec::new();
            tree_ids(&page.structure_tree.as_ref().unwrap().roots, &mut tree);
            assert_eq!(
                tree, expected,
                "structure tree of page {}",
                page.page_number
            );

            // Where the core already read a node's ids right, its box is the
            // core's to the bit: same view box, same union.
            let mut compared = 0;
            for (node, core_node) in page.struct_nodes.iter().zip(&before.struct_nodes) {
                let same = |ids: &[i32]| ids.iter().copied().collect::<BTreeSet<_>>();
                if same(&node.mcids) == same(&core_node.mcids) {
                    let bits = |rect: &Option<Rect>| {
                        rect.as_ref()
                            .map(|r| [r.x, r.y, r.width, r.height].map(f32::to_bits))
                    };
                    assert_eq!(bits(&node.bbox), bits(&core_node.bbox));
                    compared += usize::from(node.bbox.is_some());
                }
            }
            assert_eq!(
                compared, 1,
                "page {} compares one placed node",
                page.page_number
            );
        }

        // The body's page-1 box is its own line's alone.
        let body = &scoped.pages[0].struct_nodes[2];
        let paragraph = &scoped.pages[0].struct_nodes[1];
        let (body_box, paragraph_box) = (
            body.bbox.as_ref().unwrap(),
            paragraph.bbox.as_ref().unwrap(),
        );
        assert!(body_box.y >= paragraph_box.y + paragraph_box.height);
    }
}
