//! Parsing composed from the public core stages, carried across OCR rounds.

use std::collections::{HashMap, HashSet};
use std::ptr::{self, NonNull};
use std::sync::Arc;

use liteparse::LiteParseError;
use liteparse::config::OutputFormat;
use liteparse::ocr::{OcrEngine, OcrResult};
use liteparse::stages::{
    self, DocumentSignals, ExtractedPages, OcrRaster, OcrRenderOptions, PageComplexityStats,
    PageOcrOutcome,
};
use liteparse::types::{
    DocumentMetadata, ExtractedImage, OutlineTarget, Page, PageError, ParsedPage, XfaPacket,
};
use liteparse::{LiteParse as CoreLiteParse, ParseResult, ScreenshotResult};
use liteparse_pdfium::{Document, Library};

use crate::budget::{bytes_of, check_result_bytes};
use crate::complexity::page_complexity;
use crate::document::{LiteParseDocument, Source, copy_page_numbers};
use crate::handle::{
    Arenas, LiteParseArenas, array_ptr, as_slice, create_handle, free_handle, opaque_handles,
    packed_len, pool_str, required_out, state_mut, state_ref, sub,
};
use crate::ocr::{
    LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE, LITEPARSE_OCR_PIXEL_FORMAT_RGB,
    LITEPARSE_OCR_RASTER_FLAG_HAS_NATIVE_TEXT, LiteParseOcrPageInput, LiteParseOcrRaster,
    LiteParseOcrWord, ocr_result,
};
use crate::records::{LiteParsePageGeometry, LiteParseRect, flag_bits};
use crate::render::{
    LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION, no_visible_area, page_geometry,
    preflight_parse_screenshots,
};
use crate::result::LiteParseResult;
use crate::result::{FormRecovery, PageGeometries, ResultState};
use crate::runtime::block_on;
use crate::status::{FfiError, FfiResult, LiteParseStatus, boundary};
use crate::structure::scope_marked_content_ids;

/// One parse in progress: everything `parse` computes before OCR, carried
/// across OCR rounds. Holds no PDFium resource between calls.
pub(crate) struct Job {
    source: Arc<Source>,
    core: CoreLiteParse,
    pages: Vec<Page>,
    page_errors: Vec<PageError>,
    images: Vec<ExtractedImage>,
    image_error_count: u32,
    form_type: Option<i32>,
    creator: Option<String>,
    producer: Option<String>,
    doc_meta: Option<DocumentMetadata>,
    xfa_packets: Option<Vec<XfaPacket>>,
    screenshots: Vec<ScreenshotResult>,
    complexity: Vec<PageComplexityStats>,
    /// What extraction flattened or repaired to read the pages' forms.
    forms: FormRecovery,
    geometries: HashMap<usize, (LiteParsePageGeometry, bool)>,
    /// OCR render options derived from what extraction did to the document.
    ocr_options: OcrRenderOptions,
    /// Index into `pages` where the next OCR round starts.
    cursor: usize,
    /// Largest RGB raster, at the requested DPI, of `pages[i..]`, which
    /// bounds any round starting at `i`.
    raster_suffix_max: Vec<u64>,
}

