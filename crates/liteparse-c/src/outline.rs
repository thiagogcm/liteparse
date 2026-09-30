//! The document outline, read so that a looping outline ends.
//!
//! The core walks bookmarks recursively, down every `/First` and along every
//! `/Next`, so an outline whose links loop never ends: a bookmark that is its
//! own descendant recurses until the native stack overflows, and siblings
//! that link back to an earlier one list it forever. PDFium hands out one
//! handle per bookmark dictionary, so this walk lists each bookmark once, on
//! an explicit stack, and otherwise as the core does: in pre-order, levels
//! 1-based, titles and targets read the same way, and a bookmark that
//! targets no page of the document dropped but its children kept.

use std::collections::HashSet;

use liteparse::types::{OutlineTarget, PdfInput};
use liteparse_pdfium::{Library, pdfium_sys};

use crate::raw::RawDocument;
use crate::status::FfiResult;

/// The outline of `input` opened with `password`, in pre-order; empty when
/// it has none. The core opened `input` first, so opening it again fails only
/// under memory pressure.
pub(crate) fn outline(
    lib: &Library,
    input: &PdfInput,
    password: Option<&str>,
) -> FfiResult<Vec<OutlineTarget>> {
    let raw = RawDocument::open(lib, input, password, "outline")?;
    let pdfium = pdfium_sys::dynamic::pdfium();
    let document = raw.handle;
    let mut outline = Vec::new();
    let mut listed = HashSet::new();
    // SAFETY: `raw` is open under the PDFium lock `lib` holds, and every
    // bookmark is one of its own.
    unsafe {
        // The next sibling of each bookmark on the path to the current one,
        // with its level: where the walk resumes once a subtree ends.
        let mut resume = vec![(
            (pdfium.FPDFBookmark_GetFirstChild)(document, std::ptr::null_mut()),
            1u8,
        )];
        while let Some((mut bookmark, mut level)) = resume.pop() {
            while !bookmark.is_null() && listed.insert(bookmark) {
                if let (Some(page_index), y_pdf) = target(pdfium, document, bookmark) {
                    outline.push(OutlineTarget {
                        level,
                        title: title(pdfium, bookmark),
                        page_index,
                        y_pdf,
                    });
                }
                resume.push((
                    (pdfium.FPDFBookmark_GetNextSibling)(document, bookmark),
                    level,
                ));
                bookmark = (pdfium.FPDFBookmark_GetFirstChild)(document, bookmark);
                level = level.saturating_add(1);
            }
        }
    }
    Ok(outline)
}

/// `bookmark`'s title, decoded as the core decodes it: UTF-16LE, one
/// trailing NUL dropped.
///
/// # Safety
///
/// `bookmark` must belong to an open document.
unsafe fn title(
    pdfium: &pdfium_sys::dynamic::PdfiumBindings,
    bookmark: pdfium_sys::FPDF_BOOKMARK,
) -> String {
    // SAFETY: the caller guarantees `bookmark` is live. A null buffer asks
    // only for the length.
    let needed =
        unsafe { (pdfium.FPDFBookmark_GetTitle)(bookmark, std::ptr::null_mut(), 0) } as usize;
    if needed < 2 {
        return String::new();
    }
    let mut buffer = vec![0u16; needed / 2];
    // SAFETY: as above, and `buffer` holds the `needed` bytes PDFium asked for.
    let written = unsafe {
        (pdfium.FPDFBookmark_GetTitle)(bookmark, buffer.as_mut_ptr().cast(), needed as _)
    } as usize;
    if written < 2 {
        return String::new();
    }
    let chars = (written / 2).min(buffer.len());
    let end = if buffer[chars - 1] == 0 {
        chars - 1
    } else {
        chars
    };
    String::from_utf16_lossy(&buffer[..end])
}

/// The zero-based page `bookmark` targets, through its `/Dest` or else its
/// action's, and the target's `y` in PDF user space when it gives one.
///
/// # Safety
///
/// `bookmark` must belong to `document`, which must be open.
unsafe fn target(
    pdfium: &pdfium_sys::dynamic::PdfiumBindings,
    document: pdfium_sys::FPDF_DOCUMENT,
    bookmark: pdfium_sys::FPDF_BOOKMARK,
) -> (Option<i32>, Option<f32>) {
    // SAFETY: the caller guarantees both are live; the destination and
    // action are the document's own, and the out-parameters are locals.
    unsafe {
        let mut dest = (pdfium.FPDFBookmark_GetDest)(document, bookmark);
        if dest.is_null() {
            let action = (pdfium.FPDFBookmark_GetAction)(bookmark);
            if !action.is_null() {
                dest = (pdfium.FPDFAction_GetDest)(document, action);
            }
        }
        if dest.is_null() {
            return (None, None);
        }
        let page_index = (pdfium.FPDFDest_GetDestPageIndex)(document, dest);
        let (mut has_x, mut has_y, mut has_z) = (0, 0, 0);
        let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
        let located = (pdfium.FPDFDest_GetLocationInPage)(
            dest, &mut has_x, &mut has_y, &mut has_z, &mut x, &mut y, &mut z,
        );
        (
            (page_index >= 0).then_some(page_index),
            (located != 0 && has_y != 0).then_some(y),
        )
    }
}

