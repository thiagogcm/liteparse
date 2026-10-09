//! The composed pipeline must equal core parsing. Ports the core
//! `stages_compose` corpus test with core `parse_input` as the oracle.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use liteparse::config::{CropBox, ImageMode, OutputFormat, PageOrientationCorrection};
use liteparse::ocr::{OcrEngine, OcrOptions, OcrResult};
use liteparse::types::PdfInput;
use liteparse::{LiteParseConfig, ParseResult};

use super::{Job, JobState};
use crate::document::Source;
use crate::handle::LiteParseStr;
use crate::ocr::{
    LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE, LITEPARSE_OCR_PIXEL_FORMAT_RGB,
    LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON, LiteParseOcrPageInput, LiteParseOcrWord,
};
use crate::runtime::block_on;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../integration_tests_data");

pub(super) fn fixture(name: &str) -> String {
    format!("{FIXTURES}/{name}")
}

pub(super) fn corpus_pdfs() -> Vec<String> {
    let mut paths: Vec<String> = std::fs::read_dir(FIXTURES)
        .expect("fixture directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "pdf").then(|| path.to_string_lossy().into_owned())
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixture PDFs under {FIXTURES}");
    paths
}

/// One box per raster whose text encodes the raster size, so a raster
/// routed to the wrong page shows in the output.
pub(super) struct MockOcr;

type Recognized = Result<Vec<OcrResult>, Box<dyn std::error::Error + Send + Sync>>;

impl MockOcr {
    pub(super) fn words(width: u32, height: u32) -> Vec<OcrResult> {
        vec![
            OcrResult {
                text: format!("MOCK {width}x{height}"),
                bbox: [10.0, 10.0, 260.0, 40.0],
                confidence: 0.99,
                polygon: None,
            },
            OcrResult {
                text: "rotated".into(),
                bbox: [300.0, 100.0, 330.0, 400.0],
                confidence: 0.9,
                polygon: Some([
                    [330.0, 100.0],
                    [330.0, 400.0],
                    [300.0, 400.0],
                    [300.0, 100.0],
                ]),
            },
        ]
    }
}

impl OcrEngine for MockOcr {
    fn name(&self) -> &str {
        "mock"
    }

    fn recognize<'a, 'b: 'a, 'c: 'a>(
        &'a self,
        _image_data: &'c [u8],
        width: u32,
        height: u32,
        _options: &'b OcrOptions,
    ) -> Pin<Box<dyn Future<Output = Recognized> + Send + '_>> {
        Box::pin(std::future::ready(Ok(Self::words(width, height))))
    }
}

/// Every reported field in a comparable form.
#[derive(Debug, PartialEq)]
pub(super) struct Snapshot {
    total_pages: u32,
    pages: Vec<serde_json::Value>,
    page_errors: Vec<(u32, String)>,
    text: String,
    outline: String,
    images: Vec<serde_json::Value>,
    screenshots: Vec<(u32, u32, u32, bool, usize, Vec<u8>)>,
    image_error_count: u32,
    form_type: Option<i32>,
    creator: Option<String>,
    producer: Option<String>,
    doc_meta: Option<serde_json::Value>,
    xfa_packets: Option<serde_json::Value>,
}

pub(super) fn snapshot(result: &ParseResult) -> Snapshot {
    macro_rules! json {
        ($value:expr) => {
            serde_json::to_value($value).unwrap()
        };
    }
    Snapshot {
        total_pages: result.total_pages,
        pages: result.pages.iter().map(|page| json!(page)).collect(),
        page_errors: result
            .page_errors
            .iter()
            .map(|error| (error.page_number, error.message.clone()))
            .collect(),
        text: result.text.clone(),
        outline: format!("{:?}", result.outline),
        images: result.images.iter().map(|image| json!(image)).collect(),
        screenshots: result
            .screenshots
            .iter()
            .map(|shot| {
                (
                    shot.page_num,
                    shot.width,
                    shot.height,
                    shot.is_solid_fill,
                    shot.rects.len(),
                    shot.image_bytes.clone(),
                )
            })
            .collect(),
        image_error_count: result.image_error_count,
        form_type: result.form_type,
        creator: result.creator.clone(),
        producer: result.producer.clone(),
        doc_meta: result.doc_meta.as_ref().map(|meta| json!(meta)),
        xfa_packets: result.xfa_packets.as_ref().map(|packets| json!(packets)),
    }
}

