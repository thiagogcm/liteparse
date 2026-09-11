use liteparse::config::CropBox;
use liteparse::markdown_layout::{Block, Cell, PositionedBlock, SpanCell};
use liteparse::types::{
    GraphicPrimitive, ImageRef, OutlineTarget, Page, Rect, StructNode, TextItem, WordBox,
};

use crate::handle::{
    LiteParseByteView, as_slice, build_handle, optional_str_view, optional_view_str,
    required_view_str, state_ref,
};
use crate::parser::{LiteParseParser, build_parser};
use crate::result::{LiteParseResultNew, ResultState};
use crate::status::{FfiError, FfiResult};
use crate::views::{
    LiteParseImageRef, LiteParseLayoutBlock, LiteParseLayoutCell, LiteParseLayoutRow,
    LiteParseOutlineEntry, LiteParseRect, LiteParseStructNode, LiteParseTextItem, LiteParseWordBox,
};

/// Values for `LiteParseContentGraphic.kind`.
pub const LITEPARSE_GRAPHIC_STROKE: u32 = 0;
pub const LITEPARSE_GRAPHIC_RECT: u32 = 1;

/// One caller-supplied page. Offset/count pairs index the shared arrays on
/// `LiteParseContent`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseContentPage {
    /// 1-based source page number.
    pub page_number: u32,
    pub page_width: f32,
    pub page_height: f32,
    pub item_offset: usize,
    pub item_count: usize,
    pub graphic_offset: usize,
    pub graphic_count: usize,
    pub struct_offset: usize,
    pub struct_count: usize,
    pub image_ref_offset: usize,
    pub image_ref_count: usize,
    pub block_offset: usize,
    pub block_count: usize,
    /// Empty view means the host supplied no `/PageLabels` entry.
    pub page_label: LiteParseByteView,
}

/// A layout graphic in the same viewport space as the page's text items.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseContentGraphic {
    /// `LITEPARSE_GRAPHIC_STROKE` or `LITEPARSE_GRAPHIC_RECT`.
    pub kind: u32,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub bbox: LiteParseRect,
    pub stroke_color: LiteParseByteView,
    pub fill_color: LiteParseByteView,
    pub line_width: f32,
    pub has_fill: bool,
    pub has_stroke: bool,
}

