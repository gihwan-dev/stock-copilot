pub fn label<T: Into<String>>(value: T) -> Label {
    Label::from(value)
}

pub fn count(value: impl Into<u64>) -> Count {
    Count::from(value)
}

pub fn weight<W>(value: W) -> Weight
where
    W: Into<i16>,
{
    Weight::from(value)
}