pub(super) fn source(config: LiteParseConfig, path: &str, ocr: bool) -> Arc<Source> {
    let source = Source::prepare(config, None, Ok(None), PdfInput::Path(path.into()))
        .unwrap_or_else(|error| panic!("prepare {path}: {}", error.message));
    Arc::new(if ocr {
        source.with_ocr_engine(Arc::new(MockOcr))
    } else {
        source
    })
}

pub(super) fn selection(source: &Source) -> Option<Vec<u32>> {
    source.config.target_pages.as_ref().map(|pages| {
        let mut pages = liteparse::config::parse_target_pages(pages).unwrap();
        pages.sort_unstable();
        pages.dedup();
        pages
    })
}

/// Core parsing through the same configured parser.
pub(super) fn oracle(source: &Source) -> ParseResult {
    let core = source.core();
    block_on(core.parse_input(source.input.clone()))
        .unwrap()
        .unwrap_or_else(|error| panic!("core parse failed: {error}"))
}

/// The C pipeline: begin, OCR rounds with the source's engine, finish.
fn composed(source: &Arc<Source>) -> ParseResult {
    let core = source.core();
    let engine = source.ocr_engine.clone().unwrap();
    let pages = selection(source);
    let mut job = Job::begin(source.clone(), core, pages.as_deref())
        .unwrap_or_else(|error| panic!("begin: {}", error.message));
    if let Some(engine) = engine {
        job.run_ocr(&engine)
            .unwrap_or_else(|error| panic!("ocr: {}", error.message));
    }
    job.finish(None).0
}

/// A parse of `pages` alone, classified against `signals`: the source's own
/// for the whole document, or none for the pages' own.
fn selected(source: &Arc<Source>, pages: &[u32], document_signals: bool) -> ParseResult {
    let signals = document_signals.then(|| {
        source
            .signals()
            .unwrap_or_else(|error| panic!("signals: {}", error.message))
    });
    let engine = source.ocr_engine.clone().unwrap();
    let mut job = Job::begin(source.clone(), source.core(), Some(pages))
        .unwrap_or_else(|error| panic!("begin: {}", error.message));
    if let Some(engine) = engine {
        job.run_ocr(&engine)
            .unwrap_or_else(|error| panic!("ocr: {}", error.message));
    }
    job.finish(signals).0
}

/// Every `step`th page of the whole parse must be the page a parse of it
/// alone with the document's signals gives. Returns how many of them a
/// parse of them alone, with their own signals, classifies differently.
fn assert_document_signals(config: LiteParseConfig, path: &str, ocr: bool, step: usize) -> usize {
    let source = source(config, path, ocr);
    let whole = composed(&source);
    let mut ranked_by_the_document = 0;
    if whole.pages.len() < 2 {
        return ranked_by_the_document;
    }
    for expected in whole.pages.iter().step_by(step) {
        let number = expected.page_number as u32;
        let expected = serde_json::to_value(expected).unwrap();
        let with_signals = selected(&source, &[number], true);
        assert_eq!(
            expected,
            serde_json::to_value(&with_signals.pages[0]).unwrap(),
            "page {number} of {path} parsed with the document's signals is not the whole parse's"
        );
        let alone = selected(&source, &[number], false);
        ranked_by_the_document +=
            usize::from(expected != serde_json::to_value(&alone.pages[0]).unwrap());
    }
    ranked_by_the_document
}