impl Job {
    /// Run the parse's configured pre-OCR work on the selected pages, retain
    /// extracted data for OCR rounds, then release PDFium resources.
    pub(crate) fn begin(
        source: Arc<Source>,
        core: CoreLiteParse,
        pages: Option<&[u32]>,
    ) -> FfiResult<Self> {
        let config = &source.config;
        let tolerant = config.continue_on_page_error;
        let lib = Library::try_init()?;
        let input = source.extraction_input(&lib);
        let document = source.open(&lib, input)?;
        let doc_meta = source
            .metadata(&lib)?
            .map(|metadata| metadata.provenance.clone());
        let form_type = config.extract_form_fields.then(|| document.form_type());
        let creator = document.meta_text("Creator").filter(|v| !v.is_empty());
        let producer = document.meta_text("Producer").filter(|v| !v.is_empty());
        let xfa_packets = config
            .extract_xfa_packets
            .then(|| stages::xfa_packets(&document));
        let mut extracted =
            stages::extract(&document, &core.extract_request(pages, config.max_pages))?;
        scope_marked_content_ids(
            &lib,
            &document,
            input,
            config.password.as_deref(),
            &mut extracted,
        )?;
        let ocr_options = core.ocr_render_options(false, &extracted);
        let forms = FormRecovery::of(&extracted, source.repaired_input(&lib).is_some());
        let screenshot_options = core.screenshot_options(false);
        // Screenshots that paint form fields need a document extraction did
        // not flatten.
        let pristine = (config.extract_screenshots
            && stages::screenshots_need_pristine_document(&extracted, &screenshot_options))
        .then(|| source.open(&lib, input))
        .transpose()?;
        let analysis = pristine.as_ref().unwrap_or(&document);
        let ExtractedPages {
            pages,
            mut page_errors,
            images,
            image_error_count,
            ..
        } = extracted;
        let geometries: HashMap<_, _> = pages
            .iter()
            .filter_map(|page| {
                Some((
                    page.page_number,
                    page_geometry(&document, page.page_number)?,
                ))
            })
            .collect();
        let mut complexity = Vec::new();
        if config.include_complexity {
            for page in &pages {
                let stats = page_complexity(analysis, page, tolerant, &mut page_errors)?;
                complexity.extend(stats);
            }
        }
        let screenshots = if config.extract_screenshots {
            // A page with no visible area has no pixels: it fails alone, and
            // the renderer is never asked for it.
            let mut numbers = Vec::with_capacity(pages.len());
            for page in &pages {
                let number = page.page_number as u32;
                if has_area(page) {
                    numbers.push(number);
                } else if tolerant {
                    page_errors.push(PageError {
                        page_number: number,
                        message: no_visible_area(number).message,
                    });
                } else {
                    return Err(no_visible_area(number));
                }
            }
            if numbers.is_empty() {
                Vec::new()
            } else {
                preflight_parse_screenshots(analysis, Some(&numbers), config)?;
                per_page_on_failure(&numbers, tolerant, &mut page_errors, |numbers| {
                    stages::screenshots(analysis, Some(numbers), &screenshot_options)
                })?
            }
        } else {
            Vec::new()
        };
        drop(pristine);
        drop(document);
        drop(lib);
        let raster_suffix_max = raster_suffix_max(&pages, &geometries, ocr_options.dpi);
        Ok(Self {
            source,
            core,
            pages,
            page_errors,
            images,
            image_error_count,
            form_type,
            creator,
            producer,
            doc_meta,
            xfa_packets,
            screenshots,
            complexity,
            forms,
            geometries,
            raster_suffix_max,
            ocr_options,
            cursor: 0,
        })
    }

    /// Render the next round of at most `max_rasters` OCR rasters and
    /// advance past it. `selection` names the pages to render instead of
    /// the automatic choice. An empty round means OCR is complete. Pages that fail to render under tolerant processing are
    /// recorded and returned.
    pub(crate) fn render_round(
        &mut self,
        selection: Option<HashSet<u32>>,
        max_rasters: usize,
        grayscale: bool,
    ) -> FfiResult<(Vec<OcrRaster>, Vec<PageError>)> {
        if let Some(selection) = &selection {
            self.check_selection(selection)?;
        }
        if self.cursor >= self.pages.len() {
            return Ok(Default::default());
        }
        let max_rasters = self.raster_cap(selection.as_ref(), max_rasters, grayscale);
        let options = OcrRenderOptions {
            max_rasters,
            grayscale,
            selection,
            continue_on_page_error: false,
            ..self.ocr_options.clone()
        };
        let lib = Library::try_init()?;
        let input = self.source.extraction_input(&lib);
        let document = self.source.open(&lib, input)?;
        let failure = match self.render_with_area(&document, &options) {
            Ok((rasters, next)) => {
                self.cursor = next;
                return Ok((rasters, Vec::new()));
            }
            Err(error) => error,
        };
        if !self.source.config.continue_on_page_error {
            return Err(failure.into());
        }
        // Isolate the failing pages on a fresh document, one page at a time.
        // Only failures take this path, so rerunning form actions per page
        // is acceptable.
        drop(document);
        let document = self.source.open(&lib, input)?;
        let single = OcrRenderOptions {
            max_rasters: 0,
            ..options
        };
        let mut rasters = Vec::new();
        let mut errors = Vec::new();
        let mut next = self.pages.len();
        for index in self.cursor..self.pages.len() {
            if !has_area(&self.pages[index]) {
                continue;
            }
            let page = &self.pages[index..=index];
            match stages::render_for_ocr(&document, page, 0, &single) {
                Ok((rendered, _)) => rasters.extend(rendered),
                Err(error) => errors.push(page_error(page[0].page_number, &error)),
            }
            if rasters.len() >= max_rasters {
                next = index + 1;
                break;
            }
        }
        self.cursor = next;
        self.page_errors.extend(errors.iter().cloned());
        Ok((rasters, errors))
    }

