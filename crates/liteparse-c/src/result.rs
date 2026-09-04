use std::collections::HashMap;
use std::sync::OnceLock;

use liteparse::ParseResult;
use liteparse::search::{SearchOptions, search_items};
use liteparse::types::TextItem;
use serde_json::Value;

use crate::document::DescriptiveInfo;
use crate::handle::{
    LiteParseByteView, build_handle, bytes_view, free_handle, opaque_handles, required_view_str,
    slice_out, state_ref, write_out,
};
use crate::render::effective_dpi;
use crate::screenshots::screenshot_views;
use crate::status::{FfiError, FfiResult, LiteParseStatus, boundary};
use crate::views::{
    BlocksPacked, LITEPARSE_FORM_TYPE_NONE, LiteParseAnnotation, LiteParseDocumentMeta,
    LiteParseDocumentMetaValue, LiteParseFormField, LiteParseFormTypeValue, LiteParseImage,
    LiteParseLayoutBlock, LiteParseLayoutCell, LiteParseLayoutRow, LiteParseOutlineEntry,
    LiteParsePageComplexity, LiteParsePageComplexityValue, LiteParsePageError,
    LiteParsePageGeometry, LiteParsePageGeometryValue, LiteParsePageSize, LiteParseRect,
    LiteParseRectValue, LiteParseScreenshot, LiteParseScreenshotRect, LiteParseStructureAttribute,
    LiteParseStructureNode, LiteParseTextItem, LiteParseVectorLine, LiteParseVectorShape,
    LiteParseWordBox, LiteParseXfaPacket, StructurePacked, VectorsPacked, views,
};

pub struct LiteParseResult {
    _opaque: [u8; 0],
}

/// The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseResultNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseResult,
}

pub struct LiteParseSearchMatches {
    _opaque: [u8; 0],
}

/// The handle is null unless `status` is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseSearchMatchesNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseSearchMatches,
}

opaque_handles! {
    LiteParseResult => ResultState, "result";
    LiteParseSearchMatches => SearchState, "matches";
}

struct PackedExtras {
    outline: Vec<LiteParseOutlineEntry>,
    page_errors: Vec<LiteParsePageError>,
    xfa_packets: Vec<LiteParseXfaPacket>,
    images: Vec<LiteParseImage>,
    screenshots: Vec<LiteParseScreenshot>,
    screenshot_rects: Vec<Vec<LiteParseScreenshotRect>>,
    annotations: Vec<Vec<LiteParseAnnotation>>,
    quadpoints: Vec<Vec<Vec<LiteParseRect>>>,
    form_fields: Vec<Vec<LiteParseFormField>>,
    field_options: Vec<Vec<Vec<LiteParseByteView>>>,
    field_selected_options: Vec<Vec<Vec<LiteParseByteView>>>,
    structure: Vec<Option<StructurePacked>>,
    blocks: Vec<Option<BlocksPacked>>,
    vectors: Vec<Option<VectorsPacked>>,
}

impl PackedExtras {
    fn pack(result: &ParseResult, requested_dpi: f32) -> Self {
        let page_sizes: HashMap<usize, (f32, f32)> = result
            .pages
            .iter()
            .map(|page| (page.page_number, (page.page_width, page.page_height)))
            .collect();
        let screenshots_with_dpi = result.screenshots.iter().map(|shot| {
            let dpi = page_sizes
                .get(&(shot.page_num as usize))
                .map_or(requested_dpi, |(width, height)| {
                    effective_dpi(requested_dpi, *width, *height)
                });
            (shot, dpi)
        });
        let (screenshots, screenshot_rects) = screenshot_views(screenshots_with_dpi);
        Self {
            outline: views(&result.outline),
            page_errors: views(&result.page_errors),
            xfa_packets: views(result.xfa_packets.as_deref().unwrap_or_default()),
            images: views(&result.images),
            screenshots,
            screenshot_rects,
            annotations: per_page(result, |page| {
                views(page.annotations.as_deref().unwrap_or_default())
            }),
            quadpoints: per_page(result, |page| {
                page.annotations
                    .iter()
                    .flatten()
                    .map(|annotation| views(&annotation.quadpoint_rects))
                    .collect()
            }),
            form_fields: per_page(result, |page| {
                views(page.form_fields.as_deref().unwrap_or_default())
            }),
            field_options: per_page(result, |page| {
                page.form_fields
                    .iter()
                    .flatten()
                    .map(|field| string_views(&field.options))
                    .collect()
            }),
            field_selected_options: per_page(result, |page| {
                page.form_fields
                    .iter()
                    .flatten()
                    .map(|field| string_views(&field.selected_options))
                    .collect()
            }),
            structure: per_page(result, |page| {
                page.structure_tree.as_ref().map(StructurePacked::pack)
            }),
            blocks: per_page(result, |page| {
                page.blocks
                    .as_deref()
                    .filter(|blocks| !blocks.is_empty())
                    .map(BlocksPacked::pack)
            }),
            vectors: per_page(result, |page| {
                page.vector_graphics.as_ref().map(VectorsPacked::pack)
            }),
        }
    }
}

