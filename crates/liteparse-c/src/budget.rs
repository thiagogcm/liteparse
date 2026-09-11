use std::cell::Cell;

use crate::status::{FfiError, FfiResult};

/// Maximum sum of flat input array bytes accepted from caller-supplied content.
pub const LITEPARSE_MAX_CONTENT_INPUT_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum size of a single caller-supplied binary payload.
pub const LITEPARSE_MAX_SINGLE_BINARY_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum sum of caller-supplied image bytes in one content input.
pub const LITEPARSE_MAX_CONTENT_BINARY_BYTES: u64 = 128 * 1024 * 1024;
/// Maximum bytes of one result's arrays and arenas. Every offset, count, and
/// arena length of a published result therefore fits in `int32_t`.
pub const LITEPARSE_MAX_RESULT_BYTES: u64 = 1024 * 1024 * 1024;

const _: () = assert!(LITEPARSE_MAX_RESULT_BYTES <= i32::MAX as u64);

/// Reject a result whose arrays and arenas total more than the result budget.
pub(crate) fn check_result_bytes(bytes: u64) -> FfiResult {
    if bytes > LITEPARSE_MAX_RESULT_BYTES {
        return Err(FfiError::resource_limit(format!(
            "result of {bytes} bytes exceeds the limit of {LITEPARSE_MAX_RESULT_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Bytes held by a packed array.
pub(crate) fn bytes_of<T>(items: &[T]) -> u64 {
    size_of_val(items) as u64
}

/// Bytes charged against one limit with checked arithmetic. Charges name
/// `where_.field`, formatted only on failure.
pub(crate) struct Budget {
    used: Cell<u64>,
    limit: u64,
}

impl Budget {
    pub(crate) const fn new(limit: u64) -> Self {
        Self {
            used: Cell::new(0),
            limit,
        }
    }

    pub(crate) fn bytes(&self, additional: usize, where_: &str, field: &str) -> FfiResult {
        let limit = self.limit;
        let label = || {
            if field.is_empty() {
                where_.to_owned()
            } else {
                format!("{where_}.{field}")
            }
        };
        let total = u64::try_from(additional)
            .ok()
            .and_then(|additional| self.used.get().checked_add(additional))
            .filter(|&total| total <= limit)
            .ok_or_else(|| {
                FfiError::resource_limit(format!("{} exceeds the limit of {limit} bytes", label()))
            })?;
        self.used.set(total);
        Ok(())
    }

    pub(crate) fn array<T>(&self, len: usize, where_: &str, field: &str) -> FfiResult {
        self.bytes(len.saturating_mul(size_of::<T>()), where_, field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::LITEPARSE_STATUS_RESOURCE_LIMIT;

    #[test]
    fn result_budget_fits_signed_32_bit_ranges() {
        check_result_bytes(LITEPARSE_MAX_RESULT_BYTES).unwrap();
        assert_eq!(
            check_result_bytes(LITEPARSE_MAX_RESULT_BYTES + 1)
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
        assert_eq!(crate::handle::packed_len(usize::MAX), u32::MAX);
    }

    #[test]
    fn checked_budgets_reject_multiplication_and_addition_overflow() {
        let budget = Budget::new(18);
        budget.bytes(10, "test", "").unwrap();
        budget.array::<u32>(2, "test", "").unwrap();
        assert_eq!(budget.used.get(), 18);
        assert_eq!(
            budget
                .array::<u32>(usize::MAX, "test", "")
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
        assert_eq!(
            budget.bytes(1, "test", "").unwrap_err().status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
    }
}
