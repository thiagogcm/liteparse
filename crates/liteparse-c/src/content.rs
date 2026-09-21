use std::collections::BTreeMap;

use liteparse::markdown_layout::{Block, Cell, PositionedBlock, SpanCell};
use liteparse::ocr_merge::PageComplexityStats;
use liteparse::types::{
    DocumentAnnotation, ExtractedImage, FormField, GraphicPrimitive, ImageRef, OutlineTarget, Page,
    Rect, StructNode, StructureAttributeValue, StructureTree, StructureTreeElement, TextItem,
    VectorGraphics, WordBox,
};

use crate::handle::{
    LiteParseByteView, as_slice, create_handle, optional_view_str, state_ref, sub, view_bytes,
};
use crate::parser::{LiteParseParser, build_parser};
use crate::records::*;
use crate::result::{LiteParseResult, ResultState};
use crate::status::{FfiError, FfiResult, LiteParseStatus};

/// Packed page content. Read from a result view, or filled by the caller for
/// `liteparse_parser_parse_content`.
///
/// Pages carry offset/count ranges into the flat arrays. `strings` holds
/// form-field options and block source lines; `annotations` holds page and
/// structure-node annotations; `mcids` holds struct-node and structure-tree
/// marked-content ids; `words` and `char_codes` are shared by every text
/// item. `document_block_offset/count` selects document-wide blocks on input
/// only; results never report them.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseContent {
    /// Must equal `sizeof(LiteParseContent)` on input.
    pub size_of_content: usize,
    pub pages: *const LiteParsePage,
    pub pages_len: usize,
    pub items: *const LiteParseTextItem,
    pub items_len: usize,
    pub words: *const LiteParseWordBox,
    pub words_len: usize,
    pub char_codes: *const u32,
    pub char_codes_len: usize,
    pub graphics: *const LiteParseGraphic,
    pub graphics_len: usize,
    pub struct_nodes: *const LiteParseStructNode,
    pub struct_nodes_len: usize,
    pub mcids: *const i32,
    pub mcids_len: usize,
    pub image_refs: *const LiteParseImageRef,
    pub image_refs_len: usize,
    pub images: *const LiteParseImage,
    pub images_len: usize,
    pub annotations: *const LiteParseAnnotation,
    pub annotations_len: usize,
    pub quadpoints: *const LiteParseRect,
    pub quadpoints_len: usize,
    pub form_fields: *const LiteParseFormField,
    pub form_fields_len: usize,
    pub strings: *const LiteParseByteView,
    pub strings_len: usize,
    pub structure_nodes: *const LiteParseStructureNode,
    pub structure_nodes_len: usize,
    pub structure_attributes: *const LiteParseStructureAttribute,
    pub structure_attributes_len: usize,
    pub blocks: *const LiteParseLayoutBlock,
    pub blocks_len: usize,
    pub cells: *const LiteParseLayoutCell,
    pub cells_len: usize,
    pub rows: *const LiteParseLayoutRow,
    pub rows_len: usize,
    pub vector_shapes: *const LiteParseVectorShape,
    pub vector_shapes_len: usize,
    pub vector_lines: *const LiteParseVectorLine,
    pub vector_lines_len: usize,
    pub outline: *const LiteParseOutlineEntry,
    pub outline_len: usize,
    pub page_errors: *const LiteParsePageError,
    pub page_errors_len: usize,
    pub document_block_offset: u32,
    pub document_block_count: u32,
}

/// Fill `content` with an empty, correctly sized value. Null is a no-op.
///
/// `content` must be null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_content_init(content: *mut LiteParseContent) {
    let value = LiteParseContent {
        size_of_content: size_of::<LiteParseContent>(),
        ..LiteParseContent::default()
    };
    unsafe { crate::handle::write_out(content, value) };
}

/// Parse caller-supplied pages without opening a document. Every view and
/// array is copied during the call.
///
/// With no block ranges the pages run grid projection (and the configured
/// classifier). Any per-page or document-level block range makes those
/// blocks the Markdown structure; `images` and per-page complexity are
/// forwarded on that path. `max_pages` truncates the page list and drops
/// document-level blocks when it does. `crop_box` and `skip_diagonal_text`
/// apply before projection, matching `liteparse_document_parse`. Output-only
/// fields (`page_errors`, page `text`/`markdown`, geometry, and the
/// result-only ranges) are ignored on input.
///
/// `parser` must be live; `content` and every non-null array or view it
/// names must be readable for the call; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_parse_content(
    parser: *const LiteParseParser,
    content: *const LiteParseContent,
    out: *mut *mut LiteParseResult,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let parser = state_ref(parser)?;
            let owned = owned_content(content)?;
            let mut pages = owned.pages;
            let mut page_blocks = owned.page_blocks;
            let mut page_stats = owned.page_stats;
            let mut document_blocks = owned.document_blocks;
            let config = parser.config();
            if pages.len() > config.max_pages {
                pages.truncate(config.max_pages);
                page_blocks.truncate(config.max_pages);
                page_stats.truncate(config.max_pages);
                document_blocks = None;
            }
            liteparse::extract::apply_content_filters(
                &mut pages,
                config.crop_box.as_ref(),
                config.skip_diagonal_text,
            );
            let core = build_parser(config.clone(), None, parser.glyph_resolver());
            let result = if page_blocks.iter().any(|blocks| !blocks.is_empty())
                || document_blocks.is_some()
            {
                core.parse_from_blocks(
                    pages,
                    page_blocks,
                    document_blocks,
                    owned.outline,
                    owned.images,
                    page_stats,
                )?
            } else {
                core.parse_from_pages(pages, owned.outline)
            };
            Ok(ResultState::parsed(result, config, None, Vec::new()))
        })
    }
}

