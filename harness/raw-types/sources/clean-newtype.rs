use rust_decimal::Decimal;

pub struct Price(Decimal);

impl Price {
    pub fn new(value: Decimal) -> Self {
        Self(value)
    }

    pub fn as_decimal(&self) -> Decimal {
        self.0
    }
}

pub struct Quantity(u64);

impl Quantity {
    pub fn try_new(value: u64) -> Result<Self, QuantityError> {
        Count::check(value).map(|()| Self(value))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

pub struct Name(String);

impl Name {
    pub fn new(value: &str) -> Option<Name> {
        Text::check(value).map(|text| Self(text.owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct Payload(Vec<u8>);

impl Payload {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
