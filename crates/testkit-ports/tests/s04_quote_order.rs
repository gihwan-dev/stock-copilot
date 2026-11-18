#[cfg(test)]
mod tests {
    use application::{CacheWriteOutcome, MarketDataRepository};
    use domain::{
        Currency, InstrumentId, Nanosecond, NumericInput, OffsetSeconds, Price, Quote, Symbol,
        Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{MemoryMarketDataRepository, MemoryReply};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected ready"),
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn quote(value: &str, observed: i64, collected: i64) -> Quote {
        Quote::try_new(
            Symbol::try_new("AAPL").expect("symbol"),
            Currency::Usd,
            Some(
                Price::parse(&NumericInput::try_new(value).expect("input"), Currency::Usd)
                    .expect("price"),
            ),
            Some(at(observed)),
            at(collected),
        )
        .expect("quote")
    }
    #[test]
    fn later_collection_does_not_replace_a_newer_server_observation() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let first = quote("192.41", 90, 100);
        let delayed = quote("181.12", 80, 110);
        for (value, expected) in [
            (&first, CacheWriteOutcome::Changed),
            (&delayed, CacheWriteOutcome::OlderIgnored),
        ] {
            repository.script_store_quote().push(MemoryReply::Memory);
            assert_eq!(
                run(repository.store_quote(&instrument, value)),
                Ok(expected)
            );
        }
        repository.script_quote().push(MemoryReply::Memory);
        assert_eq!(run(repository.quote(&instrument)), Ok(Some(first)));
    }
}