impl LiteParseContentGraphic {
    pub(crate) fn borrow(graphic: &GraphicPrimitive) -> Self {
        match graphic {
            GraphicPrimitive::Stroke {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => Self {
                kind: LITEPARSE_GRAPHIC_STROKE,
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                bbox: LiteParseRect::from(&graphic.bbox()),
                stroke_color: optional_str_view(color.as_deref()),
                fill_color: LiteParseByteView::default(),
                line_width: *width,
                has_fill: false,
                has_stroke: color.is_some(),
            },
            GraphicPrimitive::Rect { bbox, fill, stroke } => Self {
                kind: LITEPARSE_GRAPHIC_RECT,
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
                bbox: LiteParseRect::from(bbox),
                stroke_color: optional_str_view(stroke.as_deref()),
                fill_color: optional_str_view(fill.as_deref()),
                line_width: 0.0,
                has_fill: fill.is_some(),
                has_stroke: stroke.is_some(),
            },
        }
    }
}

/// Packed caller-supplied pages for `liteparse_parser_parse_content`.
///
/// Begin with `liteparse_content_default()`. All views and arrays are copied
/// during the call. Leave the block arrays empty to run projection (and, when
/// configured, classification). Any per-page or document-level block range
/// uses the supplied structure for Markdown instead of classifying.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseContent {
    /// Must equal `sizeof(LiteParseContent)`.
    pub size_of_content: usize,
    pub pages: *const LiteParseContentPage,
    pub pages_len: usize,
    pub items: *const LiteParseTextItem,
    pub items_len: usize,
    pub graphics: *const LiteParseContentGraphic,
    pub graphics_len: usize,
    pub blocks: *const LiteParseLayoutBlock,
    pub blocks_len: usize,
    pub cells: *const LiteParseLayoutCell,
    pub cells_len: usize,
    pub rows: *const LiteParseLayoutRow,
    pub rows_len: usize,
    pub lines: *const LiteParseByteView,
    pub lines_len: usize,
    pub outline: *const LiteParseOutlineEntry,
    pub outline_len: usize,
    pub words: *const LiteParseWordBox,
    pub words_len: usize,
    pub struct_nodes: *const LiteParseStructNode,
    pub struct_nodes_len: usize,
    pub image_refs: *const LiteParseImageRef,
    pub image_refs_len: usize,
    /// Range into `blocks` for document-wide structure (`all_blocks`).
    pub document_block_offset: usize,
    pub document_block_count: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn liteparse_content_default() -> LiteParseContent {
    LiteParseContent {
        size_of_content: size_of::<LiteParseContent>(),
        ..LiteParseContent::default()
    }
}

/// Parse caller-supplied pages. Copies every view during the call.
///
/// Empty block ranges run grid projection (and the configured classifier).
/// Any per-page or document-level block range treats those blocks as
/// authoritative Markdown. `max_pages` truncates the supplied page list on
/// both paths and drops document-level blocks when it does. `crop_box` and
/// `skip_diagonal_text` are applied before projection, matching parse.
///
/// # Safety
///
/// `parser` must be live. `content` and every non-null array/view it names
/// must be readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_parse_content(
    parser: *const LiteParseParser,
    content: *const LiteParseContent,
) -> LiteParseResultNew {
    let (status, handle) = build_handle(|| {
        let parser = unsafe { state_ref(parser) }?;
        let owned = unsafe { owned_content(content) }?;
        let mut pages = owned.pages;
        let mut page_blocks = owned.page_blocks;
        let mut document_blocks = owned.document_blocks;
        let max_pages = parser.config().max_pages;
        let truncated = pages.len() > max_pages;
        if truncated {
            pages.truncate(max_pages);
            page_blocks.truncate(max_pages);
            document_blocks = None;
        }
        apply_content_filters(
            &mut pages,
            parser.config().crop_box.as_ref(),
            parser.config().skip_diagonal_text,
        );
        let core = build_parser(parser.config().clone(), None, parser.glyph_resolver());
        let result =
            if page_blocks.iter().any(|blocks| !blocks.is_empty()) || document_blocks.is_some() {
                let page_stats = vec![None; pages.len()];
                core.parse_from_blocks(
                    pages,
                    page_blocks,
                    document_blocks,
                    owned.outline,
                    Vec::new(),
                    page_stats,
                )?
            } else {
                core.parse_from_pages(pages, owned.outline)
            };
        Ok(ResultState::new(
            result,
            parser.config().extract_text_metadata,
            parser.config().dpi,
            None,
        ))
    });
    LiteParseResultNew { status, handle }
}

struct OwnedContent {
    pages: Vec<Page>,
    page_blocks: Vec<Vec<PositionedBlock>>,
    document_blocks: Option<Vec<PositionedBlock>>,
    outline: Vec<OutlineTarget>,
}

