#[derive(Debug)]
pub struct Pair {
    pub left: u8,
    pub right: u8,
}

pub fn left(pair: Pair) -> (u8, u8) {
    let Pair { left, right, .. } = pair;
    (left, right)
}
