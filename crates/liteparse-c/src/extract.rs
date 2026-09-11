use liteparse::OutputFormat;
use liteparse::extract::{ExtractedPages, ExtractionOutputOptions, extract_pages_and_images};
use liteparse::types::Page;
use liteparse_pdfium::Library;

use crate::content::{LiteParseContent, LiteParseContentGraphic, LiteParseContentPage};
use crate::document::DocumentState;
use crate::handle::{
    LiteParseByteView, bytes_view, free_handle, opaque_handles, optional_str_view, slice_out,
    state_ref,
};
use crate::render::load_document;
use crate::status::{FfiResult, LiteParseStatus};
use crate::views::{
    LITEPARSE_FORM_TYPE_NONE, LiteParseAnnotation, LiteParseFormField, LiteParseFormTypeValue,
    LiteParseImage, LiteParseImageRef, LiteParsePageError, LiteParsePageGeometry,
    LiteParsePageGeometryValue, LiteParseRect, LiteParseRectValue, LiteParseStructNode,
    LiteParseStructureAttribute, LiteParseStructureNode, LiteParseTextItem, LiteParseVectorLine,
    LiteParseVectorShape, LiteParseWordBox, StructurePacked, VectorsPacked, views,
};

/// Pre-projection pages from `extract_pages_and_images`. Views borrow here.
pub struct LiteParseExtract {
    _opaque: [u8; 0],
}

/// The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseExtractNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseExtract,
}

opaque_handles! {
    LiteParseExtract => ExtractState, "extract";
}

pub(crate) struct ExtractState {
    extracted: ExtractedPages,
    form_type: Option<i32>,
    pages: Vec<LiteParseContentPage>,
    items: Vec<LiteParseTextItem>,
    words: Vec<LiteParseWordBox>,
    word_boxes: Vec<Vec<Vec<LiteParseWordBox>>>,
    graphics: Vec<LiteParseContentGraphic>,
    /// Keeps the `i32` arrays borrowed by `struct_nodes` alive.
    #[allow(dead_code)]
    struct_mcids: Vec<Vec<i32>>,
    struct_nodes: Vec<LiteParseStructNode>,
    images: Vec<LiteParseImage>,
    image_refs: Vec<LiteParseImageRef>,
    page_image_refs: Vec<Vec<LiteParseImageRef>>,
    page_errors: Vec<LiteParsePageError>,
    annotations: Vec<Vec<LiteParseAnnotation>>,
    quadpoints: Vec<Vec<Vec<LiteParseRect>>>,
    form_fields: Vec<Vec<LiteParseFormField>>,
    field_options: Vec<Vec<Vec<LiteParseByteView>>>,
    field_selected_options: Vec<Vec<Vec<LiteParseByteView>>>,
    structure: Vec<Option<StructurePacked>>,
    vectors: Vec<Option<VectorsPacked>>,
    geometries: Vec<LiteParsePageGeometryValue>,
    content_bounds: Vec<LiteParseRectValue>,
}

