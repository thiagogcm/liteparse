use std::collections::HashSet;

use liteparse::LiteParseConfig;
use liteparse::types::PdfInput;

use super::parity::{everything_config, fixture, source};
use super::*;
use crate::ocr::LITEPARSE_OCR_PIXEL_FORMAT_RGB;
use crate::status::LITEPARSE_STATUS_PARSE_ERROR;

fn job_state(tolerant: bool) -> JobState {
    let config = LiteParseConfig {
        continue_on_page_error: tolerant,
        ..everything_config()
    };
    let source = source(config, &fixture("sample.pdf"), false);
    let mut job = Job::begin(source.clone(), source.core(), None).unwrap();
    // A page the document cannot load, so rendering it always fails.
    let mut missing = job.pages[0].clone();
    missing.page_number = source.total_pages as usize + 1;
    job.pages.push(missing);
    job.raster_suffix_max = raster_suffix_max(&job.pages, &job.geometries, job.ocr_options.dpi);
    JobState::new(job)
}

fn select(pages: &[u32]) -> Option<HashSet<u32>> {
    Some(pages.iter().copied().collect())
}

#[test]
fn tolerant_ocr_render_failures_are_isolated_per_page() {
    let mut state = job_state(true);
    let missing = state.job.pages.last().unwrap().page_number as u32;
    let view = *state
        .render(
            select(&[1, missing]),
            usize::MAX,
            LITEPARSE_OCR_PIXEL_FORMAT_RGB,
        )
        .unwrap();
    assert_eq!(view.rasters_len, 1, "page 1 still renders");
    assert_eq!(view.arenas.page_errors_len, 1);
    let error = unsafe { *view.arenas.page_errors };
    assert_eq!(error.page_number, missing);
    assert!(
        state
            .job
            .page_errors
            .iter()
            .any(|error| error.page_number == missing)
    );
}

#[test]
fn failed_render_leaves_the_job_unchanged() {
    let mut state = job_state(false);
    // One raster ends the round at page 1, leaving later pages pending.
    let first = *state
        .render(select(&[1]), 1, LITEPARSE_OCR_PIXEL_FORMAT_RGB)
        .unwrap();
    let (cursor, errors) = (state.job.cursor, state.job.page_errors.len());
    let missing = state.job.pages.last().unwrap().page_number as u32;
    let error = state
        .render(
            select(&[missing]),
            usize::MAX,
            LITEPARSE_OCR_PIXEL_FORMAT_RGB,
        )
        .err()
        .unwrap();
    assert_eq!(error.status, LITEPARSE_STATUS_PARSE_ERROR);
    assert_eq!(state.job.cursor, cursor);
    assert_eq!(state.job.page_errors.len(), errors);
    let kept = state.round.as_ref().unwrap().view;
    assert_eq!(kept.rasters, first.rasters, "the rendered round survives");

    let foreign = state
        .render(select(&[999]), usize::MAX, LITEPARSE_OCR_PIXEL_FORMAT_RGB)
        .err()
        .unwrap();
    assert!(foreign.message.contains("not among the job's pages"));
    assert_eq!(state.job.cursor, cursor);

    // Repeating a selection whose pages were passed ends in an empty round.
    for _ in 0..2 {
        let repeated = *state
            .render(select(&[1]), 1, LITEPARSE_OCR_PIXEL_FORMAT_RGB)
            .unwrap();
        assert_eq!(repeated.rasters_len, 0);
    }
}

#[test]
fn rounds_stay_within_the_raster_budget() {
    let state = job_state(false);
    let largest = state.job.raster_suffix_max[0];
    let cap = (LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION / largest) as usize;
    assert_eq!(state.job.raster_cap(None, 0, false), 1);
    assert_eq!(state.job.raster_cap(None, 1, false), 1);
    assert_eq!(state.job.raster_cap(None, usize::MAX, false), cap);
    let gray = (LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION / (largest / 3)) as usize;
    assert_eq!(state.job.raster_cap(None, usize::MAX, true), gray);
    let huge = Page {
        page_width: 1.0e6,
        page_height: 1.0e6,
        ..state.job.pages[0].clone()
    };
    let suffix = raster_suffix_max(&[huge], &HashMap::new(), 150.0);
    assert!(suffix[0] > LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION);
}

#[test]
fn per_page_retry_records_only_failing_pages() {
    let mut errors = Vec::new();
    let work = |pages: &[u32]| {
        if pages.contains(&2) {
            Err(LiteParseError::Other("page 2 broke".into()))
        } else {
            Ok(pages.to_vec())
        }
    };
    let values = per_page_on_failure(&[1, 2, 3], true, &mut errors, work).unwrap();
    assert_eq!(values, [1, 3]);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].page_number, 2);
    let mut strict = Vec::new();
    assert!(per_page_on_failure(&[1, 2], false, &mut strict, work).is_err());
    assert!(strict.is_empty());
}

#[test]
fn complexity_matches_core_is_complex() {
    for path in super::parity::corpus_pdfs() {
        let source = source(everything_config(), &path, false);
        let core = source.core();
        let expected = block_on(core.is_complex(PdfInput::Path(path.clone())))
            .unwrap()
            .unwrap();
        let (actual, errors) = crate::complexity::complexity(&source, &core, None).unwrap();
        assert!(errors.is_empty());
        assert_eq!(
            serde_json::to_value(&expected).unwrap(),
            serde_json::to_value(&actual).unwrap(),
            "complexity diverged on {path}"
        );
    }
}

#[test]
fn unmapped_text_is_garbled_in_complexity_and_native_ocr_predicates() {
    use crate::records::{LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED, LiteParsePageComplexity};
    use liteparse::types::TextItem;

    let source = source(everything_config(), &fixture("sample.pdf"), false);
    let mut job = Job::begin(source.clone(), source.core(), Some(&[1])).unwrap();
    let healthy = TextItem {
        text: "A healthy paragraph with enough readable words to avoid sparse native text. "
            .repeat(4),
        font_name: Some("Helvetica".into()),
        ..Default::default()
    };
    let unmapped = |count| TextItem {
        text: "\u{E001}".repeat(count),
        has_unicode_map_error: true,
        font_name: Some("Type3".into()),
        ..Default::default()
    };
    let lib = Library::try_init().unwrap();
    let document = source.open(&lib, &source.input).unwrap();
    for (items, garbled) in [
        (vec![unmapped(32)], true),
        (vec![healthy.clone(), unmapped(60)], true),
        (vec![healthy.clone(), unmapped(1)], false),
        (vec![healthy, unmapped(32)], false), // substantial count, insignificant share
    ] {
        job.pages[0].text_items = items;
        let stats = page_complexity(&document, &job.pages[0], false, &mut Vec::new())
            .unwrap()
            .unwrap();
        let packed = LiteParsePageComplexity::from(&stats);
        assert_eq!(
            packed.flags & LITEPARSE_COMPLEXITY_FLAG_IS_GARBLED != 0,
            garbled
        );
        let round = stages::render_for_ocr(
            &document,
            &job.pages,
            0,
            &OcrRenderOptions {
                selection: Some(HashSet::from([1])),
                ..job.ocr_options.clone()
            },
        )
        .unwrap()
        .0;
        assert_eq!(round.len(), 1);
        assert_eq!(round[0].has_native_text, !garbled);
    }
}