#[cfg(test)]
mod tests {
    use liteparse::stages;

    use super::*;
    use crate::test_pdf::assemble;

    /// Two pages and the outline `outlines`, objects numbered from 5, under
    /// an `/Outlines` dictionary whose `/First` is object 5.
    fn outlined(outlines: &[&[u8]]) -> Vec<u8> {
        let mut objects: Vec<&[u8]> = vec![
            b"<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
            b"<< /Type /Outlines /First 5 0 R >>",
        ];
        objects.extend_from_slice(outlines);
        assemble(&objects)
    }

    fn read(pdf: Vec<u8>) -> Vec<(u8, String, i32, Option<f32>)> {
        let lib = Library::try_init().expect("pdfium");
        let input = PdfInput::Bytes(pdf);
        let entries = outline(&lib, &input, None).expect("outline");
        flat(entries)
    }

    fn flat(entries: Vec<OutlineTarget>) -> Vec<(u8, String, i32, Option<f32>)> {
        entries
            .into_iter()
            .map(|e| (e.level, e.title, e.page_index, e.y_pdf))
            .collect()
    }

    /// A tree the core walks to the end: a parent with no target whose
    /// children are kept, a target through a `/GoTo` action, an `/XYZ`
    /// target with a `y` and a `/FitH` one without, one off the document,
    /// and a UTF-16 title.
    #[test]
    fn a_finite_outline_reads_as_the_core_reads_it() {
        let pdf = outlined(&[
            b"<< /Title (Part) /Parent 4 0 R /First 7 0 R /Next 9 0 R >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
            b"<< /Title (Chapter) /Parent 5 0 R /Dest [6 0 R /XYZ 0 700 0] /Next 8 0 R \
               /First 10 0 R >>",
            b"<< /Title (Action) /Parent 5 0 R /A << /S /GoTo /D [3 0 R /Fit] >> >>",
            b"<< /Title <FEFF00C9007400E9> /Parent 4 0 R /Dest [3 0 R /Fit] /Next 11 0 R >>",
            b"<< /Title (Section) /Parent 7 0 R /Dest [6 0 R /FitH 500] >>",
            b"<< /Title (Elsewhere) /Parent 4 0 R /A << /S /URI /URI (https://x.invalid) >> >>",
        ]);
        let lib = Library::try_init().expect("pdfium");
        let input = PdfInput::Bytes(pdf);
        let ours = flat(outline(&lib, &input, None).expect("outline"));
        let document = stages::open(&lib, &input, None, &[]).expect("open");
        let core = flat(stages::outline(&document));
        assert_eq!(ours, core);
        assert_eq!(
            ours,
            [
                (2, "Chapter".to_owned(), 1, Some(700.0)),
                (3, "Section".to_owned(), 1, None),
                (2, "Action".to_owned(), 0, None),
                (1, "Été".to_owned(), 0, None),
            ]
        );
    }

    /// A bookmark that is its own first child: the core recurses into it
    /// until the stack overflows.
    #[test]
    fn a_bookmark_that_is_its_own_child_is_listed_once() {
        let pdf = outlined(&[
            b"<< /Title (Self) /Parent 4 0 R /First 5 0 R /Dest [3 0 R /Fit] /Next 7 0 R >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
            b"<< /Title (After) /Parent 4 0 R /Dest [6 0 R /Fit] >>",
        ]);
        assert_eq!(
            read(pdf),
            [
                (1, "Self".to_owned(), 0, None),
                (1, "After".to_owned(), 1, None),
            ]
        );
    }

    /// Siblings whose `/Next` links loop back, and a child whose `/First`
    /// climbs back to its parent: the core lists them forever.
    #[test]
    fn looping_siblings_and_descendants_are_listed_once() {
        let pdf = outlined(&[
            b"<< /Title (One) /Parent 4 0 R /Dest [3 0 R /Fit] /Next 7 0 R /First 8 0 R >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
            b"<< /Title (Two) /Parent 4 0 R /Dest [6 0 R /Fit] /Next 5 0 R >>",
            b"<< /Title (Child) /Parent 5 0 R /Dest [6 0 R /Fit] /First 5 0 R >>",
        ]);
        assert_eq!(
            read(pdf),
            [
                (1, "One".to_owned(), 0, None),
                (2, "Child".to_owned(), 1, None),
                (1, "Two".to_owned(), 1, None),
            ]
        );
    }
}
