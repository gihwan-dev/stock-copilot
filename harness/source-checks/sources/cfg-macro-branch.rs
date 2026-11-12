pub fn mode() -> u8 {
    if cfg!(test) { 1 } else { 2 }
}
