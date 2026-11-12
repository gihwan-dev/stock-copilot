impl core::str::FromStr for Symbol {
    type Err = SymbolError;

    fn from_str(text: &str) -> Result<Self, SymbolError> {
        Self::parse(text)
    }
}
