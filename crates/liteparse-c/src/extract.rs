use liteparse::stages;
use liteparse_pdfium::Library;

use crate::document::DocumentState;
use crate::result::ResultState;
use crate::status::FfiResult;

/// Pre-projection pages: heuristic text items, graphics, and configured
/// extras, packed as an extract-only result.
pub(crate) fn extract_pages(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<ResultState> {
    let lib = Library::init();
    let password = state.config.password.as_deref();
    let repaired = state
        .config
        .extract_form_fields
        .then(|| stages::repair_acroform(&lib, &state.input, password))
        .flatten();
    let document_input = repaired.as_ref().unwrap_or(&state.input);
    let document = stages::open(
        &lib,
        document_input,
        password,
        &state.config.page_orientation_corrections,
    )?;
    let form_type = state
        .config
        .extract_form_fields
        .then(|| document.form_type());
    let parser = state.parser_for(pages.as_deref());
    let request = parser.extract_request(pages.as_deref(), state.config.max_pages);
    let extracted = stages::extract(&document, &request)?;
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