impl ExtractState {
    fn pack(extracted: ExtractedPages, form_type: Option<i32>, extract_text_metadata: bool) -> Self {
        let mut items = Vec::new();
        let mut words = Vec::new();
        let mut graphics = Vec::new();
        let mut struct_mcids = Vec::new();
        let mut struct_nodes = Vec::new();
        let mut image_refs = Vec::new();
        let mut pages = Vec::with_capacity(extracted.pages.len());
        for page in &extracted.pages {
            let item_offset = items.len();
            let graphic_offset = graphics.len();
            let struct_offset = struct_nodes.len();
            let image_ref_offset = image_refs.len();
            for item in &page.text_items {
                let mut view = LiteParseTextItem::borrow(item, extract_text_metadata);
                // Heading join is mcid ↔ struct_nodes. Result views may hide
                // mcid behind EXTRACT_TEXT_METADATA; the extract snapshot must
                // keep it so as_content can re-enter Markdown classification.
                view.mcid = item.mcid.unwrap_or(0);
                view.has_mcid = item.mcid.is_some();
                view.word_offset = words.len();
                view.word_count = item.words.len();
                words.extend(views(&item.words));
                items.push(view);
            }
            graphics.extend(page.graphics.iter().map(LiteParseContentGraphic::borrow));
            for node in &page.struct_nodes {
                struct_mcids.push(node.mcids.clone());
                struct_nodes.push(LiteParseStructNode::borrow(node, &[]));
            }
            image_refs.extend(views(&page.image_refs));
            pages.push(LiteParseContentPage {
                page_number: page_number(page),
                page_width: page.page_width,
                page_height: page.page_height,
                item_offset,
                item_count: page.text_items.len(),
                graphic_offset,
                graphic_count: page.graphics.len(),
                struct_offset,
                struct_count: page.struct_nodes.len(),
                image_ref_offset,
                image_ref_count: page.image_refs.len(),
                block_offset: 0,
                block_count: 0,
                page_label: optional_str_view(page.page_label.as_deref()),
            });
        }
        for (node, mcids) in struct_nodes.iter_mut().zip(&struct_mcids) {
            *node = LiteParseStructNode {
                mcids: if mcids.is_empty() {
                    std::ptr::null()
                } else {
                    mcids.as_ptr()
                },
                mcids_len: mcids.len(),
                ..*node
            };
        }

        let word_boxes = extracted
            .pages
            .iter()
            .map(|page| {
                page.text_items
                    .iter()
                    .map(|item| views(&item.words))
                    .collect()
            })
            .collect();
        let page_image_refs = extracted
            .pages
            .iter()
            .map(|page| views(&page.image_refs))
            .collect();

        Self {
            pages,
            items,
            words,
            word_boxes,
            graphics,
            struct_mcids,
            struct_nodes,
            images: views(&extracted.images),
            image_refs,
            page_image_refs,
            page_errors: views(&extracted.page_errors),
            annotations: extracted.pages.iter().map(page_annotations).collect(),
            quadpoints: extracted.pages.iter().map(page_quadpoints).collect(),
            form_fields: extracted.pages.iter().map(page_form_fields).collect(),
            field_options: extracted
                .pages
                .iter()
                .map(|page| {
                    page.form_fields
                        .iter()
                        .flatten()
                        .map(|field| string_views(&field.options))
                        .collect()
                })
                .collect(),
            field_selected_options: extracted
                .pages
                .iter()
                .map(|page| {
                    page.form_fields
                        .iter()
                        .flatten()
                        .map(|field| string_views(&field.selected_options))
                        .collect()
                })
                .collect(),
            structure: extracted
                .pages
                .iter()
                .map(|page| page.structure_tree.as_ref().map(StructurePacked::pack))
                .collect(),
            vectors: extracted
                .pages
                .iter()
                .map(|page| page.vector_graphics.as_ref().map(VectorsPacked::pack))
                .collect(),
            geometries: extracted
                .pages
                .iter()
                .map(|page| {
                    page.geometry
                        .as_ref()
                        .and_then(LiteParsePageGeometry::from_core)
                        .map(|geometry| LiteParsePageGeometryValue {
                            geometry,
                            present: true,
                        })
                        .unwrap_or_default()
                })
                .collect(),
            content_bounds: extracted
                .pages
                .iter()
                .map(|page| LiteParseRectValue::from(page.content_bounds.as_ref()))
                .collect(),
            extracted,
            form_type,
        }
    }

    fn page(&self, page_index: usize) -> Option<&Page> {
        self.extracted.pages.get(page_index)
    }
}

fn page_number(page: &Page) -> u32 {
    page.page_number.min(u32::MAX as usize) as u32
}

fn page_annotations(page: &Page) -> Vec<LiteParseAnnotation> {
    views(page.annotations.as_deref().unwrap_or_default())
}

