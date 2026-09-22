//! Packs core pages into the flat record arrays behind `LiteParseContent`
//! and `LiteParseResultView`. Strings are copied into the handle's pool;
//! binary payloads are borrowed from the core values, which the owning
//! handle keeps alive.

use liteparse::layout::LayoutBlock;
use liteparse::ocr_merge::PageComplexityStats;
use liteparse::types::{
    CutAxis, DocumentAnnotation, FormField, GraphicPrimitive, ImageRef, Page, ParsedPage,
    ProjectedLine, Rect, Region, RegionKind, StructNode, StructureTree, StructureTreeElement,
    TextItem, VectorGraphics,
};

use crate::content::LiteParseContent;
use crate::handle::{LiteParseStr, Pool, array_ptr, packed_len};
use crate::records::*;

/// Every flat array a result or content view can reference. Vectors are
/// never modified after packing, so views may hold pointers into them.
#[derive(Default)]
pub(crate) struct Packed {
    pub pool: Pool,
    pub pages: Vec<LiteParsePage>,
    pub items: Vec<LiteParseTextItem>,
    pub words: Vec<LiteParseWordBox>,
    pub char_codes: Vec<u32>,
    pub graphics: Vec<LiteParseGraphic>,
    pub struct_nodes: Vec<LiteParseStructNode>,
    pub mcids: Vec<i32>,
    pub image_refs: Vec<LiteParseImageRef>,
    pub images: Vec<LiteParseImage>,
    pub annotations: Vec<LiteParseAnnotation>,
    pub quadpoints: Vec<LiteParseRect>,
    pub form_fields: Vec<LiteParseFormField>,
    pub strings: Vec<LiteParseStr>,
    pub structure_nodes: Vec<LiteParseStructureNode>,
    pub structure_attributes: Vec<LiteParseStructureAttribute>,
    pub blocks: Vec<LiteParseLayoutBlock>,
    pub cells: Vec<LiteParseLayoutCell>,
    pub rows: Vec<LiteParseLayoutRow>,
    pub vector_shapes: Vec<LiteParseVectorShape>,
    pub vector_lines: Vec<LiteParseVectorLine>,
    pub outline: Vec<LiteParseOutlineEntry>,
    pub page_errors: Vec<LiteParsePageError>,
    // Result-only arrays.
    pub figures: Vec<LiteParseRect>,
    pub item_frames: Vec<LiteParseItemFrame>,
    pub projected_lines: Vec<LiteParseProjectedLine>,
    pub projected_spans: Vec<LiteParseTextItem>,
    pub region_paths: Vec<u16>,
    pub regions: Vec<LiteParseProjectedRegion>,
    pub region_children: Vec<u32>,
}

/// The page fields shared by extraction `Page` and `ParsedPage`.
pub(crate) struct PageParts<'a> {
    pub page_number: usize,
    pub page_label: Option<&'a str>,
    pub width: f32,
    pub height: f32,
    pub content_bounds: Option<&'a Rect>,
    pub text: &'a str,
    pub markdown: &'a str,
    pub text_items: &'a [TextItem],
    pub graphics: &'a [GraphicPrimitive],
    pub vector_graphics: Option<&'a VectorGraphics>,
    pub struct_nodes: &'a [StructNode],
    pub image_refs: &'a [ImageRef],
    pub annotations: Option<&'a [DocumentAnnotation]>,
    pub form_fields: Option<&'a [FormField]>,
    pub structure_tree: Option<&'a StructureTree>,
    pub blocks: Option<&'a [LayoutBlock]>,
    pub complexity: Option<&'a PageComplexityStats>,
    pub figures: &'a [Rect],
    pub item_frames: &'a [(Rect, Rect)],
    pub projected_lines: &'a [ProjectedLine],
    pub regions: Option<&'a Region>,
    /// Geometry and whether its rotation was reportable.
    pub geometry: Option<(LiteParsePageGeometry, bool)>,
}

