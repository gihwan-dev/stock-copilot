fn main() {
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let now = std::time::SystemTime::now();
    let _ = (arguments, now);
}
