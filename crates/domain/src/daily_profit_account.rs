use crate::{
    CalendarDate, Currency, ExternalFlow, FlowDirection, FlowPeriod, FxRate, KrwValuation, Money,
    SnapshotCashBasis, SnapshotError, SnapshotPrevious, Timestamp, ValuationSnapshot,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountCurrencyChange(Money);
impl AccountCurrencyChange {
    #[must_use]
    pub const fn amount(self) -> Money {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKrwChange(KrwValuation);
impl AccountKrwChange {
    #[must_use]
    pub const fn valuation(&self) -> &KrwValuation {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountChange {
    kr: Result<AccountCurrencyChange, SnapshotError>,
    us: Result<AccountCurrencyChange, SnapshotError>,
    converted: Result<AccountKrwChange, SnapshotError>,
    previous: Result<SnapshotPrevious, SnapshotError>,
}
impl AccountChange {
    #[must_use]
    pub fn between(
        today: &ValuationSnapshot,
        previous: Option<&ValuationSnapshot>,
        periods: (&FlowPeriod, &FlowPeriod),
        flow_rates: &[FxRate],
    ) -> Self {
        let pair = crate::daily_profit::validated_previous(today, previous);
        let validated = pair.and_then(|(_state, old)| validate_periods(today, old, periods));
        let native = |currency| {
            let (old, kr, us) = validated?;
            native_change(
                today,
                old,
                match currency {
                    Currency::Krw => kr,
                    Currency::Usd => us,
                },
                currency,
            )
        };
        let converted = validated
            .and_then(|(old, kr, us)| krw_change(today, old, (kr, us), flow_rates))
            .map(AccountKrwChange);
        Self {
            kr: native(Currency::Krw),
            us: native(Currency::Usd),
            converted,
            previous: pair.map(|(state, _old)| state),
        }
    }
    pub const fn currency(
        &self,
        currency: Currency,
    ) -> Result<AccountCurrencyChange, SnapshotError> {
        match currency {
            Currency::Krw => self.kr,
            Currency::Usd => self.us,
        }
    }
    pub fn krw(&self) -> Result<&AccountKrwChange, SnapshotError> {
        self.converted.as_ref().map_err(|cause| *cause)
    }
    pub const fn previous(&self) -> Result<SnapshotPrevious, SnapshotError> {
        self.previous
    }
}
fn validate_periods<'action>(
    today: &'action ValuationSnapshot,
    old: &'action ValuationSnapshot,
    periods: (&'action FlowPeriod, &'action FlowPeriod),
) -> Result<
    (
        &'action ValuationSnapshot,
        &'action crate::ConfirmedFlowPeriod,
        &'action crate::ConfirmedFlowPeriod,
    ),
    SnapshotError,
> {
    let extract = |item: &'action FlowPeriod| match item {
        FlowPeriod::Confirmed(entry) => Ok(entry),
        FlowPeriod::Uncertain(_) => Err(SnapshotError::MissingFlows),
    };
    let kr = extract(periods.0)?;
    let us = extract(periods.1)?;
    if today.cash_basis() != SnapshotCashBasis::SettledLedger
        || old.cash_basis() != SnapshotCashBasis::SettledLedger
    {
        return Err(SnapshotError::WrongCashBasis);
    }
    for (currency, period) in [(Currency::Krw, kr), (Currency::Usd, us)] {
        if period.currency() != currency
            || period.window().from() > old.settlement_date()
            || period.window().to() < today.settlement_date()
            || today.confirmed_period(currency) != Some(period)
        {
            return Err(SnapshotError::OutsideConfirmedPeriod);
        }
    }
    Ok((old, kr, us))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowInclusion {
    Inside(Timestamp),
    Outside,
}
fn included(
    date: CalendarDate,
    observed_time: Option<Timestamp>,
    old: &ValuationSnapshot,
    today: &ValuationSnapshot,
) -> Result<FlowInclusion, SnapshotError> {
    let time = observed_time.ok_or(SnapshotError::MissingFlowTime)?;
    let in_time = old.valuation_at() < time && time <= today.valuation_at();
    let effective = |snapshot: &ValuationSnapshot| {
        date <= snapshot.settlement_date() && time <= snapshot.valuation_at()
    };
    if in_time != (!effective(old) && effective(today)) {
        return Err(SnapshotError::WrongPeriod);
    }
    if in_time {
        Ok(FlowInclusion::Inside(time))
    } else {
        Ok(FlowInclusion::Outside)
    }
}
fn signed(value: Money, direction: FlowDirection) -> Money {
    match direction {
        FlowDirection::Inflow => value,
        FlowDirection::Outflow => negative(value),
    }
}
fn negative(value: Money) -> Money {
    let mut amount = value.amount().as_decimal();
    amount.set_sign_negative(!amount.is_sign_negative());
    Money::new(crate::Amount::new(amount), value.currency())
}
fn krw_change(
    today: &ValuationSnapshot,
    old: &ValuationSnapshot,
    periods: (&crate::ConfirmedFlowPeriod, &crate::ConfirmedFlowPeriod),
    rates: &[FxRate],
) -> Result<KrwValuation, SnapshotError> {
    if rates.len() > 4096 {
        return Err(SnapshotError::TooManyInputs);
    }
    let prior = crate::valuation_snapshot::aggregate(
        negative(old.account_value(Currency::Krw)?),
        negative(old.account_value(Currency::Usd)?),
        old.fx(),
        old.valuation_at(),
    )?;
    let mut result = today
        .account_krw_value()?
        .checked_add(&prior)
        .map_err(SnapshotError::Conversion)?;
    for flow in periods.0.flows().iter().chain(periods.1.flows()) {
        if let FlowInclusion::Inside(time) =
            included(flow.date(), flow.transaction_time(), old, today)?
        {
            result = result
                .checked_add(&converted_flow(*flow, time, rates)?)
                .map_err(SnapshotError::Conversion)?;
        }
    }
    Ok(result)
}
fn converted_flow(
    flow: ExternalFlow,
    time: Timestamp,
    rates: &[FxRate],
) -> Result<KrwValuation, SnapshotError> {
    let value = negative(signed(flow.amount(), flow.direction()));

    match value.currency() {
        Currency::Krw => KrwValuation::from_native_krw(value).map_err(SnapshotError::Conversion),
        Currency::Usd => {
            if value.amount().as_decimal().is_zero() {
                return Ok(KrwValuation::zero());
            }
            let candidates = rates
                .iter()
                .filter(|record| {
                    record.kind() == crate::FxRateKind::Mid
                        && record.validity_at(time) == crate::FxValidity::Valid
                })
                .copied()
                .collect::<Vec<_>>();
            let [rate] = candidates.as_slice() else {
                return Err(SnapshotError::MissingFx);
            };
            crate::valuation_snapshot::aggregate(
                crate::holding::zero(Currency::Krw),
                value,
                Some(*rate),
                time,
            )
        }
    }
}

fn native_change(
    today: &ValuationSnapshot,
    old: &ValuationSnapshot,
    period: &crate::ConfirmedFlowPeriod,
    currency: Currency,
) -> Result<AccountCurrencyChange, SnapshotError> {
    let mut result = today
        .account_value(currency)?
        .checked_sub(&old.account_value(currency)?)
        .map_err(SnapshotError::Arithmetic)?;
    for flow in period.flows() {
        if let FlowInclusion::Inside(_time) =
            included(flow.date(), flow.transaction_time(), old, today)?
        {
            result = result
                .checked_sub(&signed(flow.amount(), flow.direction()))
                .map_err(SnapshotError::Arithmetic)?;
        }
    }
    for row in today
        .exchanges()
        .iter()
        .filter(|record| record.data().currency == currency)
    {
        if let FlowInclusion::Inside(_time) = included(
            row.data().settlement_date,
            row.data().transaction_time,
            old,
            today,
        )? {
            result = result
                .checked_sub(
                    &crate::cash_ledger::cash_delta(row.data()).map_err(SnapshotError::Cash)?,
                )
                .map_err(SnapshotError::Arithmetic)?;
        }
    }
    Ok(AccountCurrencyChange(result))
}

#[cfg(test)]
mod tests {
    use super::{AccountChange, converted_flow, included, krw_change, native_change};
    use crate::valuation_snapshot::tests::{
        account_parts, at, confirmed, date, fixture, fx, money, set_exchange, snapshot, tx,
    };
    use crate::{Currency, KrwValuation, SnapshotError};
    #[test]
    fn flow_timestamp_and_zero_foreign_flow_boundaries() {
        let f = fixture(
            vec![tx(
                crate::TransactionKind::Deposit,
                Currency::Usd,
                3,
                2,
                "0",
            )],
            ("0", "0"),
            ("0", "0"),
        );
        let old = snapshot(&f, 2, &[], "1375");
        let today = snapshot(&f, 3, &[], "1375");
        assert_eq!(
            included(date(3), None, &old, &today),
            Err(SnapshotError::MissingFlowTime)
        );
        assert_eq!(
            converted_flow(confirmed(&f.up).flows()[0], at(3, 2), &[]),
            Ok(KrwValuation::zero())
        );
        assert_eq!(
            AccountChange::between(&today, Some(&old), (&f.kp, &f.up), &[])
                .previous()
                .expect("전일")
                .source(),
            crate::SnapshotSource::Recalculated
        );
        let f2 = fixture(
            vec![
                tx(
                    crate::TransactionKind::ExchangeOut,
                    Currency::Krw,
                    1,
                    2,
                    "1",
                ),
                tx(crate::TransactionKind::ExchangeIn, Currency::Usd, 1, 2, "1"),
            ],
            ("1", "0"),
            ("0", "1"),
        );
        let old2 = snapshot(&f2, 2, &[], "1375");
        let today2 = snapshot(&f2, 3, &[], "1375");
        assert_eq!(
            AccountChange::between(&today2, Some(&old2), (&f2.kp, &f2.up), &[])
                .currency(Currency::Usd)
                .expect("구간 밖 환전")
                .amount(),
            money("0", Currency::Usd)
        );
    }
    #[test]
    fn rejected_periods_and_asset_error_paths() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let old = snapshot(&f, 2, &[], "1");
        let today = snapshot(&f, 3, &[], "1");
        assert_eq!(
            AccountChange::between(&today, Some(&old), (&f.up, &f.kp), &[]).currency(Currency::Krw),
            Err(SnapshotError::OutsideConfirmedPeriod)
        );
        let zero = |currency| Ok(money("0", currency));
        let bad = account_parts(
            3,
            Err(SnapshotError::MissingPrice),
            zero(Currency::Usd),
            None,
        );
        assert_eq!(
            native_change(&bad, &old, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::MissingPrice)
        );
        assert_eq!(
            native_change(&today, &bad, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::MissingPrice)
        );
        assert_eq!(
            krw_change(&today, &bad, (confirmed(&f.kp), confirmed(&f.up)), &[]),
            Err(SnapshotError::MissingPrice)
        );
        assert_eq!(
            krw_change(&bad, &old, (confirmed(&f.kp), confirmed(&f.up)), &[]),
            Err(SnapshotError::MissingPrice)
        );
        let bad_us = account_parts(
            2,
            zero(Currency::Krw),
            Err(SnapshotError::MissingHistory),
            None,
        );
        assert_eq!(
            krw_change(&today, &bad_us, (confirmed(&f.kp), confirmed(&f.up)), &[]),
            Err(SnapshotError::MissingHistory)
        );
        let no_fx = account_parts(2, zero(Currency::Krw), Ok(money("1", Currency::Usd)), None);
        assert_eq!(
            krw_change(&today, &no_fx, (confirmed(&f.kp), confirmed(&f.up)), &[]),
            Err(SnapshotError::MissingFx)
        );
        assert_eq!(
            krw_change(
                &today,
                &old,
                (confirmed(&f.kp), confirmed(&f.up)),
                &vec![fx(at(3, 7), "1"); 4097]
            ),
            Err(SnapshotError::TooManyInputs)
        );
    }
    #[test]
    fn arithmetic_and_conversion_overflows_do_not_leak_values() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let max = money("79228162514264337593543950335", Currency::Krw);
        let old = account_parts(
            2,
            Ok(money("-1", Currency::Krw)),
            Ok(money("0", Currency::Usd)),
            None,
        );
        let today = account_parts(3, Ok(max), Ok(money("0", Currency::Usd)), None);
        assert_eq!(
            native_change(&today, &old, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            krw_change(&today, &old, (confirmed(&f.kp), confirmed(&f.up)), &[]),
            Err(SnapshotError::Conversion(
                crate::FxConversionError::Arithmetic(crate::MoneyError::Overflow)
            ))
        );
    }
    #[test]
    fn subtracting_external_flow_cannot_overflow() {
        let f = fixture(
            vec![tx(
                crate::TransactionKind::Deposit,
                Currency::Krw,
                3,
                2,
                "1",
            )],
            ("0", "0"),
            ("1", "0"),
        );
        let zero = account_parts(
            2,
            Ok(money("0", Currency::Krw)),
            Ok(money("0", Currency::Usd)),
            None,
        );
        let lowest = account_parts(
            3,
            Ok(money("-79228162514264337593543950335", Currency::Krw)),
            Ok(money("0", Currency::Usd)),
            None,
        );
        assert_eq!(
            native_change(&lowest, &zero, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            krw_change(&lowest, &zero, (confirmed(&f.kp), confirmed(&f.up)), &[]),
            Err(SnapshotError::Conversion(
                crate::FxConversionError::Arithmetic(crate::MoneyError::Overflow)
            ))
        );
    }
    #[test]
    fn exchange_cash_effect_errors_and_overflows_are_preserved() {
        let zero = account_parts(
            2,
            Ok(money("0", Currency::Krw)),
            Ok(money("0", Currency::Usd)),
            None,
        );
        let lowest = account_parts(
            3,
            Ok(money("-79228162514264337593543950335", Currency::Krw)),
            Ok(money("0", Currency::Usd)),
            None,
        );
        let f = fixture(
            vec![tx(
                crate::TransactionKind::ExchangeIn,
                Currency::Krw,
                3,
                2,
                "1",
            )],
            ("0", "0"),
            ("1", "0"),
        );
        let mut today = lowest;
        set_exchange(
            &mut today,
            tx(crate::TransactionKind::ExchangeIn, Currency::Krw, 3, 2, "1"),
        );
        assert_eq!(
            native_change(&today, &zero, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::Arithmetic(crate::MoneyError::Overflow))
        );
        let mut invalid = tx(crate::TransactionKind::ExchangeIn, Currency::Krw, 3, 2, "1")
            .data()
            .clone();
        invalid.net_amount = None;
        set_exchange(
            &mut today,
            crate::Transaction::try_new(invalid).expect("행"),
        );
        assert_eq!(
            native_change(&today, &zero, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::Cash(
                crate::CashUnavailable::MissingNetAmount
            ))
        );
    }
    #[test]
    fn exchange_without_a_processing_time_is_unavailable() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let old = snapshot(&f, 2, &[], "1");
        let mut today = snapshot(&f, 3, &[], "1");
        let mut row = tx(crate::TransactionKind::ExchangeIn, Currency::Krw, 3, 2, "1")
            .data()
            .clone();
        row.transaction_time = None;
        set_exchange(&mut today, crate::Transaction::try_new(row).expect("행"));
        assert_eq!(
            native_change(&today, &old, confirmed(&f.kp), Currency::Krw),
            Err(SnapshotError::MissingFlowTime)
        );
    }
}
