//! Loading PDFium from a file the host names, instead of the search
//! `liteparse_pdfium::Library` performs on first use.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use liteparse_pdfium::pdfium_sys;

use crate::handle::required_str;
use crate::status::{FfiError, LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE, LiteParseStatus, boundary};

/// The file the first successful call loaded; held under the lock so two
/// racing calls cannot load two libraries.
static LOADED_FROM: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Load PDFium for this process from the file at `path`: UTF-8, `path_len`
/// bytes, no terminating NUL, and absolute, so the dynamic linker opens that
/// file and never searches. Call it before the first document is opened.
///
/// Without this call, the first open searches for PDFium (the
/// `PDFIUM_LIB_PATH` environment variable, the directory baked in at build
/// time, beside this library, beside the executable, then the system search
/// path). After it, that search is never made. A process holds one PDFium:
/// calling again with the same file succeeds and does nothing, and calling
/// with another file fails with `LITEPARSE_STATUS_INVALID_ARGUMENT`. Once a
/// first open has loaded PDFium through the search, this call cannot replace
/// it and reports success.
///
/// `LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE` means the file could not be
/// loaded or does not export PDFium's API; `liteparse_last_error` says why.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_pdfium_load(
    path: *const u8,
    path_len: usize,
) -> LiteParseStatus {
    boundary(|| {
        let path = Path::new(unsafe { required_str(path, path_len, "path") }?);
        if !path.is_absolute() {
            return Err(FfiError::invalid_argument(format!(
                "the PDFium path must be absolute, so no search resolves it: {}",
                path.display()
            )));
        }
        let mut loaded = LOADED_FROM.lock().unwrap_or_else(PoisonError::into_inner);
        match loaded.as_deref() {
            Some(existing) if existing == path => return Ok(()),
            Some(existing) => {
                return Err(FfiError::invalid_argument(format!(
                    "PDFium is already loaded from {}; a process holds one PDFium, so {} is not loaded",
                    existing.display(),
                    path.display()
                )));
            }
            None => {}
        }
        pdfium_sys::dynamic::load(path)
            .map_err(|error| FfiError::new(LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE, error))?;
        *loaded = Some(path.to_path_buf());
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::LITEPARSE_STATUS_INVALID_ARGUMENT;

    fn load(path: &str) -> LiteParseStatus {
        unsafe { liteparse_pdfium_load(path.as_ptr(), path.len()) }
    }

    #[test]
    fn a_relative_path_is_refused_before_any_search() {
        assert_eq!(load("libpdfium.so"), LITEPARSE_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn a_null_path_is_refused() {
        let status = unsafe { liteparse_pdfium_load(std::ptr::null(), 0) };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
    }
}