fn per_page<T>(result: &ParseResult, f: impl Fn(&liteparse::ParsedPage) -> T) -> Vec<T> {
    result.pages.iter().map(f).collect()
}

fn string_views(strings: &[String]) -> Vec<LiteParseByteView> {
    strings.iter().map(|s| bytes_view(s.as_bytes())).collect()
}

pub(crate) struct ResultState {
    result: ParseResult,
    extract_text_metadata: bool,
    requested_dpi: f32,
    descriptive: Option<DescriptiveInfo>,
    json: OnceLock<Result<String, String>>,
    items: OnceLock<Vec<Vec<LiteParseTextItem>>>,
    words: OnceLock<Vec<Vec<Vec<LiteParseWordBox>>>>,
    extras: OnceLock<PackedExtras>,
}

impl ResultState {
    pub(crate) fn new(
        result: ParseResult,
        extract_text_metadata: bool,
        requested_dpi: f32,
        descriptive: Option<DescriptiveInfo>,
    ) -> Self {
        Self {
            result,
            extract_text_metadata,
            requested_dpi,
            descriptive,
            json: OnceLock::new(),
            items: OnceLock::new(),
            words: OnceLock::new(),
            extras: OnceLock::new(),
        }
    }

    fn page(&self, index: usize) -> Option<&liteparse::ParsedPage> {
        self.result.pages.get(index)
    }

    fn json(&self) -> FfiResult<&str> {
        self.json
            .get_or_init(|| {
                format_result(
                    &self.result,
                    self.extract_text_metadata,
                    self.descriptive.as_ref(),
                )
                .map_err(|error| error.message)
            })
            .as_deref()
            .map_err(FfiError::serialization)
    }

    fn items(&self) -> &[Vec<LiteParseTextItem>] {
        self.items.get_or_init(|| {
            self.result
                .pages
                .iter()
                .map(|page| {
                    page.text_items
                        .iter()
                        .map(|item| LiteParseTextItem::borrow(item, self.extract_text_metadata))
                        .collect()
                })
                .collect()
        })
    }

    fn words(&self) -> &[Vec<Vec<LiteParseWordBox>>] {
        self.words.get_or_init(|| {
            self.result
                .pages
                .iter()
                .map(|page| {
                    page.text_items
                        .iter()
                        .map(|item| views(&item.words))
                        .collect()
                })
                .collect()
        })
    }

    fn extras(&self) -> &PackedExtras {
        self.extras
            .get_or_init(|| PackedExtras::pack(&self.result, self.requested_dpi))
    }
}

fn format_result(
    result: &ParseResult,
    extract_text_metadata: bool,
    descriptive: Option<&DescriptiveInfo>,
) -> FfiResult<String> {
    let json = liteparse::output::json::format_json_result(result, extract_text_metadata)
        .map_err(FfiError::serialization)?;
    let mut value: Value = serde_json::from_str(&json).map_err(FfiError::serialization)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| FfiError::serialization("core parse JSON was not an object"))?;
    object.insert("text".into(), Value::String(result.text.clone()));
    if let Some(creator) = &result.creator {
        object.insert("creator".into(), Value::String(creator.clone()));
    }
    if let Some(producer) = &result.producer {
        object.insert("producer".into(), Value::String(producer.clone()));
    }
    if let Some(metadata) = &result.doc_meta {
        let mut metadata = serde_json::to_value(metadata).map_err(FfiError::serialization)?;
        if let (Some(descriptive), Some(object)) = (descriptive, metadata.as_object_mut()) {
            for (key, value) in [
                ("title", &descriptive.title),
                ("author", &descriptive.author),
                ("subject", &descriptive.subject),
                ("keywords", &descriptive.keywords),
                ("trapped", &descriptive.trapped),
            ] {
                if let Some(value) = value {
                    object.insert(key.into(), Value::String(value.clone()));
                }
            }
        }
        object.insert("doc_meta".into(), metadata);
    }
    if !result.outline.is_empty() {
        let entries = result
            .outline
            .iter()
            .map(|entry| {
                let mut entry_json = serde_json::json!({
                    "level": entry.level,
                    "title": entry.title,
                    "page_index": entry.page_index,
                });
                if let Some(y) = entry.y_pdf {
                    entry_json["y_pdf"] = Value::from(y);
                }
                entry_json
            })
            .collect();
        object.insert("outline".into(), Value::Array(entries));
    }
    serde_json::to_string_pretty(&value).map_err(FfiError::serialization)
}

