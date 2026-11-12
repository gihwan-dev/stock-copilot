pub struct Error;

pub struct Quote;

pub trait Port {
    type Item;

    fn quote(&self) -> Self::Item;
}

pub fn same<T>(value: T) -> T {
    value
}

pub fn borrowed<'book>(quote: &'book Quote) -> &'book Quote {
    quote
}

pub fn first<T: Iterator>(values: T) -> Option<T::Item> {
    Some(values)
}

pub fn quotes(source: &dyn Port<Item = Quote>) -> impl Iterator<Item = Quote> {
    source.quotes()
}

pub fn settle(quote: Quote) -> Result<(), Error> {
    quote.settle()
}

pub fn nothing(quote: Quote) -> () {
    quote.drop_it()
}

impl Port for Quote {
    type Item = Self;

    fn quote(&self) -> Self {
        Self
    }
}
