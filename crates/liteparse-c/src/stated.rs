//! What the document states about its own pages, as it is written rather than
//! as extraction projects it.
//!
//! Extraction answers in a viewport: top-left origin, the visible box as page
//! size, annotations flattened to rectangles. This module answers in PDF user
//! space, the space the page dictionary uses:
//!
//! - the five page boxes, each present only when the page states it, or, for
//!   the media and crop box, when an ancestor in the page tree does;
//! - every annotation of the page's `/Annots` array in array order, with its
//!   `/Subtype` name, `/F` flags, `/NM`, `/T`, `/Contents`, raw `/M` and
//!   `/CreationDate`, the true corners of its `/QuadPoints`, its `/C` colour,
//!   a link's URI and the page a link jumps to;
//! - the structure elements of the page's tree, in pre-order, with `/Lang`,
//!   `/ID`, alternate and actual text and the marked-content ids of the
//!   element's own content.
//!
//! PDFium reads the pages, from [a second document](crate::raw) over the same
//! input, so no orientation correction or form flattening applies to them.
//! PDFium reads the page dictionary alone and reports a colour as three
//! converted components, so the boxes a page inherits and the exact `/C`
//! arrays come from the file's own objects ([`FileFacts`]), read once per
//! document. PDFium reads no structure tree in a document whose catalog does
//! not mark it tagged (`/MarkInfo /Marked true`); a page of a document that
//! has a tree all the same says so ([`LITEPARSE_STATED_PAGE_FLAG_STRUCTURE_UNREAD`]).

use std::ffi::{c_int, c_ulong, c_void};

use liteparse::types::{PageError, PdfInput};
use liteparse_pdfium::{Library, pdfium_sys};

use crate::budget::{bytes_of, check_result_bytes};
use crate::document::{DocumentState, LiteParseDocument, copy_page_numbers};
use crate::handle::{
    Arenas, LiteParseArenas, LiteParseStr, array_ptr, create_handle, free_handle, opaque_handles,
    packed_len, state_ref, view_of, view_state,
};
use crate::page_objects::LITEPARSE_MAX_OBJECTS;
use crate::pdfium_extra::{Extra, PDFACTION_GOTO, PageBoxFn, extra};
use crate::raw::{RawDocument, RawPage};
use crate::records::flag_bits;
use crate::status::{
    FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR, LITEPARSE_STATUS_RESOURCE_LIMIT,
    LiteParseStatus,
};

/// `LiteParseStatedPage.flags` bit: the page states, or inherits, a media box.
/// A page that does not has no usable size; PDFium itself falls back to letter.
pub const LITEPARSE_STATED_PAGE_FLAG_MEDIA_BOX: u32 = 1 << 0;
/// Flag: a crop box is stated or inherited.
pub const LITEPARSE_STATED_PAGE_FLAG_CROP_BOX: u32 = 1 << 1;
/// Flag: the page dictionary states a bleed box.
pub const LITEPARSE_STATED_PAGE_FLAG_BLEED_BOX: u32 = 1 << 2;
/// Flag: the page dictionary states a trim box.
pub const LITEPARSE_STATED_PAGE_FLAG_TRIM_BOX: u32 = 1 << 3;
/// Flag: the page dictionary states an art box.
pub const LITEPARSE_STATED_PAGE_FLAG_ART_BOX: u32 = 1 << 4;
/// Flag: the catalog names a structure tree that PDFium does not read, because
/// the catalog does not mark the document tagged (`/MarkInfo /Marked true`).
/// The page then states no structure elements, whatever the tree holds.
pub const LITEPARSE_STATED_PAGE_FLAG_STRUCTURE_UNREAD: u32 = 1 << 5;

/// `LiteParseStatedAnnotation.flags` bit: PDFium reported the annotation's
/// `/Rect`.
pub const LITEPARSE_STATED_ANNOTATION_FLAG_HAS_RECT: u32 = 1 << 0;
/// Flag: the annotation states `/F`; `pdf_flags` holds it.
pub const LITEPARSE_STATED_ANNOTATION_FLAG_HAS_PDF_FLAGS: u32 = 1 << 1;
/// Flag: the annotation states `/C`.
pub const LITEPARSE_STATED_ANNOTATION_FLAG_COLOUR_STATED: u32 = 1 << 2;
/// Flag: `colour_count` and the first `colour_count` of `colour` hold the stated
/// `/C`. Clear while `COLOUR_STATED` is set when the value could not be read.
pub const LITEPARSE_STATED_ANNOTATION_FLAG_COLOUR_READ: u32 = 1 << 3;
/// Flag: the colour components are the array as the file writes it. Clear
/// when the file's objects could not be read and PDFium supplied the colour:
/// three components, `0` to `1` in 8-bit steps, as PDFium converts them.
pub const LITEPARSE_STATED_ANNOTATION_FLAG_COLOUR_EXACT: u32 = 1 << 4;