impl<'a> PageParts<'a> {
    pub(crate) fn extracted(
        page: &'a Page,
        geometry: Option<(LiteParsePageGeometry, bool)>,
    ) -> Self {
        Self {
            page_number: page.page_number,
            page_label: page.page_label.as_deref(),
            width: page.page_width,
            height: page.page_height,
            content_bounds: page.content_bounds.as_ref(),
            text: "",
            markdown: "",
            text_items: &page.text_items,
            graphics: &page.graphics,
            vector_graphics: page.vector_graphics.as_ref(),
            struct_nodes: &page.struct_nodes,
            image_refs: &page.image_refs,
            annotations: page.annotations.as_deref(),
            form_fields: page.form_fields.as_deref(),
            structure_tree: page.structure_tree.as_ref(),
            blocks: None,
            complexity: None,
            figures: &[],
            item_frames: &[],
            projected_lines: &[],
            regions: None,
            geometry,
        }
    }

    pub(crate) fn parsed(
        page: &'a ParsedPage,
        geometry: Option<(LiteParsePageGeometry, bool)>,
    ) -> Self {
        Self {
            page_number: page.page_number,
            page_label: page.page_label.as_deref(),
            width: page.page_width,
            height: page.page_height,
            content_bounds: page.content_bounds.as_ref(),
            text: &page.text,
            markdown: &page.markdown,
            text_items: &page.text_items,
            graphics: &page.graphics,
            vector_graphics: page.vector_graphics.as_ref(),
            struct_nodes: &page.struct_nodes,
            image_refs: &page.image_refs,
            annotations: page.annotations.as_deref(),
            form_fields: page.form_fields.as_deref(),
            structure_tree: page.structure_tree.as_ref(),
            blocks: page.blocks.as_deref(),
            complexity: page.complexity.as_ref(),
            figures: &page.figures,
            item_frames: &page.projected_item_frames,
            projected_lines: &page.projected_lines,
            regions: Some(&page.regions),
            geometry,
        }
    }
}