struct OwnedContent {
    pages: Vec<Page>,
    page_blocks: Vec<Vec<PositionedBlock>>,
    page_stats: Vec<Option<PageComplexityStats>>,
    document_blocks: Option<Vec<PositionedBlock>>,
    outline: Vec<OutlineTarget>,
    images: Vec<ExtractedImage>,
}

/// Borrowed input arrays, validated once.
struct Arrays<'a> {
    items: &'a [LiteParseTextItem],
    words: &'a [LiteParseWordBox],
    char_codes: &'a [u32],
    graphics: &'a [LiteParseGraphic],
    struct_nodes: &'a [LiteParseStructNode],
    mcids: &'a [i32],
    image_refs: &'a [LiteParseImageRef],
    annotations: &'a [LiteParseAnnotation],
    quadpoints: &'a [LiteParseRect],
    form_fields: &'a [LiteParseFormField],
    strings: &'a [LiteParseByteView],
    structure_nodes: &'a [LiteParseStructureNode],
    structure_attributes: &'a [LiteParseStructureAttribute],
    blocks: &'a [LiteParseLayoutBlock],
    cells: &'a [LiteParseLayoutCell],
    rows: &'a [LiteParseLayoutRow],
    vector_shapes: &'a [LiteParseVectorShape],
    vector_lines: &'a [LiteParseVectorLine],
}

impl<'a> Arrays<'a> {
    unsafe fn borrow(raw: &'a LiteParseContent) -> FfiResult<Self> {
        macro_rules! field {
            ($name:ident, $len:ident) => {
                unsafe { as_slice(raw.$name, raw.$len, stringify!($name)) }?.unwrap_or_default()
            };
        }
        Ok(Self {
            items: field!(items, items_len),
            words: field!(words, words_len),
            char_codes: field!(char_codes, char_codes_len),
            graphics: field!(graphics, graphics_len),
            struct_nodes: field!(struct_nodes, struct_nodes_len),
            mcids: field!(mcids, mcids_len),
            image_refs: field!(image_refs, image_refs_len),
            annotations: field!(annotations, annotations_len),
            quadpoints: field!(quadpoints, quadpoints_len),
            form_fields: field!(form_fields, form_fields_len),
            strings: field!(strings, strings_len),
            structure_nodes: field!(structure_nodes, structure_nodes_len),
            structure_attributes: field!(structure_attributes, structure_attributes_len),
            blocks: field!(blocks, blocks_len),
            cells: field!(cells, cells_len),
            rows: field!(rows, rows_len),
            vector_shapes: field!(vector_shapes, vector_shapes_len),
            vector_lines: field!(vector_lines, vector_lines_len),
        })
    }
}

unsafe fn owned_content(raw: *const LiteParseContent) -> FfiResult<OwnedContent> {
    let raw = unsafe { raw.as_ref() }.ok_or_else(|| {
        FfiError::invalid_argument("content must not be null; start from liteparse_content_init()")
    })?;
    if raw.size_of_content != size_of::<LiteParseContent>() {
        return Err(FfiError::invalid_argument(format!(
            "content.size_of_content is {} but this library expects {}; rebuild against the current header",
            raw.size_of_content,
            size_of::<LiteParseContent>()
        )));
    }
    let arrays = unsafe { Arrays::borrow(raw) }?;
    let pages = unsafe { as_slice(raw.pages, raw.pages_len, "pages") }?.unwrap_or_default();
    let outline_in =
        unsafe { as_slice(raw.outline, raw.outline_len, "outline") }?.unwrap_or_default();
    let images_in = unsafe { as_slice(raw.images, raw.images_len, "images") }?.unwrap_or_default();
    // Ignored on input, but the null/len invariant still holds.
    unsafe { as_slice(raw.page_errors, raw.page_errors_len, "page_errors") }?;

    let mut owned_pages = Vec::with_capacity(pages.len());
    let mut page_blocks = Vec::with_capacity(pages.len());
    let mut page_stats = Vec::with_capacity(pages.len());
    let mut any_page_blocks = false;
    for (index, page) in pages.iter().enumerate() {
        let where_ = format!("pages[{index}]");
        owned_pages.push(unsafe { copy_page(page, &where_, &arrays) }?);
        let copied = copy_blocks(&arrays, page.block_offset, page.block_count)?;
        any_page_blocks |= !copied.is_empty();
        page_blocks.push(copied);
        page_stats.push(
            has(page.flags, LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY)
                .then(|| PageComplexityStats::from(&page.complexity)),
        );
    }

    let document_blocks = (raw.document_block_count != 0)
        .then(|| copy_blocks(&arrays, raw.document_block_offset, raw.document_block_count))
        .transpose()?;
    if document_blocks.is_none() && !any_page_blocks {
        page_blocks.clear();
    }

    let mut outline = Vec::with_capacity(outline_in.len());
    for (index, entry) in outline_in.iter().enumerate() {
        let where_ = format!("outline[{index}]");
        known_flags(entry.flags, OUTLINE_FLAGS, &where_, "")?;
        finite(&[entry.y_pdf], &where_, "y_pdf")?;
        outline.push(OutlineTarget {
            level: u8::try_from(entry.level).map_err(|_| {
                FfiError::invalid_argument(format!("{where_}.level must fit in u8"))
            })?,
            title: unsafe { str_field(entry.title, &where_, "title") }?.unwrap_or_default(),
            page_index: entry.page_index,
            y_pdf: has(entry.flags, LITEPARSE_OUTLINE_FLAG_HAS_Y_PDF).then_some(entry.y_pdf),
        });
    }

    let mut images = Vec::with_capacity(images_in.len());
    for (index, image) in images_in.iter().enumerate() {
        images.push(unsafe { copy_image(image, &format!("images[{index}]")) }?);
    }

    Ok(OwnedContent {
        pages: owned_pages,
        page_blocks,
        page_stats,
        document_blocks,
        outline,
        images,
    })
}