fn page_quadpoints(page: &Page) -> Vec<Vec<LiteParseRect>> {
    page.annotations
        .iter()
        .flatten()
        .map(|annotation| views(&annotation.quadpoint_rects))
        .collect()
}

fn page_form_fields(page: &Page) -> Vec<LiteParseFormField> {
    views(page.form_fields.as_deref().unwrap_or_default())
}

fn string_views(strings: &[String]) -> Vec<LiteParseByteView> {
    strings.iter().map(|s| bytes_view(s.as_bytes())).collect()
}

fn empty_form_type() -> LiteParseFormTypeValue {
    LiteParseFormTypeValue {
        present: false,
        value: LITEPARSE_FORM_TYPE_NONE,
    }
}

pub(crate) fn extract_pages(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<ExtractState> {
    let lib = Library::init();
    let document = load_document(&lib, &state.input, state.config.password.as_deref())?;
    let form_type = state
        .config
        .extract_form_fields
        .then(|| document.form_type());
    let markdown = state.config.output_format == OutputFormat::Markdown;
    let extracted = extract_pages_and_images(
        &document,
        pages.as_deref(),
        state.config.max_pages,
        state.config.extract_links && markdown,
        state.glyph_resolver.as_deref(),
        ExtractionOutputOptions {
            continue_on_page_error: state.config.continue_on_page_error,
            extract_content_bounds: state.config.extract_content_bounds,
            extract_text_metadata: state.config.extract_text_metadata,
            extract_images: state.config.effective_extract_images(),
            extract_vector_graphics: state.config.extract_vector_graphics,
            extract_annotations: state.config.extract_annotations,
            extract_form_fields: state.config.extract_form_fields,
            extract_structure_tree: state.config.extract_structure_tree,
            emit_word_boxes: state.config.emit_word_boxes || markdown,
        },
    )?;
    Ok(ExtractState::pack(
        extracted,
        form_type,
        state.config.extract_text_metadata,
    ))
}

/// Destroy an extract handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_free(extract: *mut LiteParseExtract) {
    unsafe { free_handle(extract) };
}

/// Packed pages for `liteparse_parser_parse_content`.
///
/// Pointers borrow from `extract` and stay valid until it is freed. The
/// snapshot includes items, graphics, word boxes, struct-tree nodes, and
/// image refs. Blocks, outline, annotations, forms, and the logical
/// structure tree are empty here; use the extract accessors for those.
/// Keep the handle alive for the duration of `liteparse_parser_parse_content`.
///
/// `extract` must be live, or null (returns an empty default content).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_as_content(
    extract: *const LiteParseExtract,
) -> LiteParseContent {
    match unsafe { state_ref(extract) } {
        Ok(state) => LiteParseContent {
            size_of_content: size_of::<LiteParseContent>(),
            pages: state.pages.as_ptr(),
            pages_len: state.pages.len(),
            items: state.items.as_ptr(),
            items_len: state.items.len(),
            graphics: state.graphics.as_ptr(),
            graphics_len: state.graphics.len(),
            blocks: std::ptr::null(),
            blocks_len: 0,
            cells: std::ptr::null(),
            cells_len: 0,
            rows: std::ptr::null(),
            rows_len: 0,
            lines: std::ptr::null(),
            lines_len: 0,
            outline: std::ptr::null(),
            outline_len: 0,
            words: state.words.as_ptr(),
            words_len: state.words.len(),
            struct_nodes: state.struct_nodes.as_ptr(),
            struct_nodes_len: state.struct_nodes.len(),
            image_refs: state.image_refs.as_ptr(),
            image_refs_len: state.image_refs.len(),
            document_block_offset: 0,
            document_block_count: 0,
        },
        Err(_) => crate::liteparse_content_default(),
    }
}

/// Return the number of successfully extracted pages.
///
/// # Safety
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_page_count(extract: *const LiteParseExtract) -> usize {
    unsafe { state_ref(extract) }.map_or(0, |state| state.pages.len())
}