    /// Render from the cursor as `options` asks, never handing the core a
    /// page with no visible area: it shows nothing to recognise and has no
    /// raster PDFium could allocate. Each core call ends before the next
    /// such page, which is stepped over. Returns the rasters and the index
    /// to resume from.
    fn render_with_area(
        &self,
        document: &Document<'_>,
        options: &OcrRenderOptions,
    ) -> Result<(Vec<OcrRaster>, usize), LiteParseError> {
        let unbounded = options.max_rasters == 0;
        let mut rasters = Vec::new();
        let mut cursor = self.cursor;
        while cursor < self.pages.len() && (unbounded || rasters.len() < options.max_rasters) {
            if !has_area(&self.pages[cursor]) {
                cursor += 1;
                continue;
            }
            let end = self.pages[cursor..]
                .iter()
                .position(|page| !has_area(page))
                .map_or(self.pages.len(), |offset| cursor + offset);
            let run = OcrRenderOptions {
                max_rasters: if unbounded {
                    0
                } else {
                    options.max_rasters - rasters.len()
                },
                ..options.clone()
            };
            let (rendered, next) =
                stages::render_for_ocr(document, &self.pages[..end], cursor, &run)?;
            rasters.extend(rendered);
            cursor = next;
        }
        Ok((rasters, cursor))
    }

    /// Explicit pages must belong to the job. Rounds only move forward, so
    /// pages already passed count as handled and repeating a selection
    /// ends in an empty round.
    fn check_selection(&self, selection: &HashSet<u32>) -> FfiResult {
        // Extraction keeps pages in ascending page order.
        for &number in selection {
            let number_usize = number as usize;
            if self
                .pages
                .binary_search_by_key(&number_usize, |page| page.page_number)
                .is_err()
            {
                return Err(FfiError::invalid_argument(format!(
                    "page {number} is not among the job's pages"
                )));
            }
        }
        Ok(())
    }

    /// Bound a round so its rasters fit the per-operation raster budget,
    /// sizing each at the requested DPI, which the core only lowers. One
    /// raster always fits.
    fn raster_cap(
        &self,
        selection: Option<&HashSet<u32>>,
        max_rasters: usize,
        grayscale: bool,
    ) -> usize {
        let largest_rgb = match selection {
            Some(selection) => self.pages[self.cursor..]
                .iter()
                .filter(|page| selection.contains(&(page.page_number as u32)))
                .map(|page| rgb_raster_bytes(page, &self.geometries, self.ocr_options.dpi))
                .max()
                .unwrap_or(0),
            None => self.raster_suffix_max[self.cursor],
        };
        let largest = if grayscale {
            largest_rgb / 3
        } else {
            largest_rgb
        };
        let cap = (LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION / largest.max(1)).max(1) as usize;
        max_rasters.clamp(1, cap)
    }

