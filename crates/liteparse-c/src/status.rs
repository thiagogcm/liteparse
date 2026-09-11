use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::handle::{LiteParseByteView, bytes_view};

/// Fixed-width status code returned by fallible API functions.
pub type LiteParseStatus = u32;

/// The operation succeeded.
pub const LITEPARSE_STATUS_OK: LiteParseStatus = 0;
/// A pointer, length, or input string was invalid.
pub const LITEPARSE_STATUS_INVALID_ARGUMENT: LiteParseStatus = 1;
/// A configuration value was invalid.
pub const LITEPARSE_STATUS_INVALID_CONFIG: LiteParseStatus = 2;
/// The document could not be opened, rendered, or parsed.
pub const LITEPARSE_STATUS_PARSE_ERROR: LiteParseStatus = 3;
/// A result could not be serialized to JSON.
pub const LITEPARSE_STATUS_SERIALIZATION_ERROR: LiteParseStatus = 4;
/// The asynchronous runtime could not be initialized.
pub const LITEPARSE_STATUS_RUNTIME_ERROR: LiteParseStatus = 5;
/// The document is encrypted and the configured password did not open it.
pub const LITEPARSE_STATUS_PASSWORD_REQUIRED: LiteParseStatus = 6;
/// A non-PDF source could not be converted: an unsupported extension, or a
/// missing external converter (LibreOffice).
pub const LITEPARSE_STATUS_CONVERSION_ERROR: LiteParseStatus = 7;
/// OCR was required and could not be performed.
pub const LITEPARSE_STATUS_OCR_ERROR: LiteParseStatus = 8;
/// The source could not be read from the filesystem.
pub const LITEPARSE_STATUS_IO_ERROR: LiteParseStatus = 9;
/// A Rust panic was caught before it crossed the C ABI boundary. Free any
/// returned handle and do not reuse it.
pub const LITEPARSE_STATUS_PANIC: LiteParseStatus = 255;

#[derive(Debug)]
pub(crate) struct FfiError {
    pub(crate) status: LiteParseStatus,
    pub(crate) message: String,
}

impl FfiError {
    pub(crate) fn new(status: LiteParseStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub(crate) fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(LITEPARSE_STATUS_INVALID_ARGUMENT, message)
    }

    pub(crate) fn invalid_config(message: impl Into<String>) -> Self {
        Self::new(LITEPARSE_STATUS_INVALID_CONFIG, message)
    }

    pub(crate) fn serialization(error: impl ToString) -> Self {
        Self::new(LITEPARSE_STATUS_SERIALIZATION_ERROR, error.to_string())
    }
}

impl From<liteparse::LiteParseError> for FfiError {
    fn from(error: liteparse::LiteParseError) -> Self {
        use liteparse::LiteParseError as Core;
        use liteparse_pdfium::PdfiumError as Pdf;

        let status = match error {
            Core::Config(_) => LITEPARSE_STATUS_INVALID_CONFIG,
            Core::Json(_) => LITEPARSE_STATUS_SERIALIZATION_ERROR,
            Core::Conversion(_) => LITEPARSE_STATUS_CONVERSION_ERROR,
            Core::Ocr(_) => LITEPARSE_STATUS_OCR_ERROR,
            Core::Io(_) => LITEPARSE_STATUS_IO_ERROR,
            Core::Pdf(Pdf::PasswordRequired) => LITEPARSE_STATUS_PASSWORD_REQUIRED,
            Core::Pdf(Pdf::FileNotFound) => LITEPARSE_STATUS_IO_ERROR,
            // Unsupported security cannot be fixed with another password.
            Core::Pdf(_) | Core::Image(_) | Core::Other(_) => LITEPARSE_STATUS_PARSE_ERROR,
        };
        Self::new(status, error.to_string())
    }
}

impl From<liteparse_pdfium::PdfiumError> for FfiError {
    fn from(error: liteparse_pdfium::PdfiumError) -> Self {
        Self::from(liteparse::LiteParseError::from(error))
    }
}

pub(crate) type FfiResult<T = ()> = Result<T, FfiError>;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn store_error(error: &FfiError) {
    LAST_ERROR.with(|slot| slot.borrow_mut().clone_from(&error.message));
}

/// Borrow this thread's most recent failure message. The view stays valid
/// until the next failed call on the same thread.
#[unsafe(no_mangle)]
pub extern "C" fn liteparse_last_error() -> LiteParseByteView {
    LAST_ERROR.with(|slot| bytes_view(slot.borrow().as_bytes()))
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

/// Run an operation at the C boundary: panics become `LITEPARSE_STATUS_PANIC`,
/// and every failure is recorded for `liteparse_last_error`.
pub(crate) fn guard<T>(operation: impl FnOnce() -> FfiResult<T>) -> Result<T, LiteParseStatus> {
    let error = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => return Ok(value),
        Ok(Err(error)) => error,
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            // Dropping the payload could itself unwind; leak it instead.
            std::mem::forget(payload);
            FfiError::new(LITEPARSE_STATUS_PANIC, format!("Rust panic: {message}"))
        }
    };
    store_error(&error);
    Err(error.status)
}

pub(crate) fn boundary(operation: impl FnOnce() -> FfiResult) -> LiteParseStatus {
    guard(operation).map_or_else(|status| status, |()| LITEPARSE_STATUS_OK)
}

pub(crate) fn suppress_panics(operation: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(operation)) {
        std::mem::forget(payload);
    }
}
