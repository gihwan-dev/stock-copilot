use application::{RetryDelay, SystemFailure, SystemPort, SystemPortError};
use rusqlite::ffi;

const PRIMARY_CODE_MASK: i32 = 0xff;
const OPERATION_NOT_PERMITTED: i32 = 1;
const ACCESS_DENIED: i32 = 13;
const READ_ONLY_FILESYSTEM: i32 = 30;
const NOT_A_DIRECTORY: i32 = 20;
const IS_A_DIRECTORY: i32 = 21;

pub(crate) const fn integrity() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
}

pub(crate) const fn permission() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied)
}

pub(crate) const fn unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

pub(crate) fn sqlite(error: &rusqlite::Error) -> SystemPortError {
    match error
        .sqlite_error()
        .map(|cause| cause.extended_code & PRIMARY_CODE_MASK)
    {
        Some(ffi::SQLITE_PERM | ffi::SQLITE_READONLY | ffi::SQLITE_AUTH) => permission(),
        Some(
            ffi::SQLITE_BUSY
            | ffi::SQLITE_LOCKED
            | ffi::SQLITE_NOMEM
            | ffi::SQLITE_INTERRUPT
            | ffi::SQLITE_IOERR
            | ffi::SQLITE_FULL
            | ffi::SQLITE_CANTOPEN
            | ffi::SQLITE_PROTOCOL,
        ) => unavailable(),
        Some(_) | None => integrity(),
    }
}

pub(crate) fn io(error: &std::io::Error) -> SystemPortError {
    match error.raw_os_error() {
        Some(OPERATION_NOT_PERMITTED | ACCESS_DENIED | READ_ONLY_FILESYSTEM) => permission(),
        Some(NOT_A_DIRECTORY | IS_A_DIRECTORY) => integrity(),
        Some(_) | None => unavailable(),
    }
}

pub(crate) fn read(error: &rusqlite::Error) -> SystemPortError {
    match error.sqlite_error().map(|cause| cause.extended_code) {
        Some(ffi::SQLITE_READONLY) => integrity(),
        _ => sqlite(error),
    }
}

pub(crate) fn join(error: &tokio::task::JoinError) -> SystemPortError {
    if error.is_cancelled() {
        unavailable()
    } else {
        integrity()
    }
}

#[cfg(test)]
mod tests {
    use super::{integrity, io, join, permission, unavailable};

    #[test]
    fn storage_layout_io_errors_preserve_integrity_failure() {
        for code in [20_i32, 21_i32] {
            assert_eq!(io(&std::io::Error::from_raw_os_error(code)), integrity());
        }
    }

    #[test]
    fn task_cancellation_is_unavailable_and_panic_is_integrity() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let cancelled = runtime.spawn(std::future::pending::<()>());
        cancelled.abort();
        assert_eq!(
            join(&runtime.block_on(cancelled).expect_err("cancelled")),
            unavailable()
        );
        let panicked = runtime.spawn(async {
            std::panic::resume_unwind(Box::new("test read task panic"));
        });
        assert_eq!(
            join(&runtime.block_on(panicked).expect_err("panicked")),
            integrity()
        );
    }

    #[test]
    fn thread_creation_io_preserves_permission_and_availability() {
        for code in [1_i32, 13_i32, 30_i32] {
            assert_eq!(io(&std::io::Error::from_raw_os_error(code)), permission());
        }
        for kind in [
            std::io::ErrorKind::OutOfMemory,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::Other,
        ] {
            assert_eq!(io(&std::io::Error::from(kind)), unavailable());
        }
    }
}
