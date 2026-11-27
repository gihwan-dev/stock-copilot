use domain::{
    AdjustmentBasis, BacktestHorizon, BaseProbabilityBacktest, CalendarDate, CalendarYear, Candle,
    CandleObservation, CandlePeriod, CandleStatus, CandleTime, Currency, DayNumber, InstrumentId,
    MonthNumber, NumericInput, OffsetSeconds, Ohlcv, Price, PriceAdjustment, Quantity,
    RiskAppetite, Timestamp,
};
use std::io::{self, BufRead, Write};

fn stamp(date: &str) -> Option<Timestamp> {
    if date.len() != 8 {
        return None;
    }
    CalendarDate::from_parts(
        CalendarYear::new(date.get(..4)?.parse().ok()?),
        MonthNumber::new(date.get(4..6)?.parse().ok()?),
        DayNumber::new(date.get(6..)?.parse().ok()?),
    )
    .ok()?
    .start_at_offset(OffsetSeconds::new(32_400))
    .ok()
}

fn index_price(value: &str) -> Option<Price> {
    Price::parse(&NumericInput::try_new(value).ok()?, Currency::Krw).ok()
}

fn parse(line: &str) -> Option<(Timestamp, Ohlcv)> {
    let mut fields = line.split('\t');
    let at = stamp(fields.next()?)?;
    let values = Ohlcv::try_new(
        index_price(fields.next()?)?,
        index_price(fields.next()?)?,
        index_price(fields.next()?)?,
        index_price(fields.next()?)?,
        Quantity::parse(&NumericInput::try_new("0").ok()?).ok()?,
    )
    .ok()?;
    match fields.next() {
        None => Some((at, values)),
        Some(_) => None,
    }
}

fn read_series(input: impl BufRead) -> io::Result<Vec<Candle>> {
    let mut rows = Vec::new();
    for (line_index, line) in input.lines().enumerate() {
        rows.push(parse(&line?).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid date or OHLC on line {}",
                    line_index.saturating_add(1)
                ),
            )
        })?);
    }
    let collected = rows
        .last()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty input"))?
        .0;
    let instrument = InstrumentId::try_new("kr-price-index")
        .map_err(|_error| io::Error::other("invalid index identifier"))?;
    rows.into_iter()
        .map(|(at, values)| {
            let time = CandleTime::try_new(at, CandlePeriod::Daily)
                .map_err(|_error| io::Error::other("invalid index bar time"))?;
            Candle::try_new(
                instrument.clone(),
                time,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(collected)),
                values,
                CandleObservation::new(collected, CandleStatus::Finalized),
            )
            .map_err(|_error| io::Error::other("invalid index candle"))
        })
        .collect()
}

fn main() -> io::Result<()> {
    let series = read_series(io::stdin().lock())?;
    let mut output = io::stdout().lock();
    for horizon in [
        BacktestHorizon::FewDays,
        BacktestHorizon::WeeksToMonths,
        BacktestHorizon::HalfYearOrMore,
    ] {
        let report = BaseProbabilityBacktest::evaluate(&series, horizon, RiskAppetite::Neutral)
            .map_err(|_error| io::Error::other("invalid series or unavailable arithmetic"))?;
        write!(output, "{report}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn rejects_invalid_ohlc_dates_and_extra_columns() {
        assert!(parse("20251127\t11\t13\t10\t12").is_some());
        for bad in [
            "20250230\t11\t13\t10\t12",
            "20251127\t11\t13\t10\t14",
            "20251127\t11\t13\t10\t12\textra",
            "20251127\t11\t13\t10",
        ] {
            assert!(parse(bad).is_none());
        }
    }
}