/// A rectangle in PDF user space: normalised, `x0 <= x1` and `y0 <= y1`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct LiteParseUserRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// The four corners of one `/QuadPoints` entry in PDF user space, in the order
/// the array states them.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct LiteParseUserQuad {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub x3: f32,
    pub y3: f32,
    pub x4: f32,
    pub y4: f32,
}

/// One annotation of a page's `/Annots` array. Strings index the view's pool;
/// an absent string and an empty one are both `len == 0`. `quad_offset` and
/// `quad_count` range into the view's `quads`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStatedAnnotation {
    /// The `/Subtype` name as stated, without the slash.
    pub subtype: LiteParseStr,
    pub contents: LiteParseStr,
    /// `/T`.
    pub author: LiteParseStr,
    /// `/NM`.
    pub name: LiteParseStr,
    /// `/M` as written, not parsed.
    pub modified: LiteParseStr,
    /// `/CreationDate` as written, not parsed.
    pub created: LiteParseStr,
    /// A link's URI action.
    pub uri: LiteParseStr,
    pub rect: LiteParseUserRect,
    pub quad_offset: u32,
    pub quad_count: u32,
    /// `/F`, meaningful when `HAS_PDF_FLAGS` is set.
    pub pdf_flags: u32,
    /// The 1-based page a link jumps to within this document, `0` when it
    /// jumps nowhere or somewhere PDFium cannot resolve to a page.
    pub target_page: u32,
    /// How many of `colour` are meaningful: `0`, `1`, `3` or `4`, when
    /// `COLOUR_READ` is set.
    pub colour_count: u32,
    /// `/C` components, in the order the array holds them.
    pub colour: [f32; 4],
    /// `LITEPARSE_STATED_ANNOTATION_FLAG_*` bits.
    pub flags: u32,
}

/// One structure element of a page's tree. Strings index the view's pool;
/// `mcid_offset` and `mcid_count` range into the view's `mcids`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStatedElement {
    /// The element's type as PDFium reports it: `/S` after the structure
    /// tree's `/RoleMap`, so a type the map sends to a standard type reads as
    /// that type. The element's own `/S` is not available.
    pub element_type: LiteParseStr,
    /// `/ID`.
    pub id: LiteParseStr,
    /// `/Lang` of this element alone, never inherited.
    pub lang: LiteParseStr,
    /// `/Alt`.
    pub alt_text: LiteParseStr,
    /// `/ActualText`.
    pub actual_text: LiteParseStr,
    /// The 1-based position in this page's elements of the parent, `0` for a
    /// top element.
    pub parent: u32,
    pub mcid_offset: u32,
    pub mcid_count: u32,
}

/// One page. `annotation_*` range into the view's `annotations`, `element_*`
/// into its `elements`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStatedPage {
    pub media_box: LiteParseUserRect,
    pub crop_box: LiteParseUserRect,
    pub bleed_box: LiteParseUserRect,
    pub trim_box: LiteParseUserRect,
    pub art_box: LiteParseUserRect,
    /// `/UserUnit`, `1` when the page states none.
    pub user_unit: f32,
    pub page_number: u32,
    /// `LITEPARSE_STATED_PAGE_FLAG_*` bits.
    pub flags: u32,
    pub annotation_offset: u32,
    pub annotation_count: u32,
    pub element_offset: u32,
    pub element_count: u32,
}

/// Pages stated by the document. Views borrow from the handle.
pub struct LiteParseStatedPages {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseStatedPages => StatedPagesState, "stated_pages";
}

/// What a stated-pages handle borrows. The pool is `arenas.pool`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseStatedPagesView {
    pub arenas: LiteParseArenas,
    pub pages: *const LiteParseStatedPage,
    pub pages_len: usize,
    pub annotations: *const LiteParseStatedAnnotation,
    pub annotations_len: usize,
    pub quads: *const LiteParseUserQuad,
    pub quads_len: usize,
    pub elements: *const LiteParseStatedElement,
    pub elements_len: usize,
    pub mcids: *const i32,
    pub mcids_len: usize,
}

#[derive(Default)]
struct Packer {
    arenas: Arenas,
    pages: Vec<LiteParseStatedPage>,
    annotations: Vec<LiteParseStatedAnnotation>,
    quads: Vec<LiteParseUserQuad>,
    elements: Vec<LiteParseStatedElement>,
    mcids: Vec<i32>,
}

/// Array lengths before a page, restored when the page is skipped.
struct Mark {
    pool: usize,
    annotations: usize,
    quads: usize,
    elements: usize,
    mcids: usize,
}

