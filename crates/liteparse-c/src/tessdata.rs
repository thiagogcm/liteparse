//! Tesseract over a tessdata directory the host provisioned: a missing
//! language fails the recognition instead of being downloaded into it.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use liteparse::ocr::tesseract::TesseractOcrEngine;
use liteparse::ocr::{OcrEngine, OcrOptions, OcrResult};

type Recognition<'a> = Pin<
    Box<
        dyn Future<Output = Result<Vec<OcrResult>, Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'a,
    >,
>;

/// The built-in engine, checked against `dir` before every recognition.
pub(crate) struct ProvisionedTesseract {
    inner: Arc<dyn OcrEngine>,
    dir: PathBuf,
}

/// Wraps the core's Tesseract when the configuration names a tessdata
/// directory, which the host then owns: nothing is downloaded into it. Any
/// other engine, or no directory, is returned unchanged.
pub(crate) fn provisioned(
    engine: Option<Arc<dyn OcrEngine>>,
    tessdata_path: Option<&str>,
) -> Option<Arc<dyn OcrEngine>> {
    match (engine, tessdata_path) {
        (Some(inner), Some(dir)) if inner.name() == "tesseract" => {
            Some(Arc::new(ProvisionedTesseract {
                inner,
                dir: PathBuf::from(dir),
            }))
        }
        (engine, _) => engine,
    }
}

impl ProvisionedTesseract {
    /// The first language file the spec needs that the directory lacks.
    fn missing(&self, language: &str) -> Option<PathBuf> {
        TesseractOcrEngine::normalize_language(language)
            .split('+')
            .map(|code| self.dir.join(format!("{code}.traineddata")))
            .find(|file| !file.is_file())
    }
}

impl OcrEngine for ProvisionedTesseract {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn prefers_grayscale(&self) -> bool {
        self.inner.prefers_grayscale()
    }

    fn recognize<'a, 'b: 'a, 'c: 'a>(
        &'a self,
        image_data: &'c [u8],
        width: u32,
        height: u32,
        options: &'b OcrOptions,
    ) -> Recognition<'a> {
        if let Some(file) = self.missing(&options.language) {
            let message = format!(
                "Tesseract language data is not provisioned: {} is missing, and nothing is \
                 downloaded into a tessdata directory the host names",
                file.display()
            );
            return Box::pin(async move { Err(message.into()) });
        }
        self.inner.recognize(image_data, width, height, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(dir: &std::path::Path) -> ProvisionedTesseract {
        ProvisionedTesseract {
            inner: Arc::new(TesseractOcrEngine::new(Some(dir.display().to_string()))),
            dir: dir.to_path_buf(),
        }
    }

    fn options(language: &str) -> OcrOptions {
        OcrOptions {
            language: language.to_owned(),
            dpi: 150.0,
        }
    }

    #[test]
    fn a_missing_language_fails_without_writing_into_the_directory() {
        let dir = std::env::temp_dir().join(format!("liteparse-c-tessdata-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result =
            crate::runtime::block_on(engine(&dir).recognize(&[0; 4], 2, 2, &options("en")))
                .expect("runtime");
        let message = result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("eng.traineddata"), "{message}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_component_of_a_combined_spec_is_checked() {
        let dir = std::env::temp_dir().join(format!(
            "liteparse-c-tessdata-combined-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("eng.traineddata"), b"present").unwrap();
        let missing = engine(&dir).missing("eng+por");
        assert_eq!(missing, Some(dir.join("por.traineddata")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn other_engines_and_an_unset_directory_are_left_alone() {
        assert!(provisioned(None, Some("/tmp")).is_none());
        let core: Arc<dyn OcrEngine> = Arc::new(TesseractOcrEngine::new(None));
        let unchanged = provisioned(Some(core.clone()), None).unwrap();
        assert!(Arc::ptr_eq(&unchanged, &core));
    }
}
