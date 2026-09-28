use liteparse::stages;
use liteparse_pdfium::Library;

use crate::document::DocumentState;
use crate::render::page_geometry;
use crate::result::{FormRecovery, ResultState};
use crate::status::FfiResult;
use crate::structure::scope_marked_content_ids;

/// Pre-projection pages: heuristic text items, graphics, and configured
/// extras, packed as an extract-only result.
pub(crate) fn extract_pages(
    state: &DocumentState,
    pages: Option<Vec<u32>>,
) -> FfiResult<ResultState> {
    let config = &state.config;
    let lib = Library::try_init()?;
    let document = state.open(&lib, state.extraction_input(&lib))?;
    let form_type = config.extract_form_fields.then(|| document.form_type());
    let core = state.core();
    let request = core.extract_request(pages.as_deref(), config.max_pages);
    let mut extracted = stages::extract(&document, &request)?;
    scope_marked_content_ids(&document, &mut extracted, || {
        state.open(&lib, state.extraction_input(&lib))
    })?;
    let forms = FormRecovery::of(&extracted, state.repaired_input(&lib).is_some());
    let geometries = extracted
        .pages
        .iter()
        .map(|page| page_geometry(&document, page.page_number))
        .collect();
    ResultState::extracted(
        &extracted,
        form_type,
        forms,
        config,
        geometries,
        state.total_pages,
        &state.outline,
    )
}