unsafe fn owned_content(raw: *const LiteParseContent) -> FfiResult<OwnedContent> {
    let raw = unsafe { raw.as_ref() }.ok_or_else(|| {
        FfiError::invalid_argument(
            "content must not be null; start from liteparse_content_default()",
        )
    })?;
    if raw.size_of_content != size_of::<LiteParseContent>() {
        return Err(FfiError::invalid_argument(format!(
            "content.size_of_content is {} but this library expects {}; rebuild against the current header",
            raw.size_of_content,
            size_of::<LiteParseContent>()
        )));
    }

    let pages = unsafe { as_slice(raw.pages, raw.pages_len, "pages") }?.unwrap_or_default();
    let items = unsafe { as_slice(raw.items, raw.items_len, "items") }?.unwrap_or_default();
    let graphics =
        unsafe { as_slice(raw.graphics, raw.graphics_len, "graphics") }?.unwrap_or_default();
    let blocks = unsafe { as_slice(raw.blocks, raw.blocks_len, "blocks") }?.unwrap_or_default();
    let cells = unsafe { as_slice(raw.cells, raw.cells_len, "cells") }?.unwrap_or_default();
    let rows = unsafe { as_slice(raw.rows, raw.rows_len, "rows") }?.unwrap_or_default();
    let lines = unsafe { as_slice(raw.lines, raw.lines_len, "lines") }?.unwrap_or_default();
    let outline_in =
        unsafe { as_slice(raw.outline, raw.outline_len, "outline") }?.unwrap_or_default();
    let words = unsafe { as_slice(raw.words, raw.words_len, "words") }?.unwrap_or_default();
    let struct_nodes =
        unsafe { as_slice(raw.struct_nodes, raw.struct_nodes_len, "struct_nodes") }?
            .unwrap_or_default();
    let image_refs =
        unsafe { as_slice(raw.image_refs, raw.image_refs_len, "image_refs") }?.unwrap_or_default();

    let mut owned_pages = Vec::with_capacity(pages.len());
    let mut page_blocks = Vec::with_capacity(pages.len());
    let mut any_page_blocks = false;
    for (index, page) in pages.iter().enumerate() {
        owned_pages.push(unsafe {
            copy_page(page, index, items, graphics, words, struct_nodes, image_refs)
        }?);
        let copied = copy_blocks(
            blocks,
            page.block_offset,
            page.block_count,
            cells,
            rows,
            lines,
        )?;
        any_page_blocks |= !copied.is_empty();
        page_blocks.push(copied);
    }

    let document_blocks = if raw.document_block_count == 0 {
        None
    } else {
        Some(copy_blocks(
            blocks,
            raw.document_block_offset,
            raw.document_block_count,
            cells,
            rows,
            lines,
        )?)
    };
    if document_blocks.is_none() && !any_page_blocks {
        page_blocks.clear();
    }

    let mut outline = Vec::with_capacity(outline_in.len());
    for (index, entry) in outline_in.iter().enumerate() {
        outline.push(unsafe { copy_outline(entry, index) }?);
    }

    Ok(OwnedContent {
        pages: owned_pages,
        page_blocks,
        document_blocks,
        outline,
    })
}

unsafe fn copy_page(
    page: &LiteParseContentPage,
    index: usize,
    items: &[LiteParseTextItem],
    graphics: &[LiteParseContentGraphic],
    words: &[LiteParseWordBox],
    struct_nodes: &[LiteParseStructNode],
    image_refs: &[LiteParseImageRef],
) -> FfiResult<Page> {
    if page.page_number == 0 {
        return Err(FfiError::invalid_argument(format!(
            "pages[{index}].page_number must be 1-based"
        )));
    }
    if !page.page_width.is_finite()
        || !page.page_height.is_finite()
        || page.page_width <= 0.0
        || page.page_height <= 0.0
    {
        return Err(FfiError::invalid_argument(format!(
            "pages[{index}] width and height must be finite and greater than zero"
        )));
    }
    let item_range = slice_range(
        page.item_offset,
        page.item_count,
        items.len(),
        &format!("pages[{index}].items"),
    )?;
    let graphic_range = slice_range(
        page.graphic_offset,
        page.graphic_count,
        graphics.len(),
        &format!("pages[{index}].graphics"),
    )?;
    let struct_range = slice_range(
        page.struct_offset,
        page.struct_count,
        struct_nodes.len(),
        &format!("pages[{index}].struct_nodes"),
    )?;
    let image_ref_range = slice_range(
        page.image_ref_offset,
        page.image_ref_count,
        image_refs.len(),
        &format!("pages[{index}].image_refs"),
    )?;
    let mut text_items = Vec::with_capacity(item_range.len());
    for (item_index, item) in items[item_range].iter().enumerate() {
        text_items.push(unsafe { copy_text_item(item, index, item_index, words) }?);
    }
    let mut copied_graphics = Vec::with_capacity(graphic_range.len());
    for (graphic_index, graphic) in graphics[graphic_range].iter().enumerate() {
        copied_graphics.push(unsafe { copy_graphic(graphic, index, graphic_index) }?);
    }
    let mut copied_structs = Vec::with_capacity(struct_range.len());
    for (struct_index, node) in struct_nodes[struct_range].iter().enumerate() {
        copied_structs.push(unsafe { copy_struct_node(node, index, struct_index) }?);
    }
    let mut copied_image_refs = Vec::with_capacity(image_ref_range.len());
    for (ref_index, image) in image_refs[image_ref_range].iter().enumerate() {
        copied_image_refs.push(unsafe { copy_image_ref(image, index, ref_index) }?);
    }
    let page_label =
        unsafe { optional_view_str(page.page_label, &format!("pages[{index}].page_label")) }?
            .filter(|label| !label.is_empty());
    Ok(Page {
        page_number: page.page_number as usize,
        page_label,
        page_width: page.page_width,
        page_height: page.page_height,
        geometry: None,
        content_bounds: None,
        text_items,
        graphics: copied_graphics,
        vector_graphics: None,
        struct_nodes: copied_structs,
        image_refs: copied_image_refs,
        annotations: None,
        form_fields: None,
        structure_tree: None,
    })
}

