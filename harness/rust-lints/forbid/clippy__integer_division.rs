pub fn half(value: u8) -> Option<u8> {
    value.checked_div(1).map(|divided| divided / 2)
}