impl Packed {
    pub(crate) fn push_page(&mut self, page: PageParts<'_>) {
        let item_offset = self.items.len();
        for item in page.text_items {
            let packed = self.text_item(item);
            self.items.push(packed);
        }
        let graphic_offset = self.graphics.len();
        self.graphics.extend(page.graphics.iter().map(graphic));
        let struct_node_offset = self.struct_nodes.len();
        for node in page.struct_nodes {
            let mcid_offset = self.mcids.len();
            self.mcids.extend_from_slice(&node.mcids);
            let (bbox, has_bbox) = optional_rect(node.bbox.as_ref());
            self.struct_nodes.push(LiteParseStructNode {
                role: self.pool.push(&node.role),
                alt_text: self.pool.push_opt(node.alt_text.as_deref()),
                bbox,
                mcid_offset: packed_len(mcid_offset),
                mcid_count: packed_len(node.mcids.len()),
                flags: flag_bits(&[(has_bbox, LITEPARSE_STRUCT_NODE_FLAG_HAS_BBOX)]),
            });
        }
        let image_ref_offset = self.image_refs.len();
        for image in page.image_refs {
            let packed = LiteParseImageRef::pack(&mut self.pool, image);
            self.image_refs.push(packed);
        }
        let annotation_offset = self.annotations.len();
        for annotation in page.annotations.unwrap_or_default() {
            let packed = self.annotation(annotation);
            self.annotations.push(packed);
        }
        // Structure nodes append their own annotations to the same array.
        let annotation_count = self.annotations.len() - annotation_offset;
        let form_field_offset = self.form_fields.len();
        for field in page.form_fields.unwrap_or_default() {
            let option_offset = self.strings.len();
            for option in &field.options {
                let packed = self.pool.push(option);
                self.strings.push(packed);
            }
            let selected_offset = self.strings.len();
            for option in &field.selected_options {
                let packed = self.pool.push(option);
                self.strings.push(packed);
            }
            let packed = LiteParseFormField::pack(
                &mut self.pool,
                field,
                packed_len(option_offset),
                packed_len(selected_offset),
            );
            self.form_fields.push(packed);
        }
        let structure_node_offset = self.structure_nodes.len();
        if let Some(tree) = page.structure_tree {
            for root in &tree.roots {
                self.structure_element(root, LITEPARSE_NO_PARENT, 0);
            }
        }
        let block_offset = self.blocks.len();
        for block in page.blocks.unwrap_or_default() {
            self.block(block);
        }
        let vector_shape_offset = self.vector_shapes.len();
        let vector_line_offset = self.vector_lines.len();
        if let Some(vectors) = page.vector_graphics {
            self.vector_shapes
                .extend(vectors.shapes.iter().map(LiteParseVectorShape::from));
            self.vector_lines
                .extend(vectors.lines.iter().map(LiteParseVectorLine::from));
        }
        let figure_offset = self.figures.len();
        self.figures
            .extend(page.figures.iter().map(LiteParseRect::from));
        let item_frame_offset = self.item_frames.len();
        self.item_frames
            .extend(
                page.item_frames
                    .iter()
                    .map(|(projected, original)| LiteParseItemFrame {
                        projected: LiteParseRect::from(projected),
                        original: LiteParseRect::from(original),
                    }),
            );
        let projected_line_offset = self.projected_lines.len();
        for line in page.projected_lines {
            self.projected_line(line);
        }
        let region_offset = self.regions.len();
        if let Some(region) = page.regions {
            self.region(region, LITEPARSE_NO_PARENT);
        }

        let (content_bounds, has_content_bounds) = optional_rect(page.content_bounds);
        let (geometry, has_rotation) = page.geometry.unwrap_or_default();
        let flags = flag_bits(&[
            (page.geometry.is_some(), LITEPARSE_PAGE_FLAG_HAS_GEOMETRY),
            (has_rotation, LITEPARSE_PAGE_FLAG_HAS_ROTATION),
            (has_content_bounds, LITEPARSE_PAGE_FLAG_HAS_CONTENT_BOUNDS),
            (
                page.complexity.is_some(),
                LITEPARSE_PAGE_FLAG_HAS_COMPLEXITY,
            ),
            (
                page.annotations.is_some(),
                LITEPARSE_PAGE_FLAG_HAS_ANNOTATIONS,
            ),
            (
                page.form_fields.is_some(),
                LITEPARSE_PAGE_FLAG_HAS_FORM_FIELDS,
            ),
            (
                page.structure_tree.is_some(),
                LITEPARSE_PAGE_FLAG_HAS_STRUCTURE_TREE,
            ),
            (page.blocks.is_some(), LITEPARSE_PAGE_FLAG_HAS_BLOCKS),
            (
                page.vector_graphics.is_some(),
                LITEPARSE_PAGE_FLAG_HAS_VECTOR_GRAPHICS,
            ),
        ]);
        self.pages.push(LiteParsePage {
            page_number: clamp_u32(page.page_number),
            flags,
            width: page.width,
            height: page.height,
            label: self.pool.push_opt(page.page_label),
            text: self.pool.push(page.text),
            markdown: self.pool.push(page.markdown),
            geometry,
            content_bounds,
            complexity: page
                .complexity
                .map(LiteParsePageComplexity::from)
                .unwrap_or_default(),
            item_offset: packed_len(item_offset),
            item_count: packed_len(self.items.len() - item_offset),
            graphic_offset: packed_len(graphic_offset),
            graphic_count: packed_len(self.graphics.len() - graphic_offset),
            struct_node_offset: packed_len(struct_node_offset),
            struct_node_count: packed_len(self.struct_nodes.len() - struct_node_offset),
            image_ref_offset: packed_len(image_ref_offset),
            image_ref_count: packed_len(self.image_refs.len() - image_ref_offset),
            annotation_offset: packed_len(annotation_offset),
            annotation_count: packed_len(annotation_count),
            form_field_offset: packed_len(form_field_offset),
            form_field_count: packed_len(self.form_fields.len() - form_field_offset),
            structure_node_offset: packed_len(structure_node_offset),
            structure_node_count: packed_len(self.structure_nodes.len() - structure_node_offset),
            block_offset: packed_len(block_offset),
            block_count: packed_len(self.blocks.len() - block_offset),
            vector_shape_offset: packed_len(vector_shape_offset),
            vector_shape_count: packed_len(self.vector_shapes.len() - vector_shape_offset),
            vector_line_offset: packed_len(vector_line_offset),
            vector_line_count: packed_len(self.vector_lines.len() - vector_line_offset),
            figure_offset: packed_len(figure_offset),
            figure_count: packed_len(self.figures.len() - figure_offset),
            item_frame_offset: packed_len(item_frame_offset),
            item_frame_count: packed_len(self.item_frames.len() - item_frame_offset),
            projected_line_offset: packed_len(projected_line_offset),
            projected_line_count: packed_len(self.projected_lines.len() - projected_line_offset),
            region_offset: packed_len(region_offset),
            region_count: packed_len(self.regions.len() - region_offset),
        });
    }