/// Destroy a result handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_free(result: *mut LiteParseResult) {
    unsafe { free_handle(result) };
}

/// Borrow the cached pretty JSON result.
///
/// # Safety
///
/// `result` must be live and `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_to_json(
    result: *const LiteParseResult,
    out: *mut LiteParseByteView,
) -> LiteParseStatus {
    unsafe { write_out(out, None) };
    boundary(|| unsafe {
        let json = state_ref(result)?.json()?;
        write_out(out, Some(bytes_view(json.as_bytes())));
        Ok(())
    })
}

/// Return the source document page count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_total_pages(result: *const LiteParseResult) -> u32 {
    unsafe { state_ref(result) }.map_or(0, |state| state.result.total_pages)
}

/// Return the number of parsed pages.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_count(result: *const LiteParseResult) -> usize {
    unsafe { state_ref(result) }.map_or(0, |state| state.result.pages.len())
}

/// Return a page's 1-based source page number.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_number(
    result: *const LiteParseResult,
    page_index: usize,
) -> u32 {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .map_or(0, |page| page.page_number.min(u32::MAX as usize) as u32)
}

/// Borrow full-document plain text or Markdown, according to the output format.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_text(
    result: *const LiteParseResult,
) -> LiteParseByteView {
    unsafe { state_ref(result) }
        .map(|state| bytes_view(state.result.text.as_bytes()))
        .unwrap_or_default()
}

/// Borrow one page's plain UTF-8 text.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_text(
    result: *const LiteParseResult,
    page_index: usize,
) -> LiteParseByteView {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .map(|page| bytes_view(page.text.as_bytes()))
        .unwrap_or_default()
}

/// Borrow one page's Markdown; empty unless Markdown output was requested.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_markdown(
    result: *const LiteParseResult,
    page_index: usize,
) -> LiteParseByteView {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .map(|page| bytes_view(page.markdown.as_bytes()))
        .unwrap_or_default()
}

/// Borrow the document's optional `/Info` Creator value. Empty when absent.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_creator(
    result: *const LiteParseResult,
) -> LiteParseByteView {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.result.creator.as_deref())
        .map(|value| bytes_view(value.as_bytes()))
        .unwrap_or_default()
}

/// Borrow the document's optional `/Info` Producer value. Empty when absent.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_producer(
    result: *const LiteParseResult,
) -> LiteParseByteView {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.result.producer.as_deref())
        .map(|value| bytes_view(value.as_bytes()))
        .unwrap_or_default()
}

/// Return one page's viewport dimensions in 72-DPI points.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_size(
    result: *const LiteParseResult,
    page_index: usize,
) -> LiteParsePageSize {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .map(|page| LiteParsePageSize {
            width: page.page_width,
            height: page.page_height,
        })
        .unwrap_or_default()
}

/// Return the resolved PDF box, user unit, and rotation for one page.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_geometry(
    result: *const LiteParseResult,
    page_index: usize,
) -> LiteParsePageGeometryValue {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .and_then(|page| page.geometry.as_ref())
        .and_then(LiteParsePageGeometry::from_core)
        .map(|geometry| LiteParsePageGeometryValue {
            geometry,
            present: true,
        })
        .unwrap_or_default()
}

/// Return the count of image extraction failures.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_image_error_count(result: *const LiteParseResult) -> u32 {
    unsafe { state_ref(result) }.map_or(0, |state| state.result.image_error_count)
}

/// Return the optional document form type.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_form_type(
    result: *const LiteParseResult,
) -> LiteParseFormTypeValue {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.result.form_type)
        .map_or(
            LiteParseFormTypeValue {
                present: false,
                value: LITEPARSE_FORM_TYPE_NONE,
            },
            |value| LiteParseFormTypeValue {
                present: true,
                value,
            },
        )
}