/// Borrow packed pages. Offset/count pairs index the shared item and graphic
/// arrays.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_pages(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const LiteParseContentPage {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(extract)?.pages.as_slice()))
        })
    }
}

/// Borrow the flattened pre-projection text items.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_items(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const LiteParseTextItem {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(extract)?.items.as_slice()))
        })
    }
}

/// Borrow the flattened structure-tree nodes used for heading detection.
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_struct_nodes(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const LiteParseStructNode {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(extract)?.struct_nodes.as_slice()))
        })
    }
}

/// Borrow the flattened layout graphics.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_graphics(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const LiteParseContentGraphic {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(extract)?.graphics.as_slice()))
        })
    }
}

/// Borrow one page's `/PageLabels` label, or an empty view when absent.
///
/// # Safety
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_page_label(
    extract: *const LiteParseExtract,
    page_index: usize,
) -> LiteParseByteView {
    unsafe { state_ref(extract) }
        .ok()
        .and_then(|state| state.page(page_index))
        .map(|page| optional_str_view(page.page_label.as_deref()))
        .unwrap_or_default()
}

/// Return the resolved PDF box, user unit, and rotation for one page.
///
/// # Safety
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_page_geometry(
    extract: *const LiteParseExtract,
    page_index: usize,
) -> LiteParsePageGeometryValue {
    unsafe { state_ref(extract) }
        .ok()
        .and_then(|state| state.geometries.get(page_index).copied())
        .unwrap_or_default()
}

/// Return one page's union content bounds.
///
/// # Safety
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_page_content_bounds(
    extract: *const LiteParseExtract,
    page_index: usize,
) -> LiteParseRectValue {
    unsafe { state_ref(extract) }
        .ok()
        .and_then(|state| state.content_bounds.get(page_index).copied())
        .unwrap_or_default()
}

/// Borrow one text item's word boxes. `item_index` is per-page.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_word_boxes(
    extract: *const LiteParseExtract,
    page_index: usize,
    item_index: usize,
    out_len: *mut usize,
) -> *const LiteParseWordBox {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .word_boxes
                .get(page_index)
                .and_then(|items| items.get(item_index))
                .map(Vec::as_slice))
        })
    }
}

/// Borrow decoded images. Empty unless `LITEPARSE_FLAG_EXTRACT_IMAGES`.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_images(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const LiteParseImage {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(extract)?.images.as_slice()))
        })
    }
}

/// Return the count of image extraction failures.
///
/// # Safety
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_image_error_count(
    extract: *const LiteParseExtract,
) -> u32 {
    unsafe { state_ref(extract) }.map_or(0, |state| state.extracted.image_error_count)
}

/// Borrow one page's image objects, including bounds when bytes were skipped.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_image_refs(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseImageRef {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .page_image_refs
                .get(page_index)
                .map(Vec::as_slice))
        })
    }
}

/// Borrow tolerated page errors.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_page_errors(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const LiteParsePageError {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(extract)?.page_errors.as_slice()))
        })
    }
}

/// Whether extraction flattened at least one selected page to recover
/// form-widget text. Flattening happens on a temporary PDFium document,
/// not the document handle.
///
/// Hosts that must reproduce the flattened content stream (for example an
/// OCR raster) should flatten exactly the pages from
/// `liteparse_extract_flattened_page_numbers`. Flattening any other page
/// hides that page's non-widget annotations. C screenshot does not flatten;
/// it renders the reopened, unflattened input.
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_flattened_form_widgets(
    extract: *const LiteParseExtract,
) -> bool {
    unsafe { state_ref(extract) }.is_ok_and(|state| state.extracted.flattened_form_widgets)
}

/// Borrow the 1-based page numbers that were flattened during extraction.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_flattened_page_numbers(
    extract: *const LiteParseExtract,
    out_len: *mut usize,
) -> *const u32 {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(
                state_ref(extract)?
                    .extracted
                    .flattened_page_numbers
                    .as_slice(),
            ))
        })
    }
}

