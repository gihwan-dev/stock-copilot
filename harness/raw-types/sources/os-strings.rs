pub struct Paths<'path> {
    os: std::ffi::OsString,
    os_ref: &'path std::ffi::OsStr,
    c: std::ffi::CString,
    c_ref: &'path std::ffi::CStr,
    owned: std::path::PathBuf,
    borrowed: &'path std::path::Path,
}