/// Return document metadata when extraction was enabled.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_doc_meta(
    result: *const LiteParseResult,
) -> LiteParseDocumentMetaValue {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| {
            state
                .result
                .doc_meta
                .as_ref()
                .map(|meta| (meta, state.descriptive.as_ref()))
        })
        .map(|(meta, descriptive)| LiteParseDocumentMetaValue {
            present: true,
            meta: LiteParseDocumentMeta::build(meta, descriptive),
        })
        .unwrap_or_default()
}

/// Return one page's union content bounds by value.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_content_bounds(
    result: *const LiteParseResult,
    page_index: usize,
) -> LiteParseRectValue {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .map(|page| LiteParseRectValue::from(page.content_bounds.as_ref()))
        .unwrap_or_default()
}

/// Return complexity when it was included during parsing.
///
/// # Safety
///
/// `result` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_complexity(
    result: *const LiteParseResult,
    page_index: usize,
) -> LiteParsePageComplexityValue {
    unsafe { state_ref(result) }
        .ok()
        .and_then(|state| state.page(page_index))
        .and_then(|page| page.complexity.as_ref())
        .map(|stats| LiteParsePageComplexityValue {
            present: true,
            stats: LiteParsePageComplexity::from(stats),
        })
        .unwrap_or_default()
}

/// Borrow one page's text items.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_text_items(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseTextItem {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .items()
                .get(page_index)
                .map(Vec::as_slice))
        })
    }
}

/// Borrow one text item's word boxes.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_word_boxes(
    result: *const LiteParseResult,
    page_index: usize,
    item_index: usize,
    out_len: *mut usize,
) -> *const LiteParseWordBox {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .words()
                .get(page_index)
                .and_then(|items| items.get(item_index))
                .map(Vec::as_slice))
        })
    }
}

/// Borrow all extracted images.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_images(
    result: *const LiteParseResult,
    out_len: *mut usize,
) -> *const LiteParseImage {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(result)?.extras().images.as_slice()))
        })
    }
}

/// Borrow screenshots produced during parsing.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_screenshots(
    result: *const LiteParseResult,
    out_len: *mut usize,
) -> *const LiteParseScreenshot {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(result)?.extras().screenshots.as_slice()))
        })
    }
}

/// Borrow one screenshot's detected rectangles.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_screenshot_rects(
    result: *const LiteParseResult,
    index: usize,
    out_len: *mut usize,
) -> *const LiteParseScreenshotRect {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .extras()
                .screenshot_rects
                .get(index)
                .map(Vec::as_slice))
        })
    }
}

/// Borrow the document outline.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_outline(
    result: *const LiteParseResult,
    out_len: *mut usize,
) -> *const LiteParseOutlineEntry {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(result)?.extras().outline.as_slice()))
        })
    }
}

/// Borrow all tolerated page errors.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_page_errors(
    result: *const LiteParseResult,
    out_len: *mut usize,
) -> *const LiteParsePageError {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(result)?.extras().page_errors.as_slice()))
        })
    }
}

/// Borrow extracted XFA packets.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_xfa_packets(
    result: *const LiteParseResult,
    out_len: *mut usize,
) -> *const LiteParseXfaPacket {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(result)?.extras().xfa_packets.as_slice()))
        })
    }
}