    /// Pack one text item, appending its words and char codes.
    pub(crate) fn text_item(&mut self, item: &TextItem) -> LiteParseTextItem {
        let word_offset = self.words.len();
        for word in &item.words {
            let packed = LiteParseWordBox::pack(&mut self.pool, word);
            self.words.push(packed);
        }
        let char_code_offset = self.char_codes.len();
        self.char_codes.extend_from_slice(&item.char_codes);
        let (font_size, has_font_size) = optional(item.font_size);
        let (confidence, has_confidence) = optional(item.confidence);
        let (font_flags, has_font_flags) = optional(item.font_flags);
        let (font_height, has_font_height) = optional(item.font_height);
        let (font_ascent, has_font_ascent) = optional(item.font_ascent);
        let (font_descent, has_font_descent) = optional(item.font_descent);
        let (font_weight, has_font_weight) = optional(item.font_weight);
        let (text_width, has_text_width) = optional(item.text_width);
        let (mcid, has_mcid) = optional(item.mcid);
        let (fill_color, has_fill_color) = optional_color(item.fill_color.as_deref());
        let (stroke_color, has_stroke_color) = optional_color(item.stroke_color.as_deref());
        LiteParseTextItem {
            text: self.pool.push(&item.text),
            font_name: self.pool.push_opt(item.font_name.as_deref()),
            link: self.pool.push_opt(item.link.as_deref()),
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            rotation: item.rotation,
            font_size,
            confidence,
            font_height,
            font_ascent,
            font_descent,
            text_width,
            font_flags,
            font_weight,
            mcid,
            fill_color,
            stroke_color,
            char_code_offset: packed_len(char_code_offset),
            char_code_count: packed_len(item.char_codes.len()),
            word_offset: packed_len(word_offset),
            word_count: packed_len(item.words.len()),
            flags: flag_bits(&[
                (has_font_size, LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_SIZE),
                (has_confidence, LITEPARSE_TEXT_ITEM_FLAG_HAS_CONFIDENCE),
                (has_font_flags, LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_FLAGS),
                (has_font_height, LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_HEIGHT),
                (has_font_ascent, LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_ASCENT),
                (has_font_descent, LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_DESCENT),
                (has_font_weight, LITEPARSE_TEXT_ITEM_FLAG_HAS_FONT_WEIGHT),
                (has_text_width, LITEPARSE_TEXT_ITEM_FLAG_HAS_TEXT_WIDTH),
                (has_mcid, LITEPARSE_TEXT_ITEM_FLAG_HAS_MCID),
                (has_fill_color, LITEPARSE_TEXT_ITEM_FLAG_HAS_FILL_COLOR),
                (has_stroke_color, LITEPARSE_TEXT_ITEM_FLAG_HAS_STROKE_COLOR),
                (item.strike, LITEPARSE_TEXT_ITEM_FLAG_STRIKE),
                (
                    item.has_unicode_map_error,
                    LITEPARSE_TEXT_ITEM_FLAG_UNICODE_MAP_ERROR,
                ),
                (item.font_is_buggy, LITEPARSE_TEXT_ITEM_FLAG_FONT_IS_BUGGY),
                (
                    item.trailing_space_generated,
                    LITEPARSE_TEXT_ITEM_FLAG_TRAILING_SPACE_GENERATED,
                ),
            ]),
        }
    }

    pub(crate) fn annotation(&mut self, annotation: &DocumentAnnotation) -> LiteParseAnnotation {
        let quadpoint_offset = self.quadpoints.len();
        self.quadpoints
            .extend(annotation.quadpoint_rects.iter().map(LiteParseRect::from));
        LiteParseAnnotation::pack(&mut self.pool, annotation, packed_len(quadpoint_offset))
    }