// Validation helpers take `where_` plus a field name and format the label
// only on failure, so the per-record success path allocates nothing.

fn label(where_: &str, field: &str) -> String {
    if field.is_empty() {
        where_.to_owned()
    } else {
        format!("{where_}.{field}")
    }
}

fn known_flags(flags: u32, known: u32, where_: &str, field: &str) -> FfiResult {
    if flags & !known == 0 {
        Ok(())
    } else {
        Err(FfiError::invalid_argument(format!(
            "{}.flags contains unknown bits",
            label(where_, field)
        )))
    }
}

fn finite(values: &[f32], where_: &str, field: &str) -> FfiResult {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(FfiError::invalid_argument(format!(
            "{} must be finite",
            label(where_, field)
        )))
    }
}

fn finite_rect(rect: &LiteParseRect, where_: &str, field: &str) -> FfiResult {
    if rect.is_finite() {
        Ok(())
    } else {
        Err(FfiError::invalid_argument(format!(
            "{} must be finite",
            label(where_, field)
        )))
    }
}

/// Optional rect gated by a flag bit.
fn flagged_rect(
    flags: u32,
    bit: u32,
    rect: &LiteParseRect,
    where_: &str,
    field: &str,
) -> FfiResult<Option<Rect>> {
    has(flags, bit)
        .then(|| finite_rect(rect, where_, field).map(|()| Rect::from(rect)))
        .transpose()
}

unsafe fn str_field(
    view: LiteParseByteView,
    where_: &str,
    field: &str,
) -> FfiResult<Option<String>> {
    unsafe { optional_view_str(view, field) }
        .map_err(|error| FfiError::invalid_argument(format!("{where_}.{}", error.message)))
}