    /// Merge one round's outcomes. Every failed outcome becomes a page
    /// error; only fatal OCR passes the failures on to the core merge,
    /// which decides whether they fail the parse. Recognized words carry
    /// word boxes under the same rule extraction used for native text.
    pub(crate) fn merge(&mut self, mut outcomes: Vec<PageOcrOutcome>) -> FfiResult {
        let fatal = self.source.config.ocr_failure_fatal;
        let emit_word_boxes = self.source.config.effective_emit_word_boxes();
        for outcome in &mut outcomes {
            if let Some(message) = &outcome.error {
                self.page_errors.push(PageError {
                    page_number: outcome.page_number as u32,
                    message: message.clone(),
                });
                if !fatal {
                    outcome.error = None;
                }
            }
        }
        stages::merge_ocr(&mut self.pages, outcomes, fatal, emit_word_boxes)?;
        Ok(())
    }

    /// Run OCR with `engine` over every page that needs it, as core parsing
    /// does: at most `num_workers` recognitions are in flight, and the next
    /// pages are rendered as soon as one finishes, so a slow page does not
    /// idle the other workers. The rasters in flight stay within the
    /// per-operation raster budget, and the outcomes are merged once.
    pub(crate) fn run_ocr(&mut self, engine: &Arc<dyn OcrEngine>) -> FfiResult {
        let config = &self.source.config;
        let (language, workers) = (config.ocr_language.clone(), config.num_workers.max(1));
        let grayscale = engine.prefers_grayscale();
        let outcomes = block_on(async {
            let mut window = stages::OcrWindow::new(engine.clone(), &language, workers);
            while self.cursor < self.pages.len() {
                window.complete_ready();
                let free = window.available_capacity();
                let in_flight = workers - free;
                let budget = self.raster_cap(None, usize::MAX, grayscale);
                let room = free.min(budget.saturating_sub(in_flight));
                if room == 0 {
                    window.complete_one().await;
                    continue;
                }
                // PDFium is held only inside the round, never across an await.
                let (rasters, _) = self.render_round(None, room, grayscale)?;
                for raster in rasters {
                    window.submit(raster).await;
                }
            }
            Ok::<_, FfiError>(window.finish().await)
        })??;
        self.merge(outcomes)
    }

    /// Finish and pack the parse result; see [`finish`](Self::finish).
    pub(crate) fn into_result(
        mut self,
        signals: Option<&DocumentSignals>,
    ) -> FfiResult<ResultState> {
        let source = self.source.clone();
        let forms = std::mem::take(&mut self.forms);
        let (result, geometries) = self.finish(signals);
        let descriptive = result.doc_meta.as_ref().and(source.descriptive());
        ResultState::parsed(&result, &source.config, descriptive, geometries, forms)
    }

    /// The pages filtered and projected, as [`finish`](Self::finish)
    /// classifies them.
    pub(crate) fn projected(mut self) -> Vec<ParsedPage> {
        stages::apply_content_filters(&mut self.pages, &self.core.content_filters());
        stages::project(self.pages)
    }

    /// Filter, project, classify, and render the pages into the parse
    /// result, with page geometries parallel to its pages. The pages are
    /// classified against `signals`, or against their own when there are
    /// none, as core parsing classifies the pages it is given.
    pub(crate) fn finish(
        mut self,
        signals: Option<&DocumentSignals>,
    ) -> (ParseResult, PageGeometries) {
        stages::apply_content_filters(&mut self.pages, &self.core.content_filters());
        let mut pages = stages::project(self.pages);
        let outline = self.source.outline.clone();
        let mut text = layout(&self.core, &mut pages, &outline, signals);
        let mut complexity = self.complexity.into_iter().peekable();
        for page in &mut pages {
            if let Some(mut stats) =
                complexity.next_if(|stats| stats.page_number == page.page_number)
            {
                stats.layout = Some(stages::layout_complexity(page));
                page.complexity = Some(stats);
            }
        }
        if self.source.config.output_format == OutputFormat::Markdown {
            stages::canonicalize_image_refs(&mut pages, &mut text, &self.images);
        }
        self.page_errors.sort_by_key(|error| error.page_number);
        let geometries = pages
            .iter()
            .map(|page| self.geometries.get(&page.page_number).copied())
            .collect();
        let result = ParseResult {
            total_pages: self.source.total_pages,
            pages,
            page_errors: self.page_errors,
            text,
            outline,
            images: self.images,
            screenshots: self.screenshots,
            image_error_count: self.image_error_count,
            form_type: self.form_type,
            creator: self.creator,
            producer: self.producer,
            doc_meta: self.doc_meta,
            xfa_packets: self.xfa_packets,
        };
        (result, geometries)
    }
}

