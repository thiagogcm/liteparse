use std::sync::LazyLock;

use tokio::runtime::Runtime;

use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_RUNTIME_ERROR};

static RUNTIME: LazyLock<Result<Runtime, String>> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
});

pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> FfiResult<T> {
    RUNTIME
        .as_ref()
        .map(|runtime| runtime.block_on(future))
        .map_err(|error| {
            FfiError::new(
                LITEPARSE_STATUS_RUNTIME_ERROR,
                format!("failed to create async runtime: {error}"),
            )
        })
}
