use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;

pub struct Quote;

pub struct QuoteId;

pub struct BookError;

pub struct Book {
    quotes: Vec<Quote>,
    best: Option<Quote>,
    shared: Arc<Quote>,
    boxed: Box<Quote>,
    index: BTreeMap<QuoteId, Quote>,
    ids: BTreeSet<QuoteId>,
    marker: PhantomData<Quote>,
    owned: alloc::vec::Vec<Quote>,
}

pub fn compare(left: &Quote, right: &Quote) -> Ordering {
    left.compare(right)
}

pub fn load(id: QuoteId) -> Result<Quote, BookError> {
    id.load()
}

impl fmt::Display for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("quote")
    }
}

pub fn bounds<T>(value: T) -> T
where
    T: PartialEq + Eq + PartialOrd + Ord + Hash + fmt::Debug + fmt::Display + Error + Clone,
    T: Send + Sync + FromStr,
{
    value
}

pub fn hashes<H: Hasher>(state: &mut H) -> Quote {
    Quote::hash_into(state)
}

pub fn iterate(values: impl Iterator<Item = Quote>) -> impl Future<Output = Quote> {
    Quote::first(values)
}

pub fn calls(first: impl Fn(Quote) -> Quote, second: impl FnMut(Quote), third: impl FnOnce(Quote)) -> Quote {
    Quote::call(first, second, third)
}

pub fn converts(from: impl From<Quote>, into: impl Into<Quote>) -> Quote {
    Quote::convert(from, into)
}