/// Classify the projected pages and render them, the layout half of core
/// `parse_from_pages` composed from its stages so that the signals can be
/// those of more pages than these: each page's blocks under
/// `extract_blocks`, its Markdown under that output format, and the
/// document text, the pages' Markdown or their projected text. The signals
/// are `signals`, or those of `pages` when there are none.
fn layout(
    core: &CoreLiteParse,
    pages: &mut [ParsedPage],
    outline: &[OutlineTarget],
    signals: Option<&DocumentSignals>,
) -> String {
    let config = core.config();
    let markdown = config.output_format == OutputFormat::Markdown;
    if markdown || config.extract_blocks {
        let own;
        let signals = match signals {
            Some(signals) => signals,
            None => {
                own = stages::document_signals(pages, config.keep_headers_footers);
                &own
            }
        };
        let options = core.block_options(outline);
        for page in pages.iter_mut() {
            let blocks = stages::extract_blocks(page, signals, &options);
            if config.extract_blocks {
                // Extraction was on, so a page with nothing to decompose
                // reports an empty list.
                page.blocks = Some(stages::layout_blocks(
                    page,
                    blocks.as_deref().unwrap_or_default(),
                ));
            }
            if markdown {
                page.markdown = stages::render_page_markdown(page, blocks.as_deref());
            }
        }
    }
    if markdown {
        let rendered: Vec<&str> = pages.iter().map(|page| page.markdown.as_str()).collect();
        rendered.join("\n\n-----\n\n")
    } else {
        let projected: Vec<&str> = pages.iter().map(|page| page.text.as_str()).collect();
        projected.join("\n\n")
    }
}

/// Whether `page` has a visible area. A crop box that misses the media box
/// leaves none, and extraction measures such a page 0x0.
fn has_area(page: &Page) -> bool {
    page.page_width > 0.0 && page.page_height > 0.0
}

/// Bytes of `page` rendered in RGB at `dpi`.
fn rgb_raster_bytes(
    page: &Page,
    geometries: &HashMap<usize, (LiteParsePageGeometry, bool)>,
    dpi: f32,
) -> u64 {
    let user_unit = geometries
        .get(&page.page_number)
        .map_or(1.0, |(geometry, _)| geometry.user_unit);
    let scale = dpi / 72.0 * user_unit;
    let edge = |points: f32| {
        let pixels = (points * scale).ceil();
        if pixels.is_finite() && pixels > 0.0 {
            pixels as u64
        } else {
            1
        }
    };
    edge(page.page_width)
        .saturating_mul(edge(page.page_height))
        .saturating_mul(3)
}

fn raster_suffix_max(
    pages: &[Page],
    geometries: &HashMap<usize, (LiteParsePageGeometry, bool)>,
    dpi: f32,
) -> Vec<u64> {
    let mut suffix = vec![0; pages.len() + 1];
    for (index, page) in pages.iter().enumerate().rev() {
        suffix[index] = suffix[index + 1].max(rgb_raster_bytes(page, geometries, dpi));
    }
    suffix
}

pub(crate) fn page_error(page_number: usize, error: &LiteParseError) -> PageError {
    PageError {
        page_number: page_number as u32,
        message: error.to_string(),
    }
}

