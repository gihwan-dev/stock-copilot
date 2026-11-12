pub fn set() {
    unsafe { std::env::set_var("HARNESS", "1") };
}
