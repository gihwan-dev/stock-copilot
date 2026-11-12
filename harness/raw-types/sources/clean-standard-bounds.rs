pub struct Quote;

pub fn copied<T: Copy + Default>(value: T) -> T {
    value
}

pub fn borrowed<T: ?Sized>(value: &T) -> &T {
    value
}

pub fn quote() -> Quote {
    Quote
}