/// Run `work` over `pages`; when it fails under tolerant processing, rerun
/// it page by page and record each page that still fails.
fn per_page_on_failure<T>(
    pages: &[u32],
    tolerant: bool,
    errors: &mut Vec<PageError>,
    work: impl Fn(&[u32]) -> Result<Vec<T>, LiteParseError>,
) -> FfiResult<Vec<T>> {
    let failure = match work(pages) {
        Ok(values) => return Ok(values),
        Err(error) => error,
    };
    if !tolerant {
        return Err(failure.into());
    }
    let mut values = Vec::new();
    for &page in pages {
        match work(&[page]) {
            Ok(page_values) => values.extend(page_values),
            Err(error) => errors.push(page_error(page as usize, &error)),
        }
    }
    Ok(values)
}

/// A parse in progress whose OCR the host performs between calls. Holds no
/// PDFium resource and may outlive its document.
pub struct LiteParseJob {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseJob => JobState, "job";
}

/// One OCR round: rasters with their pixels in `arenas.binary`, embedded
/// image rectangles, and the round's render failures in
/// `arenas.page_errors`. Borrowed from the job until its next call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseOcrRasterView {
    pub arenas: LiteParseArenas,
    pub rasters: *const LiteParseOcrRaster,
    pub rasters_len: usize,
    pub image_rects: *const LiteParseRect,
    pub image_rects_len: usize,
}

pub(crate) struct JobState {
    job: Job,
    round: Option<Round>,
    /// A fatal OCR merge failed; the job accepts only `liteparse_job_free`.
    failed: bool,
}

/// A rendered round awaiting merge.
struct Round {
    /// Raster facts with their pixels released after packing.
    facts: Vec<OcrRaster>,
    /// Backing storage for `view`.
    #[allow(dead_code)]
    arenas: Arenas,
    #[allow(dead_code)]
    rasters: Vec<LiteParseOcrRaster>,
    #[allow(dead_code)]
    image_rects: Vec<LiteParseRect>,
    view: LiteParseOcrRasterView,
}

impl Round {
    fn pack(mut facts: Vec<OcrRaster>, errors: &[PageError], pixel_format: u32) -> FfiResult<Self> {
        let mut arenas = Arenas::default();
        arenas.push_errors(errors);
        let mut rasters = Vec::with_capacity(facts.len());
        let mut image_rects = Vec::new();
        arenas
            .blobs
            .reserve(facts.iter().map(|raster| raster.pixels.len()).sum());
        for raster in &mut facts {
            let arrays = bytes_of(&rasters) + bytes_of(&image_rects);
            let pixels = arenas.push_blob(&raster.pixels, arrays)?;
            raster.pixels = Vec::new();
            let image_rect_offset = image_rects.len();
            image_rects.extend(raster.image_rects.iter().map(LiteParseRect::from));
            rasters.push(LiteParseOcrRaster {
                pixels,
                page_number: raster.page_number as u32,
                width: raster.width,
                height: raster.height,
                pixel_format,
                dpi: raster.dpi,
                flags: flag_bits(&[(
                    raster.has_native_text,
                    LITEPARSE_OCR_RASTER_FLAG_HAS_NATIVE_TEXT,
                )]),
                image_rect_offset: packed_len(image_rect_offset),
                image_rect_count: packed_len(raster.image_rects.len()),
            });
        }
        check_result_bytes(arenas.bytes() + bytes_of(&rasters) + bytes_of(&image_rects))?;
        let view = LiteParseOcrRasterView {
            arenas: arenas.view(),
            rasters: array_ptr(&rasters),
            rasters_len: rasters.len(),
            image_rects: array_ptr(&image_rects),
            image_rects_len: image_rects.len(),
        };
        Ok(Self {
            facts,
            arenas,
            rasters,
            image_rects,
            view,
        })
    }
}

impl JobState {
    pub(crate) fn new(job: Job) -> Self {
        Self {
            job,
            round: None,
            failed: false,
        }
    }

    fn usable(&self) -> FfiResult {
        if self.failed {
            return Err(FfiError::invalid_argument(
                "the job failed a fatal OCR merge; free it",
            ));
        }
        Ok(())
    }

