use liteparse::ocr::OcrResult;

use crate::handle::{LiteParseBlobRef, LiteParseStr, pool_str};
use crate::records::has;
use crate::status::FfiError;

/// `LiteParseOcrRaster.pixel_format` values and `liteparse_job_render_ocr`
/// pixel formats.
pub const LITEPARSE_OCR_PIXEL_FORMAT_RGB: u32 = 0;
pub const LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE: u32 = 1;

/// `LiteParseOcrRaster.flags` bits.
/// The page has real native text; OCR enriches it rather than replacing it.
pub const LITEPARSE_OCR_RASTER_FLAG_HAS_NATIVE_TEXT: u32 = 1 << 0;

/// `LiteParseOcrWord.flags` bits.
pub const LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON: u32 = 1 << 0;

/// One page rendered for OCR. `pixels` indexes the view's binary arena:
/// tightly packed rows of 3 bytes per pixel for RGB or 1 for grayscale.
/// `image_rect_offset/count` index the view's `image_rects`, embedded
/// figures in viewport points.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseOcrRaster {
    pub pixels: LiteParseBlobRef,
    pub page_number: u32,
    pub width: u32,
    pub height: u32,
    /// `LITEPARSE_OCR_PIXEL_FORMAT_*`.
    pub pixel_format: u32,
    /// Effective render DPI; raster pixels map to points at `72 / dpi`.
    pub dpi: f32,
    /// `LITEPARSE_OCR_RASTER_FLAG_*` bits.
    pub flags: u32,
    pub image_rect_offset: u32,
    pub image_rect_count: u32,
}

/// Recognition outcome for one rendered page. `word_offset/count` index
/// the merge's `words`. A non-empty `error` records a recognition failure
/// and requires an empty word range.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseOcrPageInput {
    pub page_number: u32,
    pub word_offset: u32,
    pub word_count: u32,
    pub error: LiteParseStr,
}

/// One recognized word in raster pixels. `text` indexes the merge's pool.
/// Box edges are finite with positive width and height; confidence is
/// finite and in [0, 1]. With `HAS_POLYGON`, `polygon` holds four finite
/// x/y corners in reading order, forming a nondegenerate,
/// non-self-intersecting quadrilateral within the raster; otherwise it is
/// ignored.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseOcrWord {
    pub text: LiteParseStr,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub confidence: f32,
    pub polygon: [f32; 8],
    /// `LITEPARSE_OCR_WORD_FLAG_*` bits.
    pub flags: u32,
}

