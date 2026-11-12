pub struct Name(String);

impl Name {
    pub fn as_parts(&self) -> (String, Vec<f64>) {
        (self.0.clone(), Vec::new())
    }
}