    fn render(
        &mut self,
        selection: Option<HashSet<u32>>,
        max_rasters: usize,
        pixel_format: u32,
    ) -> FfiResult<&LiteParseOcrRasterView> {
        self.usable()?;
        let grayscale = match pixel_format {
            LITEPARSE_OCR_PIXEL_FORMAT_RGB => false,
            LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE => true,
            _ => return Err(FfiError::invalid_argument("unknown OCR pixel format")),
        };
        let (cursor, errors) = (self.job.cursor, self.job.page_errors.len());
        let round = self
            .job
            .render_round(selection, max_rasters, grayscale)
            .and_then(|(rasters, failures)| Round::pack(rasters, &failures, pixel_format));
        match round {
            Ok(round) => Ok(&self.round.insert(round).view),
            Err(error) => {
                self.job.cursor = cursor;
                self.job.page_errors.truncate(errors);
                Err(error)
            }
        }
    }

    fn merge(
        &mut self,
        inputs: &[LiteParseOcrPageInput],
        words: &[LiteParseOcrWord],
        pool: &[u8],
    ) -> FfiResult {
        self.usable()?;
        let round = self
            .round
            .as_ref()
            .ok_or_else(|| FfiError::invalid_argument("no rendered OCR round awaits a merge"))?;
        // Recognition per raster; a rendered page without an input
        // recognized nothing, which the fatal decision counts as a success.
        let mut recognized: Vec<Option<(Vec<OcrResult>, Option<String>)>> =
            vec![None; round.facts.len()];
        for (index, input) in inputs.iter().enumerate() {
            let where_ = format!("page_inputs[{index}]");
            let slot = round
                .facts
                .iter()
                .position(|raster| raster.page_number == input.page_number as usize)
                .ok_or_else(|| {
                    FfiError::invalid_argument(format!(
                        "{where_}.page_number {} was not rendered in this round",
                        input.page_number
                    ))
                })?;
            if recognized[slot].is_some() {
                return Err(FfiError::invalid_argument(format!(
                    "{where_}.page_number {} repeats an earlier input",
                    input.page_number
                )));
            }
            let raster = &round.facts[slot];
            let error = pool_str(pool, input.error, &format!("{where_}.error"))?;
            let page_words = sub(words, input.word_offset, input.word_count, &where_, "words")?;
            if error.is_some() && !page_words.is_empty() {
                return Err(FfiError::invalid_argument(format!(
                    "{where_} has both an error and words"
                )));
            }
            let results = page_words
                .iter()
                .enumerate()
                .map(|(offset, word)| {
                    let index = input.word_offset as usize + offset;
                    ocr_result(word, pool, index, raster.width, raster.height)
                })
                .collect::<FfiResult<_>>()?;
            recognized[slot] = Some((results, error.map(str::to_owned)));
        }
        let outcomes = recognized
            .into_iter()
            .zip(&round.facts)
            .map(|(recognized, raster)| {
                let (results, error) = recognized.unwrap_or_default();
                PageOcrOutcome {
                    page_number: raster.page_number,
                    dpi: raster.dpi,
                    has_native_text: raster.has_native_text,
                    image_rects: raster.image_rects.clone(),
                    results,
                    error,
                }
            })
            .collect();
        self.round = None;
        let merged = self.job.merge(outcomes);
        self.failed = merged.is_err();
        merged
    }
}

/// Start a parse whose OCR the host performs. Runs extraction and configured
/// pre-OCR work for the same page selection, then releases PDFium. Projection
/// and classification happen in `liteparse_job_finish`, after OCR. The job
/// never runs the configured OCR engine, and it may outlive `document`.
///
/// `document` must be live, `pages` readable or null with zero length, and
/// `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_begin(
    document: *const LiteParseDocument,
    pages: *const u32,
    pages_len: usize,
    out: *mut *mut LiteParseJob,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let state = state_ref(document)?;
            let pages = state.selection(copy_page_numbers(pages, pages_len)?)?;
            let job = Job::begin(state.source(), state.core(), pages.as_deref())?;
            Ok(JobState::new(job))
        })
    }
}