unsafe fn copy_text_item(
    item: &LiteParseTextItem,
    page_index: usize,
    item_index: usize,
    words: &[LiteParseWordBox],
) -> FfiResult<TextItem> {
    let where_ = format!("pages[{page_index}].items[{item_index}]");
    if ![item.x, item.y, item.width, item.height, item.rotation]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(FfiError::invalid_argument(format!(
            "{where_} geometry must be finite"
        )));
    }
    let text =
        unsafe { optional_view_str(item.text, &format!("{where_}.text")) }?.unwrap_or_default();
    let char_codes = unsafe { as_slice(item.char_codes, item.char_codes_len, "char_codes") }?
        .unwrap_or_default()
        .to_vec();
    Ok(TextItem {
        text,
        x: item.x,
        y: item.y,
        width: item.width,
        height: item.height,
        rotation: item.rotation,
        font_name: unsafe { optional_view_str(item.font_name, &format!("{where_}.font_name")) }?,
        font_size: flagged(item.has_font_size, item.font_size),
        font_height: flagged(item.has_font_height, item.font_height),
        font_ascent: flagged(item.has_font_ascent, item.font_ascent),
        font_descent: flagged(item.has_font_descent, item.font_descent),
        font_weight: flagged(item.has_font_weight, item.font_weight),
        font_flags: flagged(item.has_font_flags, item.font_flags),
        text_width: flagged(item.has_text_width, item.text_width),
        font_is_buggy: item.has_font_is_buggy && item.font_is_buggy,
        has_unicode_map_error: item.has_unicode_map_error,
        mcid: flagged(item.has_mcid, item.mcid),
        fill_color: unsafe { optional_view_str(item.fill_color, &format!("{where_}.fill_color")) }?,
        stroke_color: unsafe {
            optional_view_str(item.stroke_color, &format!("{where_}.stroke_color"))
        }?,
        char_codes,
        trailing_space_generated: item.has_trailing_space_generated
            && item.trailing_space_generated,
        confidence: flagged(item.has_confidence, item.confidence),
        link: unsafe { optional_view_str(item.link, &format!("{where_}.link")) }?,
        strike: item.strike,
        words: copy_words(item, &where_, words)?,
    })
}

fn copy_words(
    item: &LiteParseTextItem,
    where_: &str,
    words: &[LiteParseWordBox],
) -> FfiResult<Vec<WordBox>> {
    let range = slice_range(
        item.word_offset,
        item.word_count,
        words.len(),
        &format!("{where_}.words"),
    )?;
    let mut copied = Vec::with_capacity(range.len());
    for (word_index, word) in words[range].iter().enumerate() {
        if ![word.x, word.y, word.width, word.height]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(FfiError::invalid_argument(format!(
                "{where_}.words[{word_index}] geometry must be finite"
            )));
        }
        copied.push(WordBox {
            text: unsafe { optional_view_str(word.text, &format!("{where_}.words[{word_index}].text")) }?
                .unwrap_or_default(),
            x: word.x,
            y: word.y,
            width: word.width,
            height: word.height,
        });
    }
    Ok(copied)
}

unsafe fn copy_struct_node(
    node: &LiteParseStructNode,
    page_index: usize,
    struct_index: usize,
) -> FfiResult<StructNode> {
    let where_ = format!("pages[{page_index}].struct_nodes[{struct_index}]");
    if node.has_bbox
        && ![node.bbox.x, node.bbox.y, node.bbox.width, node.bbox.height]
            .iter()
            .all(|v| v.is_finite())
    {
        return Err(FfiError::invalid_argument(format!(
            "{where_} bbox must be finite"
        )));
    }
    let mcids = unsafe { as_slice(node.mcids, node.mcids_len, "mcids") }?
        .unwrap_or_default()
        .to_vec();
    Ok(StructNode {
        role: unsafe { optional_view_str(node.role, &format!("{where_}.role")) }?.unwrap_or_default(),
        mcids,
        bbox: node.has_bbox.then(|| Rect::from(&node.bbox)),
        alt_text: unsafe { optional_view_str(node.alt_text, &format!("{where_}.alt_text")) }?,
    })
}