pub(crate) struct StatedPagesState {
    /// Backing storage for `view`.
    #[allow(dead_code)]
    packer: Packer,
    view: LiteParseStatedPagesView,
}

view_state!(StatedPagesState => LiteParseStatedPagesView, view);

impl Packer {
    fn bytes(&self) -> u64 {
        self.arenas.bytes()
            + bytes_of(&self.pages)
            + bytes_of(&self.annotations)
            + bytes_of(&self.quads)
            + bytes_of(&self.elements)
            + bytes_of(&self.mcids)
    }

    fn mark(&self) -> Mark {
        Mark {
            pool: self.arenas.pool.len(),
            annotations: self.annotations.len(),
            quads: self.quads.len(),
            elements: self.elements.len(),
            mcids: self.mcids.len(),
        }
    }

    fn rollback(&mut self, mark: Mark) {
        self.arenas.pool.truncate(mark.pool);
        self.annotations.truncate(mark.annotations);
        self.quads.truncate(mark.quads);
        self.elements.truncate(mark.elements);
        self.mcids.truncate(mark.mcids);
    }

    fn page(
        &mut self,
        extra: &Extra,
        document: &RawDocument<'_>,
        page_number: u32,
        page: &RawPage<'_>,
        inherited: Option<&PageTreeFacts>,
        structure_unread: bool,
    ) -> FfiResult {
        let pdfium = pdfium_sys::dynamic::pdfium();
        let boxes = [
            page_box(extra.media_box, page).or(inherited.and_then(|boxes| boxes.media)),
            page_box(extra.crop_box, page).or(inherited.and_then(|boxes| boxes.crop)),
            page_box(extra.bleed_box, page),
            page_box(extra.trim_box, page),
            page_box(extra.art_box, page),
        ];
        let [media, crop, bleed, trim, art] = boxes;
        // SAFETY: `page` is a live page of the open document.
        let user_unit = pdfium
            .FPDFPage_GetUserUnit
            .map_or(1.0, |user_unit| unsafe { user_unit(page.handle) });
        let annotation_offset = self.annotations.len();
        self.annotations_of(extra, document, page, inherited)?;
        let annotation_count = self.annotations.len() - annotation_offset;
        let element_offset = self.elements.len();
        self.elements_of(extra, page)?;
        let element_count = self.elements.len() - element_offset;
        self.pages.push(LiteParseStatedPage {
            media_box: media.unwrap_or_default(),
            crop_box: crop.unwrap_or_default(),
            bleed_box: bleed.unwrap_or_default(),
            trim_box: trim.unwrap_or_default(),
            art_box: art.unwrap_or_default(),
            user_unit: if user_unit.is_finite() && user_unit > 0.0 {
                user_unit
            } else {
                1.0
            },
            page_number,
            flags: flag_bits(&[
                (media.is_some(), LITEPARSE_STATED_PAGE_FLAG_MEDIA_BOX),
                (crop.is_some(), LITEPARSE_STATED_PAGE_FLAG_CROP_BOX),
                (bleed.is_some(), LITEPARSE_STATED_PAGE_FLAG_BLEED_BOX),
                (trim.is_some(), LITEPARSE_STATED_PAGE_FLAG_TRIM_BOX),
                (art.is_some(), LITEPARSE_STATED_PAGE_FLAG_ART_BOX),
                (
                    structure_unread,
                    LITEPARSE_STATED_PAGE_FLAG_STRUCTURE_UNREAD,
                ),
            ]),
            annotation_offset: packed_len(annotation_offset),
            annotation_count: packed_len(annotation_count),
            element_offset: packed_len(element_offset),
            element_count: packed_len(element_count),
        });
        check_result_bytes(self.bytes())
    }