/// The job API with the host running `MockOcr` on each round's rasters.
fn staged(source: &Arc<Source>, max_rasters: usize, pixel_format: u32) -> ParseResult {
    let pages = selection(source);
    let job = Job::begin(source.clone(), source.core(), pages.as_deref())
        .unwrap_or_else(|error| panic!("begin: {}", error.message));
    let mut state = JobState::new(job);
    loop {
        let view = *state
            .render(None, max_rasters, pixel_format)
            .unwrap_or_else(|error| panic!("render: {}", error.message));
        if view.rasters_len == 0 {
            break;
        }
        let rasters = unsafe { std::slice::from_raw_parts(view.rasters, view.rasters_len) };
        let (mut pool, mut words, mut inputs) = (Vec::new(), Vec::new(), Vec::new());
        for raster in rasters {
            let word_offset = words.len() as u32;
            for result in MockOcr::words(raster.width, raster.height) {
                let text = LiteParseStr {
                    offset: pool.len() as u32,
                    len: result.text.len() as u32,
                };
                pool.extend_from_slice(result.text.as_bytes());
                let [x1, y1, x2, y2] = result.bbox;
                words.push(LiteParseOcrWord {
                    text,
                    x1,
                    y1,
                    x2,
                    y2,
                    confidence: result.confidence,
                    polygon: result
                        .polygon
                        .map_or([0.0; 8], |p| p.as_flattened().try_into().unwrap()),
                    flags: if result.polygon.is_some() {
                        LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON
                    } else {
                        0
                    },
                });
            }
            inputs.push(LiteParseOcrPageInput {
                page_number: raster.page_number,
                word_offset,
                word_count: words.len() as u32 - word_offset,
                error: LiteParseStr::default(),
            });
        }
        state
            .merge(&inputs, &words, &pool)
            .unwrap_or_else(|error| panic!("merge: {}", error.message));
    }
    state.job.finish(None).0
}

fn assert_staged_parity(
    config: LiteParseConfig,
    path: &str,
    max_rasters: usize,
    pixel_format: u32,
) {
    let expected = oracle(&source(config.clone(), path, true));
    let actual = staged(&source(config, path, false), max_rasters, pixel_format);
    assert_eq!(
        snapshot(&expected),
        snapshot(&actual),
        "staged OCR diverged from core parse on {path}"
    );
}

fn assert_parity(config: LiteParseConfig, path: &str, ocr: bool) -> ParseResult {
    let source = source(config, path, ocr);
    let expected = oracle(&source);
    let actual = composed(&source);
    assert_eq!(
        snapshot(&expected),
        snapshot(&actual),
        "composed parse diverged from core parse on {path}"
    );
    actual
}

/// OCR in one-page rounds, so the render, recognize, and merge loop runs
/// several times.
fn ocr_config() -> LiteParseConfig {
    LiteParseConfig {
        ocr_enabled: true,
        num_workers: 1,
        quiet: true,
        output_format: OutputFormat::Markdown,
        include_complexity: true,
        ..LiteParseConfig::default()
    }
}

pub(super) fn everything_config() -> LiteParseConfig {
    LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        output_format: OutputFormat::Markdown,
        image_mode: ImageMode::Embed,
        extract_images: true,
        extract_blocks: true,
        include_complexity: true,
        extract_content_bounds: true,
        emit_word_boxes: true,
        extract_text_metadata: true,
        extract_vector_graphics: true,
        extract_annotations: true,
        extract_structure_tree: true,
        extract_xfa_packets: true,
        extract_screenshots: true,
        detect_screenshot_rects: true,
        extract_links: true,
        ..LiteParseConfig::default()
    }
}

#[test]
fn markdown_with_every_option() {
    let (mut blocks, mut screenshots) = (0, 0);
    for path in corpus_pdfs() {
        let result = assert_parity(everything_config(), &path, false);
        blocks += usize::from(
            result
                .pages
                .iter()
                .any(|page| page.blocks.as_ref().is_some_and(|b| !b.is_empty())),
        );
        screenshots += usize::from(!result.screenshots.is_empty());
    }
    assert!(blocks > 0 && screenshots > 0);
    let config = LiteParseConfig {
        target_pages: Some("1,2,108,119".into()),
        ..everything_config()
    };
    let demo = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../demo/docs/apple-10k-2024.pdf"
    );
    let result = assert_parity(config, demo, false);
    assert!(!result.images.is_empty(), "the demo 10-K embeds images");
}