/// Validate `word` against a `width` x `height` raster and convert it.
pub(crate) fn ocr_result(
    word: &LiteParseOcrWord,
    pool: &[u8],
    index: usize,
    width: u32,
    height: u32,
) -> Result<OcrResult, FfiError> {
    validate_word(word, index, width, height)?;
    Ok(OcrResult {
        text: pool_str(pool, word.text, &format!("words[{index}].text"))?
            .unwrap_or_default()
            .to_owned(),
        bbox: [word.x1, word.y1, word.x2, word.y2],
        confidence: word.confidence,
        polygon: has(word.flags, LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON).then(|| {
            let [x0, y0, x1, y1, x2, y2, x3, y3] = word.polygon;
            [[x0, y0], [x1, y1], [x2, y2], [x3, y3]]
        }),
    })
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn on_segment(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
    p[0] >= a[0].min(b[0])
        && p[0] <= a[0].max(b[0])
        && p[1] >= a[1].min(b[1])
        && p[1] <= a[1].max(b[1])
}

fn intersects(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let (ab_c, ab_d, cd_a, cd_b) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    (ab_c == 0.0 && on_segment(a, b, c))
        || (ab_d == 0.0 && on_segment(a, b, d))
        || (cd_a == 0.0 && on_segment(c, d, a))
        || (cd_b == 0.0 && on_segment(c, d, b))
        || (ab_c.signum() != ab_d.signum() && cd_a.signum() != cd_b.signum())
}

fn validate_word(
    word: &LiteParseOcrWord,
    index: usize,
    width: u32,
    height: u32,
) -> Result<(), FfiError> {
    if word.flags & !LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON != 0 {
        return Err(FfiError::invalid_argument(format!(
            "words[{index}].flags contains unknown bits"
        )));
    }
    if ![word.x1, word.y1, word.x2, word.y2]
        .into_iter()
        .all(f32::is_finite)
        || word.x1 >= word.x2
        || word.y1 >= word.y2
    {
        return Err(FfiError::invalid_argument(format!(
            "words[{index}].bbox must contain finite coordinates and have positive width and height"
        )));
    }
    if !word.confidence.is_finite() || !(0.0..=1.0).contains(&word.confidence) {
        return Err(FfiError::invalid_argument(format!(
            "words[{index}].confidence must be finite and in [0, 1]"
        )));
    }
    if has(word.flags, LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON) {
        let [x0, y0, x1, y1, x2, y2, x3, y3] = word.polygon;
        let points = [[x0, y0], [x1, y1], [x2, y2], [x3, y3]];
        if points.iter().any(|[x, y]| {
            !x.is_finite()
                || !y.is_finite()
                || *x < 0.0
                || *y < 0.0
                || f64::from(*x) > f64::from(width)
                || f64::from(*y) > f64::from(height)
        }) {
            return Err(FfiError::invalid_argument(format!(
                "words[{index}].polygon must contain finite points within the raster"
            )));
        }
        let points = points.map(|p| p.map(f64::from));
        let turns = (0..4).map(|i| cross(points[i], points[(i + 1) % 4], points[(i + 2) % 4]));
        let area_twice: f64 = (0..4)
            .map(|i| {
                let a = points[i];
                let b = points[(i + 1) % 4];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum();
        if turns.into_iter().any(|turn| turn == 0.0)
            || area_twice == 0.0
            || intersects(points[0], points[1], points[2], points[3])
            || intersects(points[1], points[2], points[3], points[0])
        {
            return Err(FfiError::invalid_argument(format!(
                "words[{index}].polygon must be a nondegenerate, non-self-intersecting quadrilateral"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POOL: &[u8] = b"valid";

    fn word() -> LiteParseOcrWord {
        LiteParseOcrWord {
            text: LiteParseStr { offset: 0, len: 5 },
            x1: 1.0,
            y1: 2.0,
            x2: 20.0,
            y2: 12.0,
            confidence: 0.5,
            polygon: [1.0, 2.0, 20.0, 2.0, 20.0, 12.0, 1.0, 12.0],
            flags: LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON,
        }
    }

    fn convert(word: &LiteParseOcrWord) -> Result<OcrResult, FfiError> {
        ocr_result(word, POOL, 0, 100, 80)
    }

    #[test]
    fn validates_ocr_words_against_the_raster() {
        let valid = word();
        assert_eq!(convert(&valid).unwrap().text, "valid");

        let mut invalid = Vec::new();
        for confidence in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
            invalid.push(LiteParseOcrWord {
                confidence,
                ..valid
            });
        }
        for (x1, y2, x2) in [
            (f32::NAN, 12.0, 20.0),
            (f32::INFINITY, 12.0, 20.0),
            (1.0, f32::NEG_INFINITY, 20.0),
            (20.0, 12.0, 20.0),
            (21.0, 12.0, 20.0),
        ] {
            invalid.push(LiteParseOcrWord {
                x1,
                y2,
                x2,
                ..valid
            });
        }
        invalid.push(LiteParseOcrWord { y1: 12.0, ..valid });
        invalid.push(LiteParseOcrWord { y1: 13.0, ..valid });
        for polygon in [
            [f32::NAN, 2.0, 20.0, 2.0, 20.0, 12.0, 1.0, 12.0],
            [1.0, f32::INFINITY, 20.0, 2.0, 20.0, 12.0, 1.0, 12.0],
            [-1.0, 2.0, 20.0, 2.0, 20.0, 12.0, 1.0, 12.0],
            [1.0, 2.0, 101.0, 2.0, 20.0, 12.0, 1.0, 12.0],
            [1.0, 2.0, 20.0, 2.0, 20.0, 81.0, 1.0, 12.0],
            [1.0, 2.0, 20.0, 12.0, 20.0, 2.0, 1.0, 12.0], // crossed
            [1.0, 2.0, 10.0, 2.0, 20.0, 2.0, 1.0, 12.0],  // collinear
        ] {
            invalid.push(LiteParseOcrWord { polygon, ..valid });
        }
        invalid.push(LiteParseOcrWord {
            flags: 1 << 12,
            ..valid
        });

        invalid.push(LiteParseOcrWord {
            text: LiteParseStr { offset: 3, len: 9 },
            ..valid
        });
        for bad in invalid {
            assert!(convert(&bad).is_err());
        }

        let mut ignored = valid;
        ignored.flags = 0;
        ignored.polygon = [f32::NAN; 8];
        ignored.confidence = 0.0;
        assert!(convert(&ignored).unwrap().polygon.is_none());
        let upper = LiteParseOcrWord {
            confidence: 1.0,
            ..valid
        };
        convert(&upper).unwrap();
        let concave = LiteParseOcrWord {
            polygon: [1.0, 1.0, 9.0, 1.0, 3.0, 3.0, 1.0, 9.0],
            ..valid
        };
        convert(&concave).unwrap();
    }
}