    fn structure_element(&mut self, element: &StructureTreeElement, parent_index: u32, depth: u32) {
        let node_index = self.structure_nodes.len();
        let attribute_offset = self.structure_attributes.len();
        for (name, value) in &element.attributes {
            let packed = structure_attribute(&mut self.pool, name, value);
            self.structure_attributes.push(packed);
        }
        let annotation_offset = self.annotations.len();
        for annotation in &element.annotations {
            let packed = self.annotation(annotation);
            self.annotations.push(packed);
        }
        let mcid_offset = self.mcids.len();
        self.mcids.extend_from_slice(&element.marked_content_ids);
        self.structure_nodes.push(LiteParseStructureNode {
            element_type: self.pool.push(&element.element_type),
            id: self.pool.push_opt(element.id.as_deref()),
            actual_text: self.pool.push_opt(element.actual_text.as_deref()),
            alt_text: self.pool.push_opt(element.alt_text.as_deref()),
            title: self.pool.push_opt(element.title.as_deref()),
            parent_index,
            depth,
            mcid_offset: packed_len(mcid_offset),
            mcid_count: packed_len(element.marked_content_ids.len()),
            attribute_offset: packed_len(attribute_offset),
            attribute_count: packed_len(element.attributes.len()),
            annotation_offset: packed_len(annotation_offset),
            annotation_count: packed_len(element.annotations.len()),
        });
        for child in &element.children {
            self.structure_element(child, packed_len(node_index), depth + 1);
        }
    }

    fn block(&mut self, block: &LayoutBlock) {
        let header_cell_offset = self.cells.len();
        for cell in block.header.iter().flatten() {
            let packed = layout_cell(&mut self.pool, cell);
            self.cells.push(packed);
        }
        let header_cell_count = self.cells.len() - header_cell_offset;
        let row_offset = self.rows.len();
        for row in block.rows.iter().flatten() {
            let cell_offset = self.cells.len();
            for cell in row {
                let packed = layout_cell(&mut self.pool, cell);
                self.cells.push(packed);
            }
            self.rows.push(LiteParseLayoutRow {
                cell_offset: packed_len(cell_offset),
                cell_count: packed_len(row.len()),
            });
        }
        let row_count = self.rows.len() - row_offset;
        let line_offset = self.strings.len();
        for line in block.lines.iter().flatten() {
            let packed = self.pool.push(line);
            self.strings.push(packed);
        }
        let line_count = self.strings.len() - line_offset;
        let (level, has_level) = optional(block.level);
        let (header_rows, has_header_rows) = optional(block.header_rows);
        let (bbox, has_bbox) = optional_rect(block.bbox.as_ref());
        self.blocks.push(LiteParseLayoutBlock {
            text: self.pool.push_opt(block.text.as_deref()),
            marker: self.pool.push_opt(block.marker.as_deref()),
            lang: self.pool.push_opt(block.lang.as_deref()),
            id: self.pool.push_opt(block.id.as_deref()),
            format: self.pool.push_opt(block.format.as_deref()),
            bbox,
            kind: block_kind(block.kind).unwrap_or(LITEPARSE_BLOCK_UNKNOWN),
            flags: flag_bits(&[
                (has_level, LITEPARSE_BLOCK_FLAG_HAS_LEVEL),
                (block.bold, LITEPARSE_BLOCK_FLAG_BOLD),
                (block.italic, LITEPARSE_BLOCK_FLAG_ITALIC),
                (block.ordered.is_some(), LITEPARSE_BLOCK_FLAG_HAS_ORDERED),
                (block.ordered == Some(true), LITEPARSE_BLOCK_FLAG_ORDERED),
                (has_bbox, LITEPARSE_BLOCK_FLAG_HAS_BBOX),
                (has_header_rows, LITEPARSE_BLOCK_FLAG_HAS_HEADER_ROWS),
                (block.header.is_some(), LITEPARSE_BLOCK_FLAG_HAS_HEADER),
            ]),
            level: u32::from(level),
            line_offset: packed_len(line_offset),
            line_count: packed_len(line_count),
            header_cell_offset: packed_len(header_cell_offset),
            header_cell_count: packed_len(header_cell_count),
            row_offset: packed_len(row_offset),
            row_count: packed_len(row_count),
            header_rows: clamp_u32(header_rows),
        });
    }

