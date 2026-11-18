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
    fn quote(value: &str, observed: Option<i64>, collected: i64) -> Quote {
        Quote::try_new(
            Symbol::try_new("AAPL").expect("symbol"),
            Currency::Usd,
            Some(
                Price::parse(&NumericInput::try_new(value).expect("input"), Currency::Usd)
                    .expect("price"),
            ),
            observed.map(at),
            at(collected),
        )
        .expect("quote")
    }
    #[test]
    fn later_collection_does_not_replace_a_newer_server_observation() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let first = quote("192.41", Some(90), 100);
        let delayed = quote("181.12", Some(80), 110);
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

    #[test]
    fn missing_server_time_does_not_erase_last_known_observation() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let first = quote("100", Some(105), 106);
        let unknown = quote("101", None, 107);
        let delayed = quote("99", Some(103), 108);
        for (value, expected) in [
            (&first, CacheWriteOutcome::Changed),
            (&unknown, CacheWriteOutcome::Changed),
            (&delayed, CacheWriteOutcome::OlderIgnored),
        ] {
            repository.script_store_quote().push(MemoryReply::Memory);
            assert_eq!(
                run(repository.store_quote(&instrument, value)),
                Ok(expected)
            );
        }
        repository.script_quote().push(MemoryReply::Memory);
        assert_eq!(run(repository.quote(&instrument)), Ok(Some(unknown)));
    }

    #[test]
    fn first_known_server_time_after_missing_is_not_replaced_by_collection_time() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let unknown = quote("100", None, 100);
        let known = quote("101", Some(90), 110);
        for value in [&unknown, &known] {
            repository.script_store_quote().push(MemoryReply::Memory);
            assert_eq!(
                run(repository.store_quote(&instrument, value)),
                Ok(CacheWriteOutcome::Changed)
            );
        }
        repository.script_quote().push(MemoryReply::Memory);
        assert_eq!(run(repository.quote(&instrument)), Ok(Some(known)));
    }

    #[test]
    fn later_known_server_time_remains_the_lower_bound_after_missing() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let first = quote("100", Some(90), 100);
        let later = quote("101", Some(95), 110);
        let unknown = quote("102", None, 120);
        let delayed = quote("99", Some(92), 130);
        for (value, expected) in [
            (&first, CacheWriteOutcome::Changed),
            (&later, CacheWriteOutcome::Changed),
            (&unknown, CacheWriteOutcome::Changed),
            (&delayed, CacheWriteOutcome::OlderIgnored),
        ] {
            repository.script_store_quote().push(MemoryReply::Memory);
            assert_eq!(
                run(repository.store_quote(&instrument, value)),
                Ok(expected)
            );
        }
        repository.script_quote().push(MemoryReply::Memory);
        assert_eq!(run(repository.quote(&instrument)), Ok(Some(unknown)));
    }

    #[test]
    fn known_server_times_are_not_forced_to_the_later_collection_time() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let unknown = quote("100", None, 100);
        let known = quote("101", Some(90), 110);
        let next = quote("102", Some(95), 120);
        for value in [&unknown, &known, &next] {
            repository.script_store_quote().push(MemoryReply::Memory);
            assert_eq!(
                run(repository.store_quote(&instrument, value)),
                Ok(CacheWriteOutcome::Changed)
            );
        }
        repository.script_quote().push(MemoryReply::Memory);
        assert_eq!(run(repository.quote(&instrument)), Ok(Some(next)));
    }

    #[test]
    fn equal_server_time_allows_a_newer_collection_to_change_the_quote() {
        let repository = MemoryMarketDataRepository::default();
        let instrument = InstrumentId::try_new("asset-one").expect("id");
        let first = quote("100", Some(90), 100);
        let updated = quote("101", Some(90), 110);
        for value in [&first, &updated] {
            repository.script_store_quote().push(MemoryReply::Memory);
            assert_eq!(
                run(repository.store_quote(&instrument, value)),
                Ok(CacheWriteOutcome::Changed)
            );
        }
        repository.script_quote().push(MemoryReply::Memory);
        assert_eq!(run(repository.quote(&instrument)), Ok(Some(updated)));
    }
}