/// Return the document form type when form-field extraction was enabled.
///
/// # Safety
///
/// `extract` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_form_type(
    extract: *const LiteParseExtract,
) -> LiteParseFormTypeValue {
    unsafe { state_ref(extract) }
        .ok()
        .and_then(|state| state.form_type)
        .map_or(empty_form_type(), |value| LiteParseFormTypeValue {
            present: true,
            value,
        })
}

/// Borrow one page's annotations.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_annotations(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseAnnotation {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .annotations
                .get(page_index)
                .map(Vec::as_slice))
        })
    }
}

/// Borrow one annotation's quadpoint rectangles.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_annotation_quadpoints(
    extract: *const LiteParseExtract,
    page_index: usize,
    annotation_index: usize,
    out_len: *mut usize,
) -> *const LiteParseRect {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .quadpoints
                .get(page_index)
                .and_then(|page| page.get(annotation_index))
                .map(Vec::as_slice))
        })
    }
}

/// Borrow one page's AcroForm widgets.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_form_fields(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseFormField {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .form_fields
                .get(page_index)
                .map(Vec::as_slice))
        })
    }
}

/// Borrow one widget's option strings.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_form_field_options(
    extract: *const LiteParseExtract,
    page_index: usize,
    field_index: usize,
    out_len: *mut usize,
) -> *const LiteParseByteView {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .field_options
                .get(page_index)
                .and_then(|fields| fields.get(field_index))
                .map(Vec::as_slice))
        })
    }
}

/// Borrow one widget's selected option strings.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_form_field_selected_options(
    extract: *const LiteParseExtract,
    page_index: usize,
    field_index: usize,
    out_len: *mut usize,
) -> *const LiteParseByteView {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(extract)?
                .field_selected_options
                .get(page_index)
                .and_then(|fields| fields.get(field_index))
                .map(Vec::as_slice))
        })
    }
}

/// Borrow one page's pre-order structure-tree nodes.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_structure_nodes(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseStructureNode {
    unsafe {
        slice_out(out_len, || {
            Ok(structure(state_ref(extract)?, page_index).map(|packed| packed.nodes.as_slice()))
        })
    }
}

/// Borrow one page's flattened structure attributes.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_structure_attributes(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseStructureAttribute {
    unsafe {
        slice_out(out_len, || {
            Ok(structure(state_ref(extract)?, page_index).map(|packed| packed.attributes.as_slice()))
        })
    }
}

/// Borrow one page's flattened structure-node annotations.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_structure_annotations(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseAnnotation {
    unsafe {
        slice_out(out_len, || {
            Ok(
                structure(state_ref(extract)?, page_index)
                    .map(|packed| packed.annotations.as_slice()),
            )
        })
    }
}

/// Borrow one page's flattened structure-node marked-content ids.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_structure_marked_content_ids(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const i32 {
    unsafe {
        slice_out(out_len, || {
            Ok(structure(state_ref(extract)?, page_index)
                .map(|packed| packed.marked_content_ids.as_slice()))
        })
    }
}

/// Borrow one page's vector path objects.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_vector_shapes(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseVectorShape {
    unsafe {
        slice_out(out_len, || {
            Ok(vectors(state_ref(extract)?, page_index).map(|packed| packed.shapes.as_slice()))
        })
    }
}

/// Borrow one page's merged vector segments.
///
/// # Safety
///
/// `extract` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_extract_vector_lines(
    extract: *const LiteParseExtract,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseVectorLine {
    unsafe {
        slice_out(out_len, || {
            Ok(vectors(state_ref(extract)?, page_index).map(|packed| packed.lines.as_slice()))
        })
    }
}

fn structure(state: &ExtractState, page_index: usize) -> Option<&StructurePacked> {
    state.structure.get(page_index)?.as_ref()
}

fn vectors(state: &ExtractState, page_index: usize) -> Option<&VectorsPacked> {
    state.vectors.get(page_index)?.as_ref()
}