    fn projected_line(&mut self, line: &ProjectedLine) {
        let span_offset = self.projected_spans.len();
        for span in &line.spans {
            let packed = self.text_item(span);
            self.projected_spans.push(packed);
        }
        let region_path_offset = self.region_paths.len();
        self.region_paths.extend_from_slice(&line.region_path);
        let (heading_font_size, has_heading_font_size) = optional(line.heading_font_size);
        let (mcid, has_mcid) = optional(line.mcid);
        self.projected_lines.push(LiteParseProjectedLine {
            text: self.pool.push(&line.text),
            dominant_font_name: self.pool.push_opt(line.dominant_font_name.as_deref()),
            bbox: LiteParseRect::from(&line.bbox),
            indent_x: line.indent_x,
            dominant_font_size: line.dominant_font_size,
            heading_font_size,
            mcid,
            anchor: anchor_value(&line.anchor),
            flags: flag_bits(&[
                (
                    has_heading_font_size,
                    LITEPARSE_LINE_FLAG_HAS_HEADING_FONT_SIZE,
                ),
                (has_mcid, LITEPARSE_LINE_FLAG_HAS_MCID),
                (line.all_bold, LITEPARSE_LINE_FLAG_ALL_BOLD),
                (line.all_italic, LITEPARSE_LINE_FLAG_ALL_ITALIC),
                (line.all_mono, LITEPARSE_LINE_FLAG_ALL_MONO),
                (line.all_strike, LITEPARSE_LINE_FLAG_ALL_STRIKE),
                (
                    line.font_size_is_estimated,
                    LITEPARSE_LINE_FLAG_FONT_SIZE_ESTIMATED,
                ),
                (line.rtl, LITEPARSE_LINE_FLAG_RTL),
                (line.in_figure, LITEPARSE_LINE_FLAG_IN_FIGURE),
            ]),
            span_offset: packed_len(span_offset),
            span_count: packed_len(line.spans.len()),
            region_path_offset: packed_len(region_path_offset),
            region_path_count: packed_len(line.region_path.len()),
        });
    }

    fn region(&mut self, region: &Region, parent_index: u32) -> u32 {
        let node_index = packed_len(self.regions.len());
        self.regions.push(LiteParseProjectedRegion {
            bbox: LiteParseRect::from(&region.bbox),
            parent_index,
            ..Default::default()
        });
        let (flags, child_indices) = match &region.kind {
            RegionKind::Leaf { .. } => (LITEPARSE_REGION_FLAG_LEAF, Vec::new()),
            RegionKind::Split { axis, children } => {
                let child_indices: Vec<u32> = children
                    .iter()
                    .map(|child| self.region(child, node_index))
                    .collect();
                let axis_flag = match axis {
                    CutAxis::Horizontal => LITEPARSE_REGION_FLAG_HORIZONTAL,
                    CutAxis::Vertical => LITEPARSE_REGION_FLAG_VERTICAL,
                };
                (LITEPARSE_REGION_FLAG_SPLIT | axis_flag, child_indices)
            }
        };
        // Appended after the recursion so a node's children stay contiguous
        // even when its descendants append their own children.
        let child_offset = self.region_children.len();
        self.region_children.extend_from_slice(&child_indices);
        let node = &mut self.regions[node_index as usize];
        node.child_offset = packed_len(child_offset);
        node.child_count = packed_len(child_indices.len());
        node.flags = flags;
        node_index
    }

