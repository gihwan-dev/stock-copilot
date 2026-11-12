#[derive(Debug)]
pub struct Positive(pub u8);

impl From<u8> for Positive {
    fn from(value: u8) -> Self {
        if value == 0 {
            panic!("zero");
        }
        Self(value)
    }
}