/// Render the next OCR round: at most `max_rasters` pages (zero uses the
/// configured `num_workers`), fewer when the round would exceed
/// `LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION`, in
/// `LITEPARSE_OCR_PIXEL_FORMAT_*` pixels. Rounds move forward through the
/// job's pages. The core OCR predicates choose the pages whatever the
/// configured OCR engine; a non-empty `pages` list names job pages and
/// renders only those not yet passed. Repeat the same list on every call
/// until an empty round, which means OCR is complete. The view is valid
/// until the next call on the job, and rasters left unmerged when the next
/// round renders receive no OCR. On failure, including `LITEPARSE_STATUS_RESOURCE_LIMIT`, the job
/// is unchanged.
///
/// `job` must be live and exclusively owned by the caller for the call,
/// `pages` readable or null with zero length, and `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_job_render_ocr(
    job: *mut LiteParseJob,
    pages: *const u32,
    pages_len: usize,
    max_rasters: u32,
    pixel_format: u32,
    out: *mut *const LiteParseOcrRasterView,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let out = required_out(out)?;
        out.as_ptr().write(ptr::null());
        let state = state_mut(job)?;
        let pages = state
            .job
            .source
            .selection(copy_page_numbers(pages, pages_len)?)?;
        let max_rasters = match max_rasters {
            0 => state.job.source.config.num_workers.max(1),
            limit => limit as usize,
        };
        let selection = pages.map(|pages| pages.into_iter().collect());
        let view = state.render(selection, max_rasters, pixel_format)?;
        out.as_ptr().write(view);
        Ok(())
    })
}

/// Merge recognition for the rendered round: one `LiteParseOcrPageInput`
/// per recognized page, with `LiteParseOcrWord` records in raster pixels
/// whose text indexes `pool`. A rendered page without an input recognized
/// nothing. Every input is validated before the job changes. Recognition
/// errors become page errors; with `LITEPARSE_FLAG_OCR_FAILURE_FATAL` the
/// core merge fails with `LITEPARSE_STATUS_OCR_ERROR` when every page in
/// the round failed and one had sparse native text, after which the job
/// accepts only `liteparse_job_free`.
///
/// `job` must be live and exclusively owned by the caller for the call;
/// each array must be readable for its length, or null with zero length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_job_merge_ocr(
    job: *mut LiteParseJob,
    page_inputs: *const LiteParseOcrPageInput,
    page_inputs_len: usize,
    words: *const LiteParseOcrWord,
    words_len: usize,
    pool: *const u8,
    pool_len: usize,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let state = state_mut(job)?;
        let inputs = as_slice(page_inputs, page_inputs_len, "page_inputs")?.unwrap_or_default();
        let words = as_slice(words, words_len, "words")?.unwrap_or_default();
        let pool = as_slice(pool, pool_len, "pool")?.unwrap_or_default();
        state.merge(inputs, words, pool)
    })
}

/// Consume the job: filter, project, classify, and render its pages into
/// the result `liteparse_document_parse` produces. A rendered round left
/// unmerged receives no OCR. `*job` is freed and set to null whether or not
/// the call succeeds.
///
/// `job` must point to a live job handle or null; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_job_finish(
    job: *mut *mut LiteParseJob,
    out: *mut *mut LiteParseResult,
) -> LiteParseStatus {
    // Take the job before anything can fail, so every outcome frees it.
    let state = NonNull::new(job)
        .and_then(|slot| NonNull::new(unsafe { slot.as_ptr().replace(ptr::null_mut()) }))
        .map(|handle| unsafe { Box::from_raw(handle.cast::<JobState>().as_ptr()) });
    unsafe {
        create_handle(out, || {
            let state = state.ok_or_else(|| FfiError::invalid_argument("job must not be null"))?;
            state.usable()?;
            state.job.into_result(None)
        })
    }
}

/// Destroy a job. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_job_free(job: *mut LiteParseJob) {
    unsafe { free_handle(job) };
}

#[cfg(test)]
mod parity;

#[cfg(test)]
mod tests;