unsafe fn copy_page(page: &LiteParsePage, where_: &str, arrays: &Arrays<'_>) -> FfiResult<Page> {
    known_flags(page.flags, PAGE_FLAGS, where_, "")?;
    if has(page.flags, LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY) {
        let c = &page.complexity;
        known_flags(c.flags, COMPLEXITY_FLAGS, where_, "complexity")?;
        known_flags(c.reasons, REASON_MASK, where_, "complexity.reasons")?;
        known_flags(
            c.layout_reasons,
            LAYOUT_REASON_MASK,
            where_,
            "complexity.layout_reasons",
        )?;
        finite(
            &[
                c.text_coverage,
                c.image_coverage,
                c.largest_image_coverage,
                c.page_area,
                c.uncovered_vector_area,
                c.layout_ruled_table_coverage,
                c.layout_figure_coverage,
            ],
            where_,
            "complexity",
        )?;
    }
    if page.page_number == 0 {
        return Err(FfiError::invalid_argument(format!(
            "{where_}.page_number must be 1-based"
        )));
    }
    if !page.width.is_finite()
        || !page.height.is_finite()
        || page.width <= 0.0
        || page.height <= 0.0
    {
        return Err(FfiError::invalid_argument(format!(
            "{where_} width and height must be finite and greater than zero"
        )));
    }

    let items = sub(
        arrays.items,
        page.item_offset,
        page.item_count,
        where_,
        "items",
    )?;
    let mut text_items = Vec::with_capacity(items.len());
    for (item_index, item) in items.iter().enumerate() {
        let what = format!("{where_}.items[{item_index}]");
        text_items.push(unsafe { copy_text_item(item, &what, arrays) }?);
    }

    let graphics = sub(
        arrays.graphics,
        page.graphic_offset,
        page.graphic_count,
        where_,
        "graphics",
    )?;
    let mut copied_graphics = Vec::with_capacity(graphics.len());
    for (graphic_index, graphic) in graphics.iter().enumerate() {
        copied_graphics.push(copy_graphic(
            graphic,
            &format!("{where_}.graphics[{graphic_index}]"),
        )?);
    }

    let struct_nodes = sub(
        arrays.struct_nodes,
        page.struct_node_offset,
        page.struct_node_count,
        where_,
        "struct_nodes",
    )?;
    let mut copied_structs = Vec::with_capacity(struct_nodes.len());
    for (struct_index, node) in struct_nodes.iter().enumerate() {
        let what = format!("{where_}.struct_nodes[{struct_index}]");
        known_flags(node.flags, STRUCT_NODE_FLAGS, &what, "")?;
        copied_structs.push(StructNode {
            role: unsafe { str_field(node.role, &what, "role") }?.unwrap_or_default(),
            mcids: sub(
                arrays.mcids,
                node.mcid_offset,
                node.mcid_count,
                &what,
                "mcids",
            )?
            .to_vec(),
            bbox: flagged_rect(
                node.flags,
                LITEPARSE_STRUCT_NODE_FLAG_HAS_BBOX,
                &node.bbox,
                &what,
                "bbox",
            )?,
            alt_text: unsafe { str_field(node.alt_text, &what, "alt_text") }?,
        });
    }

    let image_refs = sub(
        arrays.image_refs,
        page.image_ref_offset,
        page.image_ref_count,
        where_,
        "image_refs",
    )?;
    let mut copied_image_refs = Vec::with_capacity(image_refs.len());
    for (ref_index, image) in image_refs.iter().enumerate() {
        let what = format!("{where_}.image_refs[{ref_index}]");
        finite_rect(&image.bbox, &what, "bbox")?;
        finite(&[image.rotation], &what, "rotation")?;
        copied_image_refs.push(ImageRef {
            id: unsafe { str_field(image.id, &what, "id") }?.unwrap_or_default(),
            bbox: Rect::from(&image.bbox),
            obj_index: image.obj_index as usize,
            format: unsafe { str_field(image.format, &what, "format") }?.unwrap_or_default(),
            pixel_width: image.pixel_width,
            pixel_height: image.pixel_height,
            rotation: image.rotation,
            jpeg_bytes: None,
            raw_bytes: None,
            bits_per_pixel: image.bits_per_pixel,
            colorspace: image.colorspace,
        });
    }

    let flags = page.flags;
    let annotations = has(flags, LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS)
        .then(|| unsafe {
            copy_annotations(
                arrays,
                page.annotation_offset,
                page.annotation_count,
                where_,
                "annotations",
            )
        })
        .transpose()?;
    let form_fields = has(flags, LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS)
        .then(|| unsafe {
            copy_form_fields(
                arrays,
                page.form_field_offset,
                page.form_field_count,
                where_,
            )
        })
        .transpose()?;
    let structure_tree = has(flags, LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE)
        .then(|| unsafe {
            copy_structure_tree(
                arrays,
                page.structure_node_offset,
                page.structure_node_count,
                where_,
            )
        })
        .transpose()?;
    let vector_graphics = has(flags, LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS)
        .then(|| copy_vector_graphics(arrays, page, where_))
        .transpose()?;
    let content_bounds = flagged_rect(
        flags,
        LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS,
        &page.content_bounds,
        where_,
        "content_bounds",
    )?;
    Ok(Page {
        page_number: page.page_number as usize,
        page_label: unsafe { str_field(page.label, where_, "label") }?,
        page_width: page.width,
        page_height: page.height,
        content_bounds,
        text_items,
        graphics: copied_graphics,
        vector_graphics,
        struct_nodes: copied_structs,
        image_refs: copied_image_refs,
        annotations,
        form_fields,
        structure_tree,
    })
}

fn copy_vector_graphics(
    arrays: &Arrays<'_>,
    page: &LiteParsePage,
    where_: &str,
) -> FfiResult<VectorGraphics> {
    let shapes = sub(
        arrays.vector_shapes,
        page.vector_shape_offset,
        page.vector_shape_count,
        where_,
        "vector_shapes",
    )?;
    let lines = sub(
        arrays.vector_lines,
        page.vector_line_offset,
        page.vector_line_count,
        where_,
        "vector_lines",
    )?;
    for shape in shapes {
        known_flags(shape.flags, VECTOR_FLAGS, where_, "vector_shapes")?;
        finite_rect(&shape.bbox, where_, "vector_shapes")?;
    }
    for line in lines {
        known_flags(line.flags, VECTOR_FLAGS, where_, "vector_lines")?;
        finite(
            &[line.x1, line.y1, line.x2, line.y2, line.stroke_width],
            where_,
            "vector_lines",
        )?;
    }
    Ok(VectorGraphics {
        shapes: shapes.iter().map(Into::into).collect(),
        lines: lines.iter().map(Into::into).collect(),
    })
}