    /// The content view over the packed arrays. Valid while `self` lives
    /// unmoved on the heap and unmodified.
    pub(crate) fn content_view(&self) -> LiteParseContent {
        LiteParseContent {
            size_of_content: size_of::<LiteParseContent>(),
            pool: self.pool.ptr(),
            pool_len: self.pool.len(),
            pages: array_ptr(&self.pages),
            pages_len: self.pages.len(),
            items: array_ptr(&self.items),
            items_len: self.items.len(),
            words: array_ptr(&self.words),
            words_len: self.words.len(),
            char_codes: array_ptr(&self.char_codes),
            char_codes_len: self.char_codes.len(),
            graphics: array_ptr(&self.graphics),
            graphics_len: self.graphics.len(),
            struct_nodes: array_ptr(&self.struct_nodes),
            struct_nodes_len: self.struct_nodes.len(),
            mcids: array_ptr(&self.mcids),
            mcids_len: self.mcids.len(),
            image_refs: array_ptr(&self.image_refs),
            image_refs_len: self.image_refs.len(),
            images: array_ptr(&self.images),
            images_len: self.images.len(),
            annotations: array_ptr(&self.annotations),
            annotations_len: self.annotations.len(),
            quadpoints: array_ptr(&self.quadpoints),
            quadpoints_len: self.quadpoints.len(),
            form_fields: array_ptr(&self.form_fields),
            form_fields_len: self.form_fields.len(),
            strings: array_ptr(&self.strings),
            strings_len: self.strings.len(),
            structure_nodes: array_ptr(&self.structure_nodes),
            structure_nodes_len: self.structure_nodes.len(),
            structure_attributes: array_ptr(&self.structure_attributes),
            structure_attributes_len: self.structure_attributes.len(),
            blocks: array_ptr(&self.blocks),
            blocks_len: self.blocks.len(),
            cells: array_ptr(&self.cells),
            cells_len: self.cells.len(),
            rows: array_ptr(&self.rows),
            rows_len: self.rows.len(),
            vector_shapes: array_ptr(&self.vector_shapes),
            vector_shapes_len: self.vector_shapes.len(),
            vector_lines: array_ptr(&self.vector_lines),
            vector_lines_len: self.vector_lines.len(),
            outline: array_ptr(&self.outline),
            outline_len: self.outline.len(),
            page_errors: array_ptr(&self.page_errors),
            page_errors_len: self.page_errors.len(),
            document_block_offset: 0,
            document_block_count: 0,
        }
    }
}

pub(crate) fn graphic(graphic: &GraphicPrimitive) -> LiteParseGraphic {
    match graphic {
        GraphicPrimitive::Stroke {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            let (stroke_color, has_stroke) = optional_color(color.as_deref());
            LiteParseGraphic {
                kind: LITEPARSE_GRAPHIC_STROKE,
                flags: flag_bits(&[(has_stroke, LITEPARSE_GRAPHIC_FLAG_HAS_STROKE_COLOR)]),
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                line_width: *width,
                bbox: LiteParseRect::from(&graphic.bbox()),
                fill_color: 0,
                stroke_color,
            }
        }
        GraphicPrimitive::Rect { bbox, fill, stroke } => {
            let (fill_color, has_fill) = optional_color(fill.as_deref());
            let (stroke_color, has_stroke) = optional_color(stroke.as_deref());
            LiteParseGraphic {
                kind: LITEPARSE_GRAPHIC_RECT,
                flags: flag_bits(&[
                    (has_fill, LITEPARSE_GRAPHIC_FLAG_HAS_FILL_COLOR),
                    (has_stroke, LITEPARSE_GRAPHIC_FLAG_HAS_STROKE_COLOR),
                ]),
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
                line_width: 0.0,
                bbox: LiteParseRect::from(bbox),
                fill_color,
                stroke_color,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    fn leaf(items: Vec<usize>) -> Region {
        Region {
            bbox: rect(),
            kind: RegionKind::Leaf {
                item_indices: items,
            },
        }
    }

    fn split(children: Vec<Region>) -> Region {
        Region {
            bbox: rect(),
            kind: RegionKind::Split {
                axis: CutAxis::Vertical,
                children,
            },
        }
    }

    #[test]
    fn nested_region_children_are_contiguous() {
        // root -> [a -> [a1, a2], b]
        let tree = split(vec![
            split(vec![leaf(vec![0]), leaf(vec![1])]),
            leaf(vec![2]),
        ]);
        let mut packed = Packed::default();
        packed.region(&tree, LITEPARSE_NO_PARENT);

        let children = |node: usize| {
            let region = &packed.regions[node];
            let start = region.child_offset as usize;
            packed.region_children[start..start + region.child_count as usize].to_vec()
        };
        assert_eq!(children(0), [1, 4]);
        assert_eq!(children(1), [2, 3]);
        assert!(children(2).is_empty());
        for (index, region) in packed.regions.iter().enumerate().skip(1) {
            assert!(children(region.parent_index as usize).contains(&(index as u32)));
        }
    }
}