unsafe fn copy_image_ref(
    image: &LiteParseImageRef,
    page_index: usize,
    ref_index: usize,
) -> FfiResult<ImageRef> {
    let where_ = format!("pages[{page_index}].image_refs[{ref_index}]");
    if ![image.bbox.x, image.bbox.y, image.bbox.width, image.bbox.height, image.rotation]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(FfiError::invalid_argument(format!(
            "{where_} geometry must be finite"
        )));
    }
    Ok(ImageRef {
        id: unsafe { optional_view_str(image.id, &format!("{where_}.id")) }?.unwrap_or_default(),
        bbox: Rect::from(&image.bbox),
        obj_index: image.obj_index,
        format: unsafe { optional_view_str(image.format, &format!("{where_}.format")) }?
            .unwrap_or_default(),
        pixel_width: image.pixel_width,
        pixel_height: image.pixel_height,
        rotation: image.rotation,
        jpeg_bytes: None,
        raw_bytes: None,
        bits_per_pixel: 0,
        colorspace: 0,
    })
}

unsafe fn copy_graphic(
    graphic: &LiteParseContentGraphic,
    page_index: usize,
    graphic_index: usize,
) -> FfiResult<GraphicPrimitive> {
    let where_ = format!("pages[{page_index}].graphics[{graphic_index}]");
    match graphic.kind {
        LITEPARSE_GRAPHIC_STROKE => {
            if ![
                graphic.x1,
                graphic.y1,
                graphic.x2,
                graphic.y2,
                graphic.line_width,
            ]
            .iter()
            .all(|v| v.is_finite())
            {
                return Err(FfiError::invalid_argument(format!(
                    "{where_} stroke geometry must be finite"
                )));
            }
            Ok(GraphicPrimitive::Stroke {
                x1: graphic.x1,
                y1: graphic.y1,
                x2: graphic.x2,
                y2: graphic.y2,
                color: unsafe {
                    optional_view_str(graphic.stroke_color, &format!("{where_}.stroke_color"))
                }?,
                width: graphic.line_width,
            })
        }
        LITEPARSE_GRAPHIC_RECT => {
            if ![
                graphic.bbox.x,
                graphic.bbox.y,
                graphic.bbox.width,
                graphic.bbox.height,
            ]
            .iter()
            .all(|v| v.is_finite())
            {
                return Err(FfiError::invalid_argument(format!(
                    "{where_} rect geometry must be finite"
                )));
            }
            Ok(GraphicPrimitive::Rect {
                bbox: Rect::from(&graphic.bbox),
                fill: if graphic.has_fill {
                    Some(
                        unsafe {
                            optional_view_str(graphic.fill_color, &format!("{where_}.fill_color"))
                        }?
                        .unwrap_or_default(),
                    )
                } else {
                    None
                },
                stroke: if graphic.has_stroke {
                    Some(
                        unsafe {
                            optional_view_str(
                                graphic.stroke_color,
                                &format!("{where_}.stroke_color"),
                            )
                        }?
                        .unwrap_or_default(),
                    )
                } else {
                    None
                },
            })
        }
        kind => Err(FfiError::invalid_argument(format!(
            "{where_} has unknown graphic kind {kind}"
        ))),
    }
}

unsafe fn copy_outline(entry: &LiteParseOutlineEntry, index: usize) -> FfiResult<OutlineTarget> {
    Ok(OutlineTarget {
        level: entry.level,
        title: unsafe { optional_view_str(entry.title, &format!("outline[{index}].title")) }?
            .unwrap_or_default(),
        page_index: entry.page_index,
        y_pdf: flagged(entry.has_y_pdf, entry.y_pdf),
    })
}