unsafe fn copy_text_item(
    item: &LiteParseTextItem,
    where_: &str,
    arrays: &Arrays<'_>,
) -> FfiResult<TextItem> {
    finite(
        &[item.x, item.y, item.width, item.height, item.rotation],
        where_,
        "geometry",
    )?;
    known_flags(item.flags, TEXT_ITEM_FLAGS, where_, "")?;
    let flags = item.flags;
    let flagged = |bit: u32| has(flags, bit);
    let metrics = [
        (LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE, item.font_size),
        (LITEPARSE_TEXT_ITEM_FLAG_HAS_CONFIDENCE, item.confidence),
        (LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_HEIGHT, item.font_height),
        (LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_ASCENT, item.font_ascent),
        (LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_DESCENT, item.font_descent),
        (LITEPARSE_TEXT_ITEM_FLAG_HAS_TEXT_WIDTH, item.text_width),
    ];
    if metrics
        .iter()
        .any(|(bit, value)| flagged(*bit) && !value.is_finite())
    {
        return Err(FfiError::invalid_argument(format!(
            "{where_} font metrics must be finite"
        )));
    }
    let char_codes = sub(
        arrays.char_codes,
        item.char_code_offset,
        item.char_code_count,
        where_,
        "char_codes",
    )?;
    let words = sub(
        arrays.words,
        item.word_offset,
        item.word_count,
        where_,
        "words",
    )?;
    let mut copied_words = Vec::with_capacity(words.len());
    for word in words {
        finite(&[word.x, word.y, word.width, word.height], where_, "words")?;
        copied_words.push(WordBox {
            text: unsafe { str_field(word.text, where_, "words.text") }?.unwrap_or_default(),
            x: word.x,
            y: word.y,
            width: word.width,
            height: word.height,
        });
    }
    Ok(TextItem {
        text: unsafe { str_field(item.text, where_, "text") }?.unwrap_or_default(),
        x: item.x,
        y: item.y,
        width: item.width,
        height: item.height,
        rotation: item.rotation,
        font_name: unsafe { str_field(item.font_name, where_, "font_name") }?,
        font_size: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE).then_some(item.font_size),
        font_height: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_HEIGHT).then_some(item.font_height),
        font_ascent: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_ASCENT).then_some(item.font_ascent),
        font_descent: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_DESCENT)
            .then_some(item.font_descent),
        font_weight: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_WEIGHT).then_some(item.font_weight),
        font_flags: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_FLAGS).then_some(item.font_flags),
        text_width: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_TEXT_WIDTH).then_some(item.text_width),
        font_is_buggy: flagged(LITEPARSE_TEXT_ITEM_FLAG_FONT_IS_BUGGY),
        has_unicode_map_error: flagged(LITEPARSE_TEXT_ITEM_FLAG_UNICODE_MAP_ERROR),
        mcid: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_MCID).then_some(item.mcid),
        fill_color: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_FILL_COLOR)
            .then(|| hex_from_argb(item.fill_color)),
        stroke_color: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_STROKE_COLOR)
            .then(|| hex_from_argb(item.stroke_color)),
        char_codes: char_codes.to_vec(),
        trailing_space_generated: flagged(LITEPARSE_TEXT_ITEM_FLAG_TRAILING_SPACE_GENERATED),
        confidence: flagged(LITEPARSE_TEXT_ITEM_FLAG_HAS_CONFIDENCE).then_some(item.confidence),
        link: unsafe { str_field(item.link, where_, "link") }?,
        strike: flagged(LITEPARSE_TEXT_ITEM_FLAG_STRIKE),
        words: copied_words,
    })
}

fn copy_graphic(graphic: &LiteParseGraphic, where_: &str) -> FfiResult<GraphicPrimitive> {
    known_flags(graphic.flags, GRAPHIC_FLAGS, where_, "")?;
    let fill = has(graphic.flags, LITEPARSE_GRAPHIC_FLAG_HAS_FILL_COLOR)
        .then(|| hex_from_argb(graphic.fill_color));
    let stroke = has(graphic.flags, LITEPARSE_GRAPHIC_FLAG_HAS_STROKE_COLOR)
        .then(|| hex_from_argb(graphic.stroke_color));
    match graphic.kind {
        LITEPARSE_GRAPHIC_STROKE => {
            finite(
                &[
                    graphic.x1,
                    graphic.y1,
                    graphic.x2,
                    graphic.y2,
                    graphic.line_width,
                ],
                where_,
                "stroke geometry",
            )?;
            Ok(GraphicPrimitive::Stroke {
                x1: graphic.x1,
                y1: graphic.y1,
                x2: graphic.x2,
                y2: graphic.y2,
                color: stroke,
                width: graphic.line_width,
            })
        }
        LITEPARSE_GRAPHIC_RECT => {
            finite_rect(&graphic.bbox, where_, "bbox")?;
            Ok(GraphicPrimitive::Rect {
                bbox: Rect::from(&graphic.bbox),
                fill,
                stroke,
            })
        }
        kind => Err(FfiError::invalid_argument(format!(
            "{where_} has unknown graphic kind {kind}"
        ))),
    }
}

