pub trait PartialOrd {
    fn lt(&self, other: &Self) -> bool;
}
