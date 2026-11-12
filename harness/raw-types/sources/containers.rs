pub fn convert(value: Result<(i32, [u16; 2]), Box<str>>, rest: &[i64]) -> Converted {
    Converted::new(value, rest)
}
