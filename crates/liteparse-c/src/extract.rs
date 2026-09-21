use liteparse::OutputFormat;
use liteparse::extract::{ExtractionOutputOptions, extract_pages_and_images};
use liteparse_pdfium::Library;

use crate::document::DocumentState;
use crate::render::load_document;
use crate::result::ResultState;
use crate::status::FfiResult;

/// Pre-projection pages: heuristic text items, graphics, and configured
/// extras, packed as an extract-only result.
pub(crate) fn extract_pages(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<ResultState> {
    let lib = Library::init();
    let document = load_document(&lib, &state.input, state.config.password.as_deref())?;
    liteparse::extract::apply_page_orientation_corrections(
        &document,
        &state.config.page_orientation_corrections,
    )?;
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
    let geometries = state.geometries_for(
        Some(&document),
        extracted.pages.iter().map(|page| page.page_number),
    );
    Ok(ResultState::extracted(
        extracted,
        form_type,
        &state.config,
        geometries,
    ))
}
