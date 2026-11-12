impl Describe for String {
    fn describe(&self) -> Description {
        Description::of(self)
    }
}