unsafe fn copy_annotations(
    arrays: &Arrays<'_>,
    offset: u32,
    count: u32,
    where_: &str,
    field: &str,
) -> FfiResult<Vec<DocumentAnnotation>> {
    let annotations = sub(arrays.annotations, offset, count, where_, field)?;
    let mut out = Vec::with_capacity(annotations.len());
    for (index, annotation) in annotations.iter().enumerate() {
        let what = format!("{where_}.{field}[{index}]");
        known_flags(annotation.flags, ANNOTATION_FLAGS, &what, "")?;
        let rect = flagged_rect(
            annotation.flags,
            LITEPARSE_ANNOTATION_FLAG_HAS_RECT,
            &annotation.rect,
            &what,
            "rect",
        )?;
        let quadpoints = sub(
            arrays.quadpoints,
            annotation.quadpoint_offset,
            annotation.quadpoint_count,
            &what,
            "quadpoints",
        )?;
        for quad in quadpoints {
            finite_rect(quad, &what, "quadpoints")?;
        }
        out.push(DocumentAnnotation {
            subtype: unsafe { str_field(annotation.subtype, &what, "subtype") }?
                .unwrap_or_default(),
            contents: unsafe { str_field(annotation.contents, &what, "contents") }?,
            created: unsafe { str_field(annotation.created, &what, "created") }?,
            modified: unsafe { str_field(annotation.modified, &what, "modified") }?,
            title: unsafe { str_field(annotation.title, &what, "title") }?,
            rect,
            quadpoint_rects: quadpoints.iter().map(Rect::from).collect(),
            uri: unsafe { str_field(annotation.uri, &what, "uri") }?,
        });
    }
    Ok(out)
}

unsafe fn copy_strings(
    arrays: &Arrays<'_>,
    offset: u32,
    count: u32,
    where_: &str,
    field: &str,
) -> FfiResult<Vec<String>> {
    let strings = sub(arrays.strings, offset, count, where_, field)?;
    let mut out = Vec::with_capacity(strings.len());
    for string in strings {
        out.push(unsafe { str_field(*string, where_, field) }?.unwrap_or_default());
    }
    Ok(out)
}

unsafe fn copy_form_fields(
    arrays: &Arrays<'_>,
    offset: u32,
    count: u32,
    where_: &str,
) -> FfiResult<Vec<FormField>> {
    let fields = sub(arrays.form_fields, offset, count, where_, "form_fields")?;
    let mut out = Vec::with_capacity(fields.len());
    for (index, field) in fields.iter().enumerate() {
        let what = format!("{where_}.form_fields[{index}]");
        let flags = field.flags;
        known_flags(flags, FORM_FIELD_FLAGS, &what, "")?;
        out.push(FormField {
            id: unsafe { str_field(field.id, &what, "id") }?.unwrap_or_default(),
            field_type: unsafe { str_field(field.field_type, &what, "field_type") }?
                .unwrap_or_default(),
            page: field.page,
            annotation_index: field.annotation_index,
            widget_index: field.widget_index,
            object_number: has(flags, LITEPARSE_FORM_FIELD_FLAG_HAS_OBJECT_NUMBER)
                .then_some(field.object_number),
            name: unsafe { str_field(field.name, &what, "name") }?,
            alternate_name: unsafe { str_field(field.alternate_name, &what, "alternate_name") }?,
            value: unsafe { str_field(field.value, &what, "value") }?,
            export_value: unsafe { str_field(field.export_value, &what, "export_value") }?,
            field_flags: field.field_flags,
            control_count: has(flags, LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_COUNT)
                .then_some(field.control_count),
            control_index: has(flags, LITEPARSE_FORM_FIELD_FLAG_HAS_CONTROL_INDEX)
                .then_some(field.control_index),
            checked: has(flags, LITEPARSE_FORM_FIELD_FLAG_HAS_CHECKED)
                .then(|| has(flags, LITEPARSE_FORM_FIELD_FLAG_CHECKED)),
            rect: flagged_rect(
                flags,
                LITEPARSE_FORM_FIELD_FLAG_HAS_RECT,
                &field.rect,
                &what,
                "rect",
            )?,
            options: unsafe {
                copy_strings(
                    arrays,
                    field.option_offset,
                    field.option_count,
                    &what,
                    "options",
                )
            }?,
            selected_options: unsafe {
                copy_strings(
                    arrays,
                    field.selected_option_offset,
                    field.selected_option_count,
                    &what,
                    "selected_options",
                )
            }?,
        });
    }
    Ok(out)
}