fn copy_blocks(
    blocks: &[LiteParseLayoutBlock],
    offset: usize,
    count: usize,
    cells: &[LiteParseLayoutCell],
    rows: &[LiteParseLayoutRow],
    lines: &[LiteParseByteView],
) -> FfiResult<Vec<PositionedBlock>> {
    let range = slice_range(offset, count, blocks.len(), "blocks")?;
    let mut out = Vec::with_capacity(range.len());
    for (index, block) in blocks[range].iter().enumerate() {
        out.push(copy_block(block, offset + index, cells, rows, lines)?);
    }
    Ok(out)
}

fn copy_block(
    block: &LiteParseLayoutBlock,
    index: usize,
    cells: &[LiteParseLayoutCell],
    rows: &[LiteParseLayoutRow],
    lines: &[LiteParseByteView],
) -> FfiResult<PositionedBlock> {
    let kind = unsafe { required_view_str(block.kind, &format!("blocks[{index}].kind")) }?;
    let text = unsafe { optional_view_str(block.text, &format!("blocks[{index}].text")) }?;
    let marker = unsafe { optional_view_str(block.marker, &format!("blocks[{index}].marker")) }?;
    let lang = unsafe { optional_view_str(block.lang, &format!("blocks[{index}].lang")) }?;
    let id = unsafe { optional_view_str(block.id, &format!("blocks[{index}].id")) }?;
    let format = unsafe { optional_view_str(block.format, &format!("blocks[{index}].format")) }?;
    let bbox = block.has_bbox.then(|| Rect::from(&block.bbox));
    if block.has_bbox
        && ![
            block.bbox.x,
            block.bbox.y,
            block.bbox.width,
            block.bbox.height,
        ]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(FfiError::invalid_argument(format!(
            "blocks[{index}].bbox must be finite"
        )));
    }

    let copied_lines = copy_lines(block, index, lines)?;
    let header = copy_cells(
        cells,
        block.header_cell_offset,
        block.header_cell_count,
        &format!("blocks[{index}].header"),
    )?;
    let body = copy_rows(block, index, rows, cells)?;

    let inner = match kind.as_str() {
        "heading" => {
            let level = if block.has_level { block.level } else { 1 };
            if !(1..=6).contains(&level) {
                return Err(FfiError::invalid_argument(format!(
                    "blocks[{index}] heading level must be 1-6"
                )));
            }
            Block::Heading {
                level,
                text: text.unwrap_or_default(),
            }
        }
        "paragraph" => Block::Paragraph {
            text: text.unwrap_or_default(),
            bold: block.bold,
            italic: block.italic,
        },
        "list_item" => Block::ListItem {
            ordered: block.has_ordered && block.ordered,
            marker: marker.unwrap_or_default(),
            level: if block.has_level { block.level } else { 0 },
            text: text.unwrap_or_default(),
            bold: block.bold,
            italic: block.italic,
        },
        "code" => Block::CodeBlock {
            lines: copied_lines,
            lang,
        },
        "table" => Block::Table {
            header: (!header.is_empty()).then_some(header.iter().map(to_cell).collect()),
            rows: body
                .iter()
                .map(|row| row.iter().map(to_cell).collect())
                .collect(),
        },
        "merged_table" => Block::MergedTable {
            rows: body
                .iter()
                .map(|row| row.iter().map(to_span_cell).collect())
                .collect(),
            header_rows: if block.has_header_rows {
                block.header_rows
            } else {
                0
            },
        },
        "grid_fallback" => Block::GridFallback {
            lines: copied_lines,
        },
        "rule" => Block::HorizontalRule,
        "figure" => Block::Figure {
            id: id.unwrap_or_default(),
            format: format.unwrap_or_default(),
        },
        other => {
            return Err(FfiError::invalid_argument(format!(
                "blocks[{index}] has unknown kind '{other}'"
            )));
        }
    };
    Ok(PositionedBlock::new(inner, bbox))
}

fn copy_lines(
    block: &LiteParseLayoutBlock,
    index: usize,
    lines: &[LiteParseByteView],
) -> FfiResult<Vec<String>> {
    let range = slice_range(
        block.line_offset,
        block.line_count,
        lines.len(),
        &format!("blocks[{index}].lines"),
    )?;
    let mut out = Vec::with_capacity(range.len());
    for (line_index, line) in lines[range].iter().enumerate() {
        out.push(
            unsafe { optional_view_str(*line, &format!("blocks[{index}].lines[{line_index}]")) }?
                .unwrap_or_default(),
        );
    }
    Ok(out)
}