/// Borrow one page's annotations.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_annotations(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseAnnotation {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .extras()
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
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_annotation_quadpoints(
    result: *const LiteParseResult,
    page_index: usize,
    annotation_index: usize,
    out_len: *mut usize,
) -> *const LiteParseRect {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .extras()
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
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_form_fields(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseFormField {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .extras()
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
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_form_field_options(
    result: *const LiteParseResult,
    page_index: usize,
    field_index: usize,
    out_len: *mut usize,
) -> *const LiteParseByteView {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .extras()
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
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_form_field_selected_options(
    result: *const LiteParseResult,
    page_index: usize,
    field_index: usize,
    out_len: *mut usize,
) -> *const LiteParseByteView {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(result)?
                .extras()
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
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_structure_nodes(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseStructureNode {
    unsafe {
        slice_out(out_len, || {
            Ok(structure(state_ref(result)?, page_index).map(|packed| packed.nodes.as_slice()))
        })
    }
}

/// Borrow one page's flattened structure attributes.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_structure_attributes(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseStructureAttribute {
    unsafe {
        slice_out(out_len, || {
            Ok(
                structure(state_ref(result)?, page_index)
                    .map(|packed| packed.attributes.as_slice()),
            )
        })
    }
}

/// Borrow one page's flattened structure-node annotations.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_structure_annotations(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseAnnotation {
    unsafe {
        slice_out(out_len, || {
            Ok(structure(state_ref(result)?, page_index)
                .map(|packed| packed.annotations.as_slice()))
        })
    }
}

/// Borrow one page's flattened structure-node marked-content ids.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_structure_marked_content_ids(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const i32 {
    unsafe {
        slice_out(out_len, || {
            Ok(structure(state_ref(result)?, page_index)
                .map(|packed| packed.marked_content_ids.as_slice()))
        })
    }
}

/// Borrow one page's classified layout blocks.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_blocks(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseLayoutBlock {
    unsafe {
        slice_out(out_len, || {
            Ok(blocks(state_ref(result)?, page_index).map(|packed| packed.blocks.as_slice()))
        })
    }
}

/// Borrow one page's packed layout table cells. Block header ranges and row
/// offsets index into this slice.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_block_cells(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseLayoutCell {
    unsafe {
        slice_out(out_len, || {
            Ok(blocks(state_ref(result)?, page_index).map(|packed| packed.cells.as_slice()))
        })
    }
}

/// Borrow one page's packed layout table rows.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_block_rows(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseLayoutRow {
    unsafe {
        slice_out(out_len, || {
            Ok(blocks(state_ref(result)?, page_index).map(|packed| packed.rows.as_slice()))
        })
    }
}

/// Borrow one page's verbatim layout source lines (`code`, `grid_fallback`).
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_block_lines(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseByteView {
    unsafe {
        slice_out(out_len, || {
            Ok(blocks(state_ref(result)?, page_index).map(|packed| packed.lines.as_slice()))
        })
    }
}

/// Borrow one page's vector path objects.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_vector_shapes(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseVectorShape {
    unsafe {
        slice_out(out_len, || {
            Ok(vectors(state_ref(result)?, page_index).map(|packed| packed.shapes.as_slice()))
        })
    }
}

/// Borrow one page's merged vector segments.
///
/// # Safety
///
/// `result` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_vector_lines(
    result: *const LiteParseResult,
    page_index: usize,
    out_len: *mut usize,
) -> *const LiteParseVectorLine {
    unsafe {
        slice_out(out_len, || {
            Ok(vectors(state_ref(result)?, page_index).map(|packed| packed.lines.as_slice()))
        })
    }
}

fn structure(state: &ResultState, page_index: usize) -> Option<&StructurePacked> {
    state.extras().structure.get(page_index)?.as_ref()
}

fn blocks(state: &ResultState, page_index: usize) -> Option<&BlocksPacked> {
    state.extras().blocks.get(page_index)?.as_ref()
}

fn vectors(state: &ResultState, page_index: usize) -> Option<&VectorsPacked> {
    state.extras().vectors.get(page_index)?.as_ref()
}

pub(crate) struct SearchState {
    // Keeps the strings borrowed by `items` alive.
    #[allow(dead_code)]
    source: Vec<TextItem>,
    items: Vec<LiteParseTextItem>,
}

/// Search one page. Matches outlive the result handle.
///
/// # Safety
///
/// `result` must be live and `phrase` readable UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_search(
    result: *const LiteParseResult,
    page_index: usize,
    phrase: LiteParseByteView,
    case_sensitive: bool,
) -> LiteParseSearchMatchesNew {
    let (status, handle) = build_handle(|| unsafe {
        let phrase = required_view_str(phrase, "phrase")?;
        let state = state_ref(result)?;
        let page = state.page(page_index).ok_or_else(|| {
            FfiError::invalid_argument(format!(
                "page_index {page_index} is out of range for {} parsed pages",
                state.result.pages.len()
            ))
        })?;
        let options = SearchOptions {
            phrase,
            case_sensitive,
        };
        let source = search_items(&page.text_items, &options);
        let items = source
            .iter()
            .map(|item| LiteParseTextItem::borrow(item, state.extract_text_metadata))
            .collect();
        Ok(SearchState { source, items })
    });
    LiteParseSearchMatchesNew { status, handle }
}

/// Borrow all phrase matches.
///
/// # Safety
///
/// `matches` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_slice(
    matches: *const LiteParseSearchMatches,
    out_len: *mut usize,
) -> *const LiteParseTextItem {
    unsafe { slice_out(out_len, || Ok(Some(state_ref(matches)?.items.as_slice()))) }
}

/// Destroy a search-match handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_free(matches: *mut LiteParseSearchMatches) {
    unsafe { free_handle(matches) };
}