/// Rebuild a tree from pre-order nodes whose `parent_index` is absolute.
/// Parents must precede their children and lie in the same page range.
unsafe fn copy_structure_tree(
    arrays: &Arrays<'_>,
    offset: u32,
    count: u32,
    where_: &str,
) -> FfiResult<StructureTree> {
    let nodes = sub(
        arrays.structure_nodes,
        offset,
        count,
        where_,
        "structure_nodes",
    )?;
    let start = offset as usize;
    let mut slots: Vec<Option<(u32, StructureTreeElement)>> = Vec::with_capacity(nodes.len());
    for (local, node) in nodes.iter().enumerate() {
        let what = format!("{where_}.structure_nodes[{local}]");
        let parent = node.parent_index;
        if parent != LITEPARSE_NO_PARENT && !(start..start + local).contains(&(parent as usize)) {
            return Err(FfiError::invalid_argument(format!(
                "{what}.parent_index must name an earlier node of the same page"
            )));
        }
        let attributes = sub(
            arrays.structure_attributes,
            node.attribute_offset,
            node.attribute_count,
            &what,
            "attributes",
        )?;
        let mut attribute_map = BTreeMap::new();
        for attribute in attributes {
            let name =
                unsafe { str_field(attribute.name, &what, "attributes.name") }?.unwrap_or_default();
            let value = match attribute.kind {
                LITEPARSE_STRUCTURE_ATTR_BOOL => {
                    StructureAttributeValue::Boolean(attribute.number != 0.0)
                }
                LITEPARSE_STRUCTURE_ATTR_NUMBER => {
                    StructureAttributeValue::Number(attribute.number)
                }
                LITEPARSE_STRUCTURE_ATTR_STRING => StructureAttributeValue::String(
                    unsafe { str_field(attribute.string, &what, "attributes.string") }?
                        .unwrap_or_default(),
                ),
                kind => {
                    return Err(FfiError::invalid_argument(format!(
                        "{what}.attributes has unknown kind {kind}"
                    )));
                }
            };
            attribute_map.insert(name, value);
        }
        slots.push(Some((
            parent,
            StructureTreeElement {
                element_type: unsafe { str_field(node.element_type, &what, "element_type") }?
                    .unwrap_or_default(),
                id: unsafe { str_field(node.id, &what, "id") }?,
                actual_text: unsafe { str_field(node.actual_text, &what, "actual_text") }?,
                alt_text: unsafe { str_field(node.alt_text, &what, "alt_text") }?,
                title: unsafe { str_field(node.title, &what, "title") }?,
                attributes: attribute_map,
                marked_content_ids: sub(
                    arrays.mcids,
                    node.mcid_offset,
                    node.mcid_count,
                    &what,
                    "mcids",
                )?
                .to_vec(),
                children: Vec::new(),
                annotations: unsafe {
                    copy_annotations(
                        arrays,
                        node.annotation_offset,
                        node.annotation_count,
                        &what,
                        "annotations",
                    )
                }?,
            },
        )));
    }
    // Children have larger indices than their parent, so walking backwards
    // completes every child list (reversed) before its parent is taken.
    let mut roots = Vec::new();
    for local in (0..slots.len()).rev() {
        let (parent, mut element) = slots[local].take().expect("taken once");
        element.children.reverse();
        if parent == LITEPARSE_NO_PARENT {
            roots.push(element);
        } else {
            slots[parent as usize - start]
                .as_mut()
                .expect("parent not yet taken")
                .1
                .children
                .push(element);
        }
    }
    roots.reverse();
    Ok(StructureTree { roots })
}

unsafe fn copy_image(image: &LiteParseImage, where_: &str) -> FfiResult<ExtractedImage> {
    finite_rect(&image.bbox, where_, "bbox")?;
    finite(&[image.rotation], where_, "rotation")?;
    Ok(ExtractedImage {
        id: unsafe { str_field(image.id, where_, "id") }?.unwrap_or_default(),
        name: unsafe { str_field(image.name, where_, "name") }?.unwrap_or_default(),
        path: unsafe { str_field(image.path, where_, "path") }?,
        page: image.page,
        bbox: Rect::from(&image.bbox),
        width: image.width,
        height: image.height,
        rotation: image.rotation,
        format: unsafe { str_field(image.format, where_, "format") }?.unwrap_or_default(),
        duplicate_of: unsafe { str_field(image.duplicate_of, where_, "duplicate_of") }?,
        bytes: std::sync::Arc::new(unsafe { view_bytes(image.bytes, "bytes") }?.to_vec()),
    })
}

fn copy_blocks(arrays: &Arrays<'_>, offset: u32, count: u32) -> FfiResult<Vec<PositionedBlock>> {
    let blocks = sub(arrays.blocks, offset, count, "content", "blocks")?;
    let mut out = Vec::with_capacity(blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        out.push(copy_block(
            block,
            &format!("blocks[{}]", offset as usize + index),
            arrays,
        )?);
    }
    Ok(out)
}