#[test]
fn pages_parsed_with_document_signals_are_the_whole_parses() {
    let config = LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        output_format: OutputFormat::Markdown,
        extract_blocks: true,
        ..LiteParseConfig::default()
    };
    for path in corpus_pdfs() {
        assert_document_signals(config.clone(), &path, false, 1);
        assert_document_signals(ocr_config(), &path, true, 1);
    }
    let demo = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../demo/docs/apple-10k-2024.pdf"
    );
    let ranked = assert_document_signals(config, demo, false, 12);
    assert!(
        ranked > 0,
        "no sampled page of the demo 10-K is classified differently alone, so the signals went untested"
    );
}

#[test]
fn text_output_with_blocks() {
    let config = LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        output_format: OutputFormat::Text,
        extract_blocks: true,
        ..LiteParseConfig::default()
    };
    for path in corpus_pdfs() {
        assert_parity(config.clone(), &path, false);
    }
}

#[test]
fn ocr_rounds() {
    let config = ocr_config();
    let mut ocr_pages = 0;
    for path in corpus_pdfs() {
        // Several recognitions in flight must merge as one at a time does.
        let several = LiteParseConfig {
            num_workers: 4,
            ..config.clone()
        };
        assert_parity(several, &path, true);
        let result = assert_parity(config.clone(), &path, true);
        ocr_pages += result
            .pages
            .iter()
            .filter(|page| page.text_items.iter().any(|i| i.text.starts_with("MOCK ")))
            .count();
    }
    assert!(ocr_pages > 0, "no fixture page merged mock OCR text");
}

#[test]
fn converted_image() {
    if std::env::var("SKIP_INTEGRATION_TESTS").as_deref() == Ok("yes") {
        return;
    }
    let config = LiteParseConfig {
        ocr_enabled: true,
        quiet: true,
        output_format: OutputFormat::Markdown,
        ..LiteParseConfig::default()
    };
    let result = assert_parity(config, &fixture("receipt.png"), true);
    assert!(result.text.contains("MOCK "), "{}", result.text);
}

#[test]
fn orientation_selection_and_filters() {
    let config = LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        output_format: OutputFormat::Markdown,
        page_orientation_corrections: vec![PageOrientationCorrection { page: 1, angle: 90 }],
        target_pages: Some("1".into()),
        max_pages: 1,
        crop_box: Some(CropBox {
            top: 0.05,
            bottom: 0.05,
            left: 0.0,
            right: 0.0,
        }),
        skip_diagonal_text: true,
        ..LiteParseConfig::default()
    };
    for name in ["sample_rotated_90cw.pdf", "diagonal_text.pdf"] {
        assert_parity(config.clone(), &fixture(name), false);
    }
}

#[test]
fn form_fields_with_ocr() {
    let config = LiteParseConfig {
        ocr_enabled: true,
        quiet: true,
        output_format: OutputFormat::Markdown,
        extract_form_fields: true,
        ..LiteParseConfig::default()
    };
    assert_parity(config, &fixture("filled_acroform.pdf"), true);
}

#[test]
fn form_fields_with_rendered_screenshots() {
    let config = LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        output_format: OutputFormat::Markdown,
        extract_form_fields: true,
        extract_screenshots: true,
        render_form_fields: true,
        include_complexity: true,
        ..LiteParseConfig::default()
    };
    let result = assert_parity(config, &fixture("filled_acroform.pdf"), false);
    assert!(!result.screenshots.is_empty());
}

#[test]
fn staged_ocr_rounds() {
    let config = ocr_config();
    for path in corpus_pdfs() {
        assert_staged_parity(config.clone(), &path, 1, LITEPARSE_OCR_PIXEL_FORMAT_RGB);
        // Round sizes and pixel formats change nothing the merge sees.
        assert_staged_parity(
            config.clone(),
            &path,
            2,
            LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE,
        );
    }
    let forms = LiteParseConfig {
        extract_form_fields: true,
        num_workers: 2,
        ..config
    };
    assert_staged_parity(
        forms,
        &fixture("filled_acroform.pdf"),
        2,
        LITEPARSE_OCR_PIXEL_FORMAT_RGB,
    );
}