    /// `facts` are the page tree's facts about this page, `None` when the
    /// file's objects could not be read.
    fn annotations_of(
        &mut self,
        extra: &Extra,
        document: &RawDocument<'_>,
        page: &RawPage<'_>,
        facts: Option<&PageTreeFacts>,
    ) -> FfiResult {
        let pdfium = pdfium_sys::dynamic::pdfium();
        // SAFETY: `page` is a live page; each annotation is closed before the
        // next is opened.
        unsafe {
            let count = (pdfium.FPDFPage_GetAnnotCount)(page.handle);
            for index in 0..count.max(0) {
                let annot = (pdfium.FPDFPage_GetAnnot)(page.handle, index);
                if annot.is_null() {
                    // An /Annots entry that is not a dictionary.
                    continue;
                }
                // `/Annots` entries and PDFium's annotations share an index.
                let colour =
                    facts.map(|facts| facts.colours.get(index as usize).and_then(Option::as_deref));
                let record = self.annotation(extra, document, annot, colour);
                (pdfium.FPDFPage_CloseAnnot)(annot);
                self.annotations.push(record);
                check_result_bytes(self.bytes())?;
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// `annot` must be open on `document`.
    unsafe fn annotation(
        &mut self,
        extra: &Extra,
        document: &RawDocument<'_>,
        annot: pdfium_sys::FPDF_ANNOTATION,
        colour: Option<Option<&[f32]>>,
    ) -> LiteParseStatedAnnotation {
        let pdfium = pdfium_sys::dynamic::pdfium();
        // SAFETY: the caller guarantees `annot` is open.
        unsafe {
            let subtype = key_text(annot, c"Subtype").unwrap_or_else(|| {
                let known = (pdfium.FPDFAnnot_GetSubtype)(annot);
                known_subtype(known).to_owned()
            });
            let mut rect = pdfium_sys::FS_RECTF::default();
            let rect = ((pdfium.FPDFAnnot_GetRect)(annot, &mut rect) != 0)
                .then(|| normalise(rect.left, rect.bottom, rect.right, rect.top))
                .flatten();

            let quad_offset = self.quads.len();
            if (pdfium.FPDFAnnot_HasAttachmentPoints)(annot) != 0 {
                for index in 0..(pdfium.FPDFAnnot_CountAttachmentPoints)(annot) {
                    let mut quad = pdfium_sys::FS_QUADPOINTSF::default();
                    if (pdfium.FPDFAnnot_GetAttachmentPoints)(annot, index, &mut quad) != 0 {
                        self.quads.push(LiteParseUserQuad {
                            x1: quad.x1,
                            y1: quad.y1,
                            x2: quad.x2,
                            y2: quad.y2,
                            x3: quad.x3,
                            y3: quad.y3,
                            x4: quad.x4,
                            y4: quad.y4,
                        });
                    }
                }
            }

            let has_pdf_flags = has_key(annot, c"F");
            let colour_stated = has_key(annot, c"C");
            let colour = colour_stated
                .then(|| match colour {
                    // The array as the file writes it.
                    Some(components) => components
                        .filter(|components| components.len() <= 4)
                        .map(|components| (components.to_vec(), true)),
                    // The file's objects could not be read: PDFium's conversion.
                    None => read_colour(extra, annot).map(|rgb| (rgb, false)),
                })
                .flatten();
            let (uri, target_page) = if subtype == "Link" {
                link_target(extra, document, annot)
            } else {
                (None, 0)
            };

            let pool = &mut self.arenas.pool;
            LiteParseStatedAnnotation {
                subtype: pool.push(&subtype),
                contents: pool.push_opt(key_text(annot, c"Contents").as_deref()),
                author: pool.push_opt(key_text(annot, c"T").as_deref()),
                name: pool.push_opt(key_text(annot, c"NM").as_deref()),
                modified: pool.push_opt(key_text(annot, c"M").as_deref()),
                created: pool.push_opt(key_text(annot, c"CreationDate").as_deref()),
                uri: pool.push_opt(uri.as_deref()),
                rect: rect.unwrap_or_default(),
                quad_offset: packed_len(quad_offset),
                quad_count: packed_len(self.quads.len() - quad_offset),
                pdf_flags: if has_pdf_flags {
                    (pdfium.FPDFAnnot_GetFlags)(annot) as u32
                } else {
                    0
                },
                target_page,
                colour_count: colour.as_ref().map_or(0, |(c, _)| c.len() as u32),
                colour: colour.as_ref().map_or([0.0; 4], |(components, _)| {
                    let mut padded = [0.0; 4];
                    padded[..components.len()].copy_from_slice(components);
                    padded
                }),
                flags: flag_bits(&[
                    (rect.is_some(), LITEPARSE_STATED_ANNOTATION_FLAG_HAS_RECT),
                    (
                        has_pdf_flags,
                        LITEPARSE_STATED_ANNOTATION_FLAG_HAS_PDF_FLAGS,
                    ),
                    (
                        colour_stated,
                        LITEPARSE_STATED_ANNOTATION_FLAG_COLOUR_STATED,
                    ),
                    (
                        colour.is_some(),
                        LITEPARSE_STATED_ANNOTATION_FLAG_COLOUR_READ,
                    ),
                    (
                        colour.as_ref().is_some_and(|(_, exact)| *exact),
                        LITEPARSE_STATED_ANNOTATION_FLAG_COLOUR_EXACT,
                    ),
                ]),
            }
        }
    }

    /// The page's structure elements in pre-order. The tree PDFium builds for a
    /// page holds the elements with content on the page and their ancestors.
    fn elements_of(&mut self, extra: &Extra, page: &RawPage<'_>) -> FfiResult {
        let pdfium = pdfium_sys::dynamic::pdfium();
        let base = self.elements.len();
        // SAFETY: `page` is a live page; the tree is closed before returning,
        // and elements are used only while it is open.
        unsafe {
            let tree = (pdfium.FPDF_StructTree_GetForPage)(page.handle);
            if tree.is_null() {
                return Ok(());
            }
            // The walk is explicit, so a tree that nests deeply or loops
            // cannot overflow the stack; the element count is the bound.
            let mut pending = Vec::new();
            for index in (0..(pdfium.FPDF_StructTree_CountChildren)(tree)).rev() {
                let root = (pdfium.FPDF_StructTree_GetChildAtIndex)(tree, index);
                if !root.is_null() {
                    pending.push((root, 0u32));
                }
            }
            let mut walked = Ok(());
            while let Some((element, parent)) = pending.pop() {
                if self.elements.len() - base >= LITEPARSE_MAX_OBJECTS as usize {
                    walked = Err(FfiError::new(
                        LITEPARSE_STATUS_RESOURCE_LIMIT,
                        "structure element count exceeds the operation limit",
                    ));
                    break;
                }
                let ordinal = packed_len(self.elements.len() - base + 1);
                let mcid_offset = self.mcids.len();
                let mut children = Vec::new();
                for index in 0..(pdfium.FPDF_StructElement_CountChildren)(element) {
                    let id = (pdfium.FPDF_StructElement_GetChildMarkedContentID)(element, index);
                    if id >= 0 {
                        self.mcids.push(id);
                    }
                    let child = (pdfium.FPDF_StructElement_GetChildAtIndex)(element, index);
                    if !child.is_null() {
                        children.push(child);
                    }
                }
                let pool = &mut self.arenas.pool;
                self.elements.push(LiteParseStatedElement {
                    element_type: pool.push_opt(
                        utf16(|buffer, length| {
                            (pdfium.FPDF_StructElement_GetType)(element, buffer, length)
                        })
                        .as_deref(),
                    ),
                    id: pool.push_opt(
                        utf16(|buffer, length| {
                            (pdfium.FPDF_StructElement_GetID)(element, buffer, length)
                        })
                        .as_deref(),
                    ),
                    lang: pool.push_opt(
                        utf16(|buffer, length| {
                            (extra.struct_element_lang)(element, buffer, length)
                        })
                        .as_deref(),
                    ),
                    alt_text: pool.push_opt(
                        utf16(|buffer, length| {
                            (pdfium.FPDF_StructElement_GetAltText)(element, buffer, length)
                        })
                        .as_deref(),
                    ),
                    actual_text: pool.push_opt(
                        utf16(|buffer, length| {
                            (pdfium.FPDF_StructElement_GetActualText)(element, buffer, length)
                        })
                        .as_deref(),
                    ),
                    parent,
                    mcid_offset: packed_len(mcid_offset),
                    mcid_count: packed_len(self.mcids.len() - mcid_offset),
                });
                if let Err(error) = check_result_bytes(self.bytes()) {
                    walked = Err(error);
                    break;
                }
                for child in children.into_iter().rev() {
                    pending.push((child, ordinal));
                }
            }
            (pdfium.FPDF_StructTree_Close)(tree);
            walked
        }
    }

    fn finish(mut self, errors: &[PageError]) -> FfiResult<StatedPagesState> {
        self.arenas.push_errors(errors);
        check_result_bytes(self.bytes())?;
        let view = LiteParseStatedPagesView {
            arenas: self.arenas.view(),
            pages: array_ptr(&self.pages),
            pages_len: self.pages.len(),
            annotations: array_ptr(&self.annotations),
            annotations_len: self.annotations.len(),
            quads: array_ptr(&self.quads),
            quads_len: self.quads.len(),
            elements: array_ptr(&self.elements),
            elements_len: self.elements.len(),
            mcids: array_ptr(&self.mcids),
            mcids_len: self.mcids.len(),
        };
        Ok(StatedPagesState { packer: self, view })
    }
}

/// What the file's own objects state that PDFium does not report: the facts
/// of each page of the page tree, in page order, and whether the catalog names
/// a structure tree.
#[derive(Clone, Debug, Default)]
pub(crate) struct FileFacts {
    pages: Vec<PageTreeFacts>,
    structure_tree: bool,
}

impl FileFacts {
    /// The facts of the file; `None` when its objects cannot be read. The
    /// reader loads every object of the file, so this runs once per document.
    pub(crate) fn read(input: &PdfInput, password: Option<&str>) -> Option<Self> {
        let pdf = crate::stated_metadata::load(input, password)?;
        let structure_tree = pdf
            .catalog()
            .is_ok_and(|catalog| catalog.has(b"StructTreeRoot"));
        Some(Self {
            pages: PageTreeFacts::read(&pdf),
            structure_tree,
        })
    }
}

/// What the file's own objects state about one page and PDFium does not
/// report: the media and crop boxes the page inherits from the page tree, and
/// the exact `/C` array of each of its annotations. PDFium reads the page
/// dictionary alone, hides how many components a colour array holds and
/// refuses the colour of an annotation that has an appearance stream.
#[derive(Clone, Debug, Default)]
pub(crate) struct PageTreeFacts {
    media: Option<LiteParseUserRect>,
    crop: Option<LiteParseUserRect>,
    /// `/C` of each `/Annots` entry, by array index: `None` for an entry that is
    /// not a dictionary or has no `/C` array.
    colours: Vec<Option<Vec<f32>>>,
}

/// The deepest page tree followed.
const MAX_PAGE_TREE_DEPTH: usize = 64;

impl PageTreeFacts {
    /// The facts of every page of the file's page tree, in page order.
    fn read(pdf: &lopdf::Document) -> Vec<Self> {
        pdf.get_pages()
            .values()
            .map(|page| Self {
                media: Self::inherited_box(pdf, *page, b"MediaBox"),
                crop: Self::inherited_box(pdf, *page, b"CropBox"),
                colours: Self::colours(pdf, *page),
            })
            .collect()
    }

    /// The nearest box called `key` on the page or an ancestor.
    fn inherited_box(
        pdf: &lopdf::Document,
        page: lopdf::ObjectId,
        key: &[u8],
    ) -> Option<LiteParseUserRect> {
        let mut id = page;
        for _ in 0..MAX_PAGE_TREE_DEPTH {
            let node = pdf.get_dictionary(id).ok()?;
            if let Ok(value) = node.get(key) {
                return Self::rect(pdf, value);
            }
            id = node.get(b"Parent").ok()?.as_reference().ok()?;
        }
        None
    }

    fn rect(pdf: &lopdf::Document, value: &lopdf::Object) -> Option<LiteParseUserRect> {
        let [left, bottom, right, top] = *Self::numbers(pdf, value)? else {
            return None;
        };
        normalise(left, bottom, right, top)
    }

    /// An array of numbers, each possibly an indirect object.
    fn numbers(pdf: &lopdf::Document, value: &lopdf::Object) -> Option<Vec<f32>> {
        pdf.dereference(value)
            .ok()?
            .1
            .as_array()
            .ok()?
            .iter()
            .map(|number| pdf.dereference(number).ok()?.1.as_float().ok())
            .collect()
    }

    fn colours(pdf: &lopdf::Document, page: lopdf::ObjectId) -> Vec<Option<Vec<f32>>> {
        let Some(annots) = pdf
            .get_dictionary(page)
            .ok()
            .and_then(|page| page.get(b"Annots").ok())
            .and_then(|annots| pdf.dereference(annots).ok())
            .and_then(|(_, annots)| annots.as_array().ok())
        else {
            return Vec::new();
        };
        annots
            .iter()
            .map(|annot| {
                let annot = pdf.dereference(annot).ok()?.1.as_dict().ok()?;
                Self::numbers(pdf, annot.get(b"C").ok()?)
            })
            .collect()
    }
}

fn page_box(read: PageBoxFn, page: &RawPage<'_>) -> Option<LiteParseUserRect> {
    let (mut left, mut bottom, mut right, mut top) = (0.0, 0.0, 0.0, 0.0);
    // SAFETY: `page` is a live page and the four floats are writable.
    let stated = unsafe { read(page.handle, &mut left, &mut bottom, &mut right, &mut top) };
    if stated == 0 {
        return None;
    }
    normalise(left, bottom, right, top)
}

/// The rectangle with its corners ordered; `None` for a non-finite edge.
fn normalise(left: f32, bottom: f32, right: f32, top: f32) -> Option<LiteParseUserRect> {
    [left, bottom, right, top]
        .iter()
        .all(|edge| edge.is_finite())
        .then(|| LiteParseUserRect {
            x0: left.min(right),
            y0: bottom.min(top),
            x1: left.max(right),
            y1: bottom.max(top),
        })
}

/// `PDF_ANNOT_*` subtype names in PDFium's enumeration order, for an
/// annotation whose `/Subtype` is not a readable string.
fn known_subtype(subtype: c_int) -> &'static str {
    const NAMES: [&str; 28] = [
        "Unknown",
        "Text",
        "Link",
        "FreeText",
        "Line",
        "Square",
        "Circle",
        "Polygon",
        "PolyLine",
        "Highlight",
        "Underline",
        "Squiggly",
        "StrikeOut",
        "Stamp",
        "Caret",
        "Ink",
        "Popup",
        "FileAttachment",
        "Sound",
        "Movie",
        "Widget",
        "Screen",
        "PrinterMark",
        "TrapNet",
        "Watermark",
        "3D",
        "RichMedia",
        "XFAWidget",
    ];
    usize::try_from(subtype)
        .ok()
        .and_then(|index| NAMES.get(index))
        .copied()
        .unwrap_or("Unknown")
}

/// Whether the annotation dictionary has `key`.
///
/// # Safety
///
/// `annot` must be open.
unsafe fn has_key(annot: pdfium_sys::FPDF_ANNOTATION, key: &'static std::ffi::CStr) -> bool {
    // SAFETY: the caller guarantees `annot` is open; `key` is NUL-terminated.
    unsafe { (pdfium_sys::dynamic::pdfium().FPDFAnnot_HasKey)(annot, key.as_ptr()) != 0 }
}

/// The text of the dictionary entry `key` (a string or a name), `None` when the
/// key is absent or its value is empty.
///
/// # Safety
///
/// `annot` must be open.
unsafe fn key_text(
    annot: pdfium_sys::FPDF_ANNOTATION,
    key: &'static std::ffi::CStr,
) -> Option<String> {
    let pdfium = pdfium_sys::dynamic::pdfium();
    // SAFETY: the caller guarantees `annot` is open; `key` is NUL-terminated.
    unsafe {
        if !has_key(annot, key) {
            return None;
        }
        utf16(|buffer, length| {
            (pdfium.FPDFAnnot_GetStringValue)(annot, key.as_ptr(), buffer.cast(), length)
        })
    }
}

/// A UTF-16LE string from PDFium's length-then-fill protocol: `read(null, 0)`
/// reports the byte length with the terminating NUL, `read(buffer, length)`
/// fills it. `None` for an empty string.
fn utf16(mut read: impl FnMut(*mut c_void, c_ulong) -> c_ulong) -> Option<String> {
    let needed = read(std::ptr::null_mut(), 0) as usize;
    if needed < 2 {
        return None;
    }
    let mut buffer = vec![0u16; needed.div_ceil(2)];
    let written = read(buffer.as_mut_ptr().cast(), needed as c_ulong) as usize;
    let mut end = (written / 2).min(buffer.len());
    while end > 0 && buffer[end - 1] == 0 {
        end -= 1;
    }
    (end > 0).then(|| String::from_utf16_lossy(&buffer[..end]))
}

/// `/C` as PDFium converts it: red, green and blue in `0` to `1`, in 8-bit
/// steps. PDFium reads the first three components whatever the array holds,
/// converts a four-component one and refuses an annotation that has an
/// appearance stream.
///
/// # Safety
///
/// `annot` must be open.
unsafe fn read_colour(extra: &Extra, annot: pdfium_sys::FPDF_ANNOTATION) -> Option<Vec<f32>> {
    let (mut red, mut green, mut blue, mut alpha) = (0, 0, 0, 0);
    // SAFETY: the caller guarantees `annot` is open; the four are writable.
    let read =
        unsafe { (extra.annot_color)(annot, 0, &mut red, &mut green, &mut blue, &mut alpha) };
    (read != 0).then(|| {
        [red, green, blue]
            .map(|channel| channel.min(255) as f32 / 255.0)
            .to_vec()
    })
}

/// A link annotation's URI and the 1-based page it jumps to.
///
/// # Safety
///
/// `annot` must be open on `document`.
unsafe fn link_target(
    extra: &Extra,
    document: &RawDocument<'_>,
    annot: pdfium_sys::FPDF_ANNOTATION,
) -> (Option<String>, u32) {
    let pdfium = pdfium_sys::dynamic::pdfium();
    // SAFETY: the caller guarantees `annot` is open on `document`; the link,
    // action and destination handles belong to it and are not freed.
    unsafe {
        let link = (pdfium.FPDFAnnot_GetLink)(annot);
        if link.is_null() {
            return (None, 0);
        }
        let mut dest = (extra.link_dest)(document.handle, link);
        let action = (pdfium.FPDFLink_GetAction)(link);
        let mut uri = None;
        if !action.is_null() {
            uri = action_uri(document.handle, action);
            if dest.is_null() && (extra.action_type)(action) == PDFACTION_GOTO {
                dest = (pdfium.FPDFAction_GetDest)(document.handle, action);
            }
        }
        let page = if dest.is_null() {
            -1
        } else {
            (pdfium.FPDFDest_GetDestPageIndex)(document.handle, dest)
        };
        (uri, u32::try_from(page).map_or(0, |index| index + 1))
    }
}

/// The URI of a URI action, `None` for any other action.
///
/// # Safety
///
/// `action` must belong to `document`.
unsafe fn action_uri(
    document: pdfium_sys::FPDF_DOCUMENT,
    action: pdfium_sys::FPDF_ACTION,
) -> Option<String> {
    let pdfium = pdfium_sys::dynamic::pdfium();
    // SAFETY: the caller guarantees `action` belongs to `document`.
    unsafe {
        let needed = (pdfium.FPDFAction_GetURIPath)(document, action, std::ptr::null_mut(), 0);
        // PDFium reports the length with the NUL for a URI action, 0 otherwise.
        if needed < 2 {
            return None;
        }
        let mut buffer = vec![0u8; needed as usize];
        let written = (pdfium.FPDFAction_GetURIPath)(
            document,
            action,
            buffer.as_mut_ptr().cast::<c_void>(),
            needed,
        ) as usize;
        buffer.truncate(written.saturating_sub(1).min(buffer.len()));
        (!buffer.is_empty()).then(|| String::from_utf8_lossy(&buffer).into_owned())
    }
}

pub(crate) fn extract_stated_pages(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<StatedPagesState> {
    // Before the PDFium lock: loading the file's objects does not touch PDFium.
    let facts = state
        .file_facts
        .get_or_init(|| FileFacts::read(&state.input, state.config.password.as_deref()));
    let lib = Library::try_init()?;
    let extra_symbols = extra()?;
    let raw = RawDocument::open(
        &lib,
        &state.input,
        state.config.password.as_deref(),
        "page facts",
    )?;
    let page_count = u32::try_from(
        // SAFETY: `raw` is open under the PDFium lock `lib` holds.
        unsafe { (pdfium_sys::dynamic::pdfium().FPDF_GetPageCount)(raw.handle) }.max(0),
    )
    .unwrap_or(0);
    let mut pages = pages.unwrap_or_else(|| (1..=page_count).collect());
    pages.truncate(state.config.max_pages);
    // Page order is the same in both readers when the page counts agree.
    let inherited = facts
        .as_ref()
        .map(|facts| facts.pages.as_slice())
        .filter(|pages| pages.len() == page_count as usize);
    // SAFETY: `raw` is open under the PDFium lock `lib` holds.
    let tagged = unsafe { (pdfium_sys::dynamic::pdfium().FPDFCatalog_IsTagged)(raw.handle) } != 0;
    let structure_unread = !tagged && facts.as_ref().is_some_and(|facts| facts.structure_tree);
    let mut packer = Packer::default();
    let mut errors = Vec::new();
    for page_number in pages {
        let mark = packer.mark();
        let packed = raw.page(page_number as i32 - 1).and_then(|page| {
            let boxes = inherited.and_then(|boxes| boxes.get(page_number as usize - 1));
            packer.page(
                extra_symbols,
                &raw,
                page_number,
                &page,
                boxes,
                structure_unread,
            )
        });
        match packed {
            Ok(()) => {}
            Err(error) => {
                packer.rollback(mark);
                if state.config.continue_on_page_error
                    && error.status == LITEPARSE_STATUS_PARSE_ERROR
                {
                    errors.push(PageError {
                        page_number,
                        message: error.message,
                    });
                } else {
                    return Err(error);
                }
            }
        }
    }
    packer.finish(&errors)
}

/// Read what the document states about 1-based pages, in PDF user space: page
/// boxes, annotations and structure elements (see [`LiteParseStatedPagesView`]).
/// Null `pages` with zero length selects all; a selection is validated against
/// the page count, de-duplicated and read in ascending order, and
/// `max_pages` caps either form.
///
/// The media and crop boxes a page inherits and the exact `/C` arrays of its
/// annotations come from the file's own objects, which the first call on a
/// document loads whole: on a file with many objects that costs more than a
/// parse, and the document keeps what it found. When those objects cannot be
/// read, a page states the boxes its own dictionary holds and PDFium's
/// conversion of a colour (`COLOUR_EXACT` clear), and no page sets
/// `STRUCTURE_UNREAD`.
///
/// `document` must be live, `pages` readable or null with zero length, and
/// `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_stated_pages(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    out: *mut *mut LiteParseStatedPages,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = state.selection(copy_page_numbers(pages, pages_len)?)?;
            extract_stated_pages(state, pages)
        })
    }
}

/// Destroy a stated-pages handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_stated_pages_free(stated_pages: *mut LiteParseStatedPages) {
    unsafe { free_handle(stated_pages) };
}

/// Borrow the stated pages; null for a null handle.
///
/// `stated_pages` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_stated_pages_view(
    stated_pages: *const LiteParseStatedPages,
) -> *const LiteParseStatedPagesView {
    unsafe { view_of(stated_pages) }
}