fn copy_rows(
    block: &LiteParseLayoutBlock,
    index: usize,
    rows: &[LiteParseLayoutRow],
    cells: &[LiteParseLayoutCell],
) -> FfiResult<Vec<Vec<OwnedCell>>> {
    let range = slice_range(
        block.first_row,
        block.row_count,
        rows.len(),
        &format!("blocks[{index}].rows"),
    )?;
    let mut out = Vec::with_capacity(range.len());
    for (row_index, row) in rows[range].iter().enumerate() {
        out.push(copy_cells(
            cells,
            row.cell_offset,
            row.cell_count,
            &format!("blocks[{index}].rows[{row_index}]"),
        )?);
    }
    Ok(out)
}

fn copy_cells(
    cells: &[LiteParseLayoutCell],
    offset: usize,
    count: usize,
    name: &str,
) -> FfiResult<Vec<OwnedCell>> {
    let range = slice_range(offset, count, cells.len(), name)?;
    let mut out = Vec::with_capacity(range.len());
    for (cell_index, cell) in cells[range].iter().enumerate() {
        if cell.has_bbox
            && ![cell.bbox.x, cell.bbox.y, cell.bbox.width, cell.bbox.height]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err(FfiError::invalid_argument(format!(
                "{name}[{cell_index}].bbox must be finite"
            )));
        }
        out.push(OwnedCell {
            text: unsafe { optional_view_str(cell.text, &format!("{name}[{cell_index}].text")) }?
                .unwrap_or_default(),
            bbox: cell.has_bbox.then(|| Rect::from(&cell.bbox)),
            colspan: cell.colspan,
            rowspan: cell.rowspan,
        });
    }
    Ok(out)
}

struct OwnedCell {
    text: String,
    bbox: Option<Rect>,
    colspan: u16,
    rowspan: u16,
}

fn to_cell(cell: &OwnedCell) -> Cell {
    Cell {
        text: cell.text.clone(),
        bbox: cell.bbox.clone(),
    }
}

fn to_span_cell(cell: &OwnedCell) -> SpanCell {
    let mut span = SpanCell::spanning(cell.text.clone(), cell.colspan, cell.rowspan);
    if let Some(bbox) = cell.bbox.clone() {
        span = span.with_bbox(bbox);
    }
    span
}

fn slice_range(
    offset: usize,
    count: usize,
    len: usize,
    name: &str,
) -> FfiResult<std::ops::Range<usize>> {
    if count == 0 {
        return Ok(0..0);
    }
    offset
        .checked_add(count)
        .filter(|end| *end <= len)
        .map(|end| offset..end)
        .ok_or_else(|| {
            FfiError::invalid_argument(format!("{name} range {offset}+{count} is outside 0..{len}"))
        })
}

fn flagged<T: Copy>(has: bool, value: T) -> Option<T> {
    has.then_some(value)
}

/// Same retain rules as core `apply_content_filters`, applied here because
/// `parse_from_pages` does not run them.
fn apply_content_filters(pages: &mut [Page], crop_box: Option<&CropBox>, skip_diagonal: bool) {
    if crop_box.is_none() && !skip_diagonal {
        return;
    }
    for page in pages.iter_mut() {
        if skip_diagonal {
            page.text_items
                .retain(|item| !is_diagonal_rotation(item.rotation));
        }
        if let Some(crop) = crop_box {
            let width = page.page_width;
            let height = page.page_height;
            let min_x = crop.left * width;
            let max_x = (1.0 - crop.right) * width;
            let min_y = crop.top * height;
            let max_y = (1.0 - crop.bottom) * height;
            page.text_items.retain(|item| {
                item.x >= min_x
                    && item.x + item.width <= max_x
                    && item.y >= min_y
                    && item.y + item.height <= max_y
            });
        }
    }
}

fn is_diagonal_rotation(rotation: f32) -> bool {
    let nearest_right_angle = (rotation / 90.0).round() * 90.0;
    (rotation - nearest_right_angle).abs() > 2.0
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OwnedContent>();
};