fn copy_block(
    block: &LiteParseLayoutBlock,
    what: &str,
    arrays: &Arrays<'_>,
) -> FfiResult<PositionedBlock> {
    let flags = block.flags;
    known_flags(flags, BLOCK_FLAGS, what, "")?;
    let text = unsafe { str_field(block.text, what, "text") }?;
    let marker = unsafe { str_field(block.marker, what, "marker") }?;
    let lang = unsafe { str_field(block.lang, what, "lang") }?;
    let id = unsafe { str_field(block.id, what, "id") }?;
    let format = unsafe { str_field(block.format, what, "format") }?;
    let bbox = flagged_rect(
        flags,
        LITEPARSE_BLOCK_FLAG_HAS_BBOX,
        &block.bbox,
        what,
        "bbox",
    )?;
    let lines =
        unsafe { copy_strings(arrays, block.line_offset, block.line_count, what, "lines") }?;
    let header = copy_cells(
        arrays,
        block.header_cell_offset,
        block.header_cell_count,
        what,
        "header",
    )?;
    let rows = sub(arrays.rows, block.row_offset, block.row_count, what, "rows")?;
    let mut body = Vec::with_capacity(rows.len());
    for row in rows {
        body.push(copy_cells(
            arrays,
            row.cell_offset,
            row.cell_count,
            what,
            "rows",
        )?);
    }
    let level = has(flags, LITEPARSE_BLOCK_FLAG_HAS_LEVEL).then_some(block.level);
    let bold = has(flags, LITEPARSE_BLOCK_FLAG_BOLD);
    let italic = has(flags, LITEPARSE_BLOCK_FLAG_ITALIC);

    let inner = match block.kind {
        LITEPARSE_BLOCK_HEADING => {
            let level = level.unwrap_or(1);
            if !(1..=6).contains(&level) {
                return Err(FfiError::invalid_argument(format!(
                    "{what} heading level must be 1-6"
                )));
            }
            Block::Heading {
                level: level as u8,
                text: text.unwrap_or_default(),
            }
        }
        LITEPARSE_BLOCK_PARAGRAPH => Block::Paragraph {
            text: text.unwrap_or_default(),
            bold,
            italic,
        },
        LITEPARSE_BLOCK_LIST_ITEM => Block::ListItem {
            ordered: has(flags, LITEPARSE_BLOCK_FLAG_HAS_ORDERED)
                && has(flags, LITEPARSE_BLOCK_FLAG_ORDERED),
            marker: marker.unwrap_or_default(),
            level: u8::try_from(level.unwrap_or(0)).map_err(|_| {
                FfiError::invalid_argument(format!("{what} list level must fit in u8"))
            })?,
            text: text.unwrap_or_default(),
            bold,
            italic,
        },
        LITEPARSE_BLOCK_CODE => Block::CodeBlock { lines, lang },
        LITEPARSE_BLOCK_TABLE => Block::Table {
            header: has(flags, LITEPARSE_BLOCK_FLAG_HAS_HEADER)
                .then(|| header.into_iter().map(OwnedCell::into_cell).collect()),
            rows: body
                .into_iter()
                .map(|row| row.into_iter().map(OwnedCell::into_cell).collect())
                .collect(),
        },
        LITEPARSE_BLOCK_MERGED_TABLE => Block::MergedTable {
            rows: body
                .into_iter()
                .map(|row| row.into_iter().map(OwnedCell::into_span_cell).collect())
                .collect(),
            header_rows: if has(flags, LITEPARSE_BLOCK_FLAG_HAS_HEADER_ROWS) {
                block.header_rows as usize
            } else {
                0
            },
        },
        LITEPARSE_BLOCK_GRID_FALLBACK => Block::GridFallback { lines },
        LITEPARSE_BLOCK_RULE => Block::HorizontalRule,
        LITEPARSE_BLOCK_FIGURE => Block::Figure {
            id: id.unwrap_or_default(),
            format: format.unwrap_or_default(),
        },
        kind => {
            return Err(FfiError::invalid_argument(format!(
                "{what} has unknown kind {kind}"
            )));
        }
    };
    Ok(PositionedBlock::new(inner, bbox))
}

struct OwnedCell {
    text: String,
    bbox: Option<Rect>,
    colspan: u16,
    rowspan: u16,
}

impl OwnedCell {
    fn into_cell(self) -> Cell {
        Cell {
            text: self.text,
            bbox: self.bbox,
        }
    }

    fn into_span_cell(self) -> SpanCell {
        let span = SpanCell::spanning(self.text, self.colspan, self.rowspan);
        match self.bbox {
            Some(bbox) => span.with_bbox(bbox),
            None => span,
        }
    }
}

fn copy_cells(
    arrays: &Arrays<'_>,
    offset: u32,
    count: u32,
    where_: &str,
    field: &str,
) -> FfiResult<Vec<OwnedCell>> {
    let cells = sub(arrays.cells, offset, count, where_, field)?;
    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        known_flags(cell.flags, CELL_FLAGS, where_, field)?;
        let span = |value: u32, axis: &str| {
            u16::try_from(value).map_err(|_| {
                FfiError::invalid_argument(format!("{where_}.{field} {axis} must fit in u16"))
            })
        };
        out.push(OwnedCell {
            text: unsafe { str_field(cell.text, where_, field) }?.unwrap_or_default(),
            bbox: flagged_rect(
                cell.flags,
                LITEPARSE_CELL_FLAG_HAS_BBOX,
                &cell.bbox,
                where_,
                field,
            )?,
            colspan: span(cell.colspan, "colspan")?,
            rowspan: span(cell.rowspan, "rowspan")?,
        });
    }
    Ok(out)
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OwnedContent>();
};
