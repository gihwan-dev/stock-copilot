use crate::{
    AccountId, BrokerOrder, BrokerOrderError, BrokerOrderId, BrokerOrderRow, BrokerOrderSet,
    CalendarDate, ClosedOrderCompleteness, ClosedOrderIncomplete, ClosedOrderSet,
    NetPurchaseInputError, OpenOrderCoverage, OrderOpenState, Symbol, TimeInterval, Timestamp,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderedAtDateBounds {
    from: Option<CalendarDate>,
    to: Option<CalendarDate>,
}

impl OrderedAtDateBounds {
    pub fn try_new(
        from: Option<CalendarDate>,
        to: Option<CalendarDate>,
    ) -> Result<Self, NetPurchaseInputError> {
        if from.is_none() && to.is_none() {
            return Err(NetPurchaseInputError::InvalidTime);
        }
        if let (Some(start), Some(end)) = (from, to)
            && start > end
        {
            return Err(NetPurchaseInputError::InvalidTime);
        }
        Ok(Self { from, to })
    }

    #[must_use]
    pub const fn from(self) -> Option<CalendarDate> {
        self.from
    }

    #[must_use]
    pub const fn to(self) -> Option<CalendarDate> {
        self.to
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDateFilter {
    Unfiltered,
    OrderedAtRange(OrderedAtDateBounds),
}

impl OrderDateFilter {
    pub fn ordered_at_range(
        from: Option<CalendarDate>,
        to: Option<CalendarDate>,
    ) -> Result<Self, NetPurchaseInputError> {
        OrderedAtDateBounds::try_new(from, to).map(Self::OrderedAtRange)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderSymbolFilter {
    AllSymbols,
    SingleSymbol(Symbol),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderQueryFilterState {
    Unfiltered,
    Filtered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderPageExhaustion {
    Exhausted,
    MorePages,
    NotReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderPageBoundary {
    HasNext,
    LastPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderRowIntake {
    Accepted,
    Rejected,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClosedOrderQueryScope {
    account_id: AccountId,
    evaluation_window: TimeInterval,
    date_filter: OrderDateFilter,
    symbol_filter: OrderSymbolFilter,
}

impl ClosedOrderQueryScope {
    #[must_use]
    pub const fn unfiltered(account_id: AccountId, evaluation_window: TimeInterval) -> Self {
        Self {
            account_id,
            evaluation_window,
            date_filter: OrderDateFilter::Unfiltered,
            symbol_filter: OrderSymbolFilter::AllSymbols,
        }
    }

    #[must_use]
    pub const fn filtered(
        account_id: AccountId,
        evaluation_window: TimeInterval,
        date_filter: OrderDateFilter,
        symbol_filter: OrderSymbolFilter,
    ) -> Self {
        Self {
            account_id,
            evaluation_window,
            date_filter,
            symbol_filter,
        }
    }

    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    #[must_use]
    pub const fn evaluation_window(&self) -> TimeInterval {
        self.evaluation_window
    }

    #[must_use]
    pub const fn date_filter(&self) -> OrderDateFilter {
        self.date_filter
    }

    #[must_use]
    pub const fn symbol_filter(&self) -> &OrderSymbolFilter {
        &self.symbol_filter
    }

    #[must_use]
    pub const fn filter_state(&self) -> OrderQueryFilterState {
        match (&self.date_filter, &self.symbol_filter) {
            (OrderDateFilter::Unfiltered, OrderSymbolFilter::AllSymbols) => {
                OrderQueryFilterState::Unfiltered
            }
            (OrderDateFilter::Unfiltered, OrderSymbolFilter::SingleSymbol(_))
            | (
                OrderDateFilter::OrderedAtRange(_),
                OrderSymbolFilter::AllSymbols | OrderSymbolFilter::SingleSymbol(_),
            ) => OrderQueryFilterState::Filtered,
        }
    }
}

impl fmt::Debug for ClosedOrderQueryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClosedOrderQueryScope")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClosedOrderCompletenessProof {
    scope: ClosedOrderQueryScope,
    exhaustion: OrderPageExhaustion,
    completeness: ClosedOrderCompleteness,
}

impl ClosedOrderCompletenessProof {
    #[must_use]
    pub const fn scope(&self) -> &ClosedOrderQueryScope {
        &self.scope
    }

    #[must_use]
    pub const fn exhaustion(&self) -> OrderPageExhaustion {
        self.exhaustion
    }

    #[must_use]
    pub const fn completeness(&self) -> ClosedOrderCompleteness {
        self.completeness
    }

    pub(crate) fn verify_for_period(
        &self,
        period: TimeInterval,
    ) -> Result<(), crate::NetPurchaseIncomplete> {
        if let ClosedOrderCompleteness::Incomplete(reason) = self.completeness {
            return Err(crate::NetPurchaseIncomplete::Query(reason));
        }
        if self.scope.evaluation_window.start() > period.start()
            || self.scope.evaluation_window.end() < period.end()
        {
            return Err(crate::NetPurchaseIncomplete::OutsideQueryRange);
        }
        Ok(())
    }
}

impl fmt::Debug for ClosedOrderCompletenessProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClosedOrderCompletenessProof")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProvenClosedOrderSet {
    orders: ClosedOrderSet,
    proof: ClosedOrderCompletenessProof,
}

impl ProvenClosedOrderSet {
    #[must_use]
    pub const fn orders(&self) -> &ClosedOrderSet {
        &self.orders
    }

    #[must_use]
    pub const fn proof(&self) -> &ClosedOrderCompletenessProof {
        &self.proof
    }

    #[must_use]
    pub fn into_closed_order_set(self) -> ClosedOrderSet {
        self.orders
    }
}

impl fmt::Debug for ProvenClosedOrderSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProvenClosedOrderSet")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageLoopState {
    Reading,
    Exhausted,
    Interrupted,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FaultPresence {
    Clear,
    Present,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowCoverage {
    Full,
    TruncatedByObservation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowValidity {
    Valid,
    Invalid,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClosedOrderPageCollector {
    scope: ClosedOrderQueryScope,
    orders: Vec<BrokerOrder>,
    seen_ids: BTreeSet<BrokerOrderId>,
    loop_state: PageLoopState,
    exhaustion: OrderPageExhaustion,
    missing_pages: FaultPresence,
    rejected_rows: FaultPresence,
    request_failed: FaultPresence,
}

impl ClosedOrderPageCollector {
    #[must_use]
    pub const fn start(scope: ClosedOrderQueryScope) -> Self {
        Self {
            scope,
            orders: Vec::new(),
            seen_ids: BTreeSet::new(),
            loop_state: PageLoopState::Reading,
            exhaustion: OrderPageExhaustion::NotReached,
            missing_pages: FaultPresence::Clear,
            rejected_rows: FaultPresence::Clear,
            request_failed: FaultPresence::Clear,
        }
    }

    pub fn accept_row(&mut self, row: BrokerOrderRow) -> OrderRowIntake {
        if self.loop_state != PageLoopState::Reading {
            self.loop_state = PageLoopState::Interrupted;
            self.exhaustion = OrderPageExhaustion::MorePages;
            self.missing_pages = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if row.account_id != self.scope.account_id {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if row.status.open_state() != OrderOpenState::Closed {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        let Ok(order) = BrokerOrder::from_lookup_result(row) else {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        };
        if self.seen_ids.contains(order.order_id()) {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if order_execution_consistent(&order) != RowValidity::Valid {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        self.seen_ids.insert(order.order_id().clone());
        self.orders.push(order);
        OrderRowIntake::Accepted
    }

    pub fn record_unidentified_row(&mut self) {
        self.rejected_rows = FaultPresence::Present;
    }

    pub fn record_page_boundary(&mut self, boundary: OrderPageBoundary) {
        match (self.loop_state, boundary) {
            (PageLoopState::Reading, OrderPageBoundary::HasNext) => {
                self.exhaustion = OrderPageExhaustion::MorePages;
            }
            (PageLoopState::Reading, OrderPageBoundary::LastPage) => {
                self.loop_state = PageLoopState::Exhausted;
                self.exhaustion = OrderPageExhaustion::Exhausted;
            }
            (
                PageLoopState::Exhausted,
                OrderPageBoundary::HasNext | OrderPageBoundary::LastPage,
            ) => {
                self.loop_state = PageLoopState::Interrupted;
                self.exhaustion = OrderPageExhaustion::MorePages;
                self.missing_pages = FaultPresence::Present;
            }
            (
                PageLoopState::Interrupted,
                OrderPageBoundary::HasNext | OrderPageBoundary::LastPage,
            ) => {}
        }
    }

    pub fn record_page_limit_reached(&mut self) {
        self.loop_state = PageLoopState::Interrupted;
        self.exhaustion = OrderPageExhaustion::MorePages;
        self.missing_pages = FaultPresence::Present;
    }

    pub fn record_row_limit_reached(&mut self) {
        self.loop_state = PageLoopState::Interrupted;
        self.exhaustion = OrderPageExhaustion::MorePages;
        self.missing_pages = FaultPresence::Present;
    }

    pub fn record_request_failed(&mut self) {
        self.loop_state = PageLoopState::Interrupted;
        self.exhaustion = OrderPageExhaustion::NotReached;
        self.request_failed = FaultPresence::Present;
    }

    pub fn finish(
        self,
        observed_at: Timestamp,
    ) -> Result<ProvenClosedOrderSet, NetPurchaseInputError> {
        let (covered, window_coverage) =
            covered_interval(self.scope.evaluation_window, observed_at)?;
        let mut rejected_rows = self.rejected_rows;
        for order in &self.orders {
            if order_matches_observation(order, observed_at) != RowValidity::Valid {
                rejected_rows = FaultPresence::Present;
            }
        }
        let completeness = self.determine_completeness(window_coverage, rejected_rows);
        let valid_orders: BTreeMap<BrokerOrderId, BrokerOrder> = self
            .orders
            .into_iter()
            .filter(|order| order_matches_observation(order, observed_at) == RowValidity::Valid)
            .map(|order| (order.order_id().clone(), order))
            .collect();
        let proof = ClosedOrderCompletenessProof {
            scope: self.scope,
            exhaustion: self.exhaustion,
            completeness,
        };
        let orders = ClosedOrderSet::from_verified_proof(
            proof.scope.account_id.clone(),
            valid_orders,
            covered,
            observed_at,
            proof.clone(),
        );
        Ok(ProvenClosedOrderSet { orders, proof })
    }

    fn determine_completeness(
        &self,
        window_coverage: WindowCoverage,
        rejected_rows: FaultPresence,
    ) -> ClosedOrderCompleteness {
        if self.request_failed == FaultPresence::Present {
            return ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RequestFailed);
        }
        if self.scope.filter_state() == OrderQueryFilterState::Filtered
            || self.exhaustion != OrderPageExhaustion::Exhausted
            || window_coverage != WindowCoverage::Full
            || self.missing_pages == FaultPresence::Present
        {
            return ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages);
        }
        if rejected_rows == FaultPresence::Present {
            return ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RejectedRows);
        }
        ClosedOrderCompleteness::Complete
    }
}

impl fmt::Debug for ClosedOrderPageCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClosedOrderPageCollector")
            .finish_non_exhaustive()
    }
}

fn covered_interval(
    evaluation_window: TimeInterval,
    observed_at: Timestamp,
) -> Result<(TimeInterval, WindowCoverage), NetPurchaseInputError> {
    if observed_at >= evaluation_window.end() {
        return Ok((evaluation_window, WindowCoverage::Full));
    }
    let truncated = TimeInterval::try_new(evaluation_window.start(), observed_at)
        .map_err(|_error| NetPurchaseInputError::InvalidTime)?;
    Ok((truncated, WindowCoverage::TruncatedByObservation))
}

fn order_execution_consistent(order: &BrokerOrder) -> RowValidity {
    if order.filled_at().is_some_and(|at| at < order.ordered_at()) {
        return RowValidity::Invalid;
    }
    if order.filled_quantity().as_decimal().is_zero()
        && (order.filled_at().is_some()
            || order.average_filled_price().is_some()
            || match order.filled_amount() {
                crate::BrokerOrderMoney::Known(amount) => !amount.amount().as_decimal().is_zero(),
                crate::BrokerOrderMoney::Absent => false,
            })
    {
        return RowValidity::Invalid;
    }
    RowValidity::Valid
}

fn canceled_not_after(canceled: crate::BrokerOrderTime, observed_at: Timestamp) -> RowValidity {
    match canceled {
        crate::BrokerOrderTime::Absent => RowValidity::Valid,
        crate::BrokerOrderTime::Known(at) if at <= observed_at => RowValidity::Valid,
        crate::BrokerOrderTime::Known(_) => RowValidity::Invalid,
    }
}

fn order_matches_observation(order: &BrokerOrder, observed_at: Timestamp) -> RowValidity {
    if order.observed_at() == observed_at
        && order.ordered_at() <= observed_at
        && order.filled_at().is_none_or(|at| at <= observed_at)
        && canceled_not_after(order.canceled_at(), observed_at) == RowValidity::Valid
    {
        RowValidity::Valid
    } else {
        RowValidity::Invalid
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OpenOrderQueryScope {
    account_id: AccountId,
    date_filter: OrderDateFilter,
    symbol_filter: OrderSymbolFilter,
}

impl OpenOrderQueryScope {
    #[must_use]
    pub const fn unfiltered(account_id: AccountId) -> Self {
        Self {
            account_id,
            date_filter: OrderDateFilter::Unfiltered,
            symbol_filter: OrderSymbolFilter::AllSymbols,
        }
    }

    #[must_use]
    pub const fn filtered(
        account_id: AccountId,
        date_filter: OrderDateFilter,
        symbol_filter: OrderSymbolFilter,
    ) -> Self {
        Self {
            account_id,
            date_filter,
            symbol_filter,
        }
    }

    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    #[must_use]
    pub const fn date_filter(&self) -> OrderDateFilter {
        self.date_filter
    }

    #[must_use]
    pub const fn symbol_filter(&self) -> &OrderSymbolFilter {
        &self.symbol_filter
    }

    #[must_use]
    pub const fn filter_state(&self) -> OrderQueryFilterState {
        match (&self.date_filter, &self.symbol_filter) {
            (OrderDateFilter::Unfiltered, OrderSymbolFilter::AllSymbols) => {
                OrderQueryFilterState::Unfiltered
            }
            (OrderDateFilter::Unfiltered, OrderSymbolFilter::SingleSymbol(_))
            | (
                OrderDateFilter::OrderedAtRange(_),
                OrderSymbolFilter::AllSymbols | OrderSymbolFilter::SingleSymbol(_),
            ) => OrderQueryFilterState::Filtered,
        }
    }
}

impl fmt::Debug for OpenOrderQueryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenOrderQueryScope")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OpenOrderCollector {
    scope: OpenOrderQueryScope,
    rows: Vec<BrokerOrderRow>,
    seen_ids: BTreeSet<BrokerOrderId>,
    exhaustion: OrderPageExhaustion,
    rejected_rows: FaultPresence,
    closed_row: FaultPresence,
}

impl OpenOrderCollector {
    #[must_use]
    pub const fn start(scope: OpenOrderQueryScope) -> Self {
        Self {
            scope,
            rows: Vec::new(),
            seen_ids: BTreeSet::new(),
            exhaustion: OrderPageExhaustion::NotReached,
            rejected_rows: FaultPresence::Clear,
            closed_row: FaultPresence::Clear,
        }
    }

    pub fn accept_row(&mut self, row: BrokerOrderRow) -> OrderRowIntake {
        if self.exhaustion != OrderPageExhaustion::NotReached {
            self.exhaustion = OrderPageExhaustion::MorePages;
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if row.account_id != self.scope.account_id {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if self.seen_ids.contains(&row.order_id) {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if BrokerOrder::from_lookup_result(row.clone()).is_err() {
            self.rejected_rows = FaultPresence::Present;
            return OrderRowIntake::Rejected;
        }
        if row.status.open_state() == OrderOpenState::Closed {
            self.closed_row = FaultPresence::Present;
        }
        self.seen_ids.insert(row.order_id.clone());
        self.rows.push(row);
        OrderRowIntake::Accepted
    }

    pub fn record_unidentified_row(&mut self) {
        self.rejected_rows = FaultPresence::Present;
    }

    pub fn record_incomplete_page(&mut self) {
        self.exhaustion = OrderPageExhaustion::MorePages;
    }

    pub fn record_all_pages_received(&mut self) {
        match self.exhaustion {
            OrderPageExhaustion::NotReached => {
                self.exhaustion = OrderPageExhaustion::Exhausted;
            }
            OrderPageExhaustion::Exhausted | OrderPageExhaustion::MorePages => {
                self.exhaustion = OrderPageExhaustion::MorePages;
            }
        }
    }

    pub fn finish(self, observed_at: Timestamp) -> Result<BrokerOrderSet, BrokerOrderError> {
        let mut rejected_rows = self.rejected_rows;
        let mut aligned_rows = Vec::with_capacity(self.rows.len());
        for row in self.rows {
            if row.observed_at == observed_at
                && row.ordered_at <= observed_at
                && row.filled_at.is_none_or(|at| at <= observed_at)
                && canceled_not_after(row.canceled_at, observed_at) == RowValidity::Valid
            {
                aligned_rows.push(row);
            } else {
                rejected_rows = FaultPresence::Present;
            }
        }
        let coverage = if self.scope.filter_state() == OrderQueryFilterState::Unfiltered
            && self.exhaustion == OrderPageExhaustion::Exhausted
            && rejected_rows == FaultPresence::Clear
            && self.closed_row == FaultPresence::Clear
        {
            OpenOrderCoverage::Complete
        } else {
            OpenOrderCoverage::Incomplete
        };
        BrokerOrderSet::from_open_lookup_result(
            self.scope.account_id,
            aligned_rows,
            coverage,
            observed_at,
        )
    }
}

impl fmt::Debug for OpenOrderCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenOrderCollector").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClosedOrderPageCollector, ClosedOrderQueryScope, OpenOrderCollector, OpenOrderQueryScope,
        OrderDateFilter, OrderPageBoundary, OrderPageExhaustion, OrderQueryFilterState,
        OrderRowIntake, OrderSymbolFilter, OrderedAtDateBounds,
    };
    use crate::net_purchase::tests::{interval, money, row, time};
    use crate::{
        AccountId, BrokerOrderMoney, BrokerOrderStatus, CalendarDate, CalendarYear,
        ClosedOrderCompleteness, ClosedOrderIncomplete, Currency, DayNumber, MonthNumber,
        NetPurchaseError, NetPurchaseIncomplete, NetPurchaseInputError, NetPurchaseScope,
        OpenOrderCoverage, OrderSide, Quantity, Symbol, net_purchase_amount,
    };
    use rust_decimal::Decimal;

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(3),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    fn symbol(text: &str) -> Symbol {
        Symbol::try_new(text).expect("심볼")
    }

    #[test]
    fn date_filter_rejects_empty_and_reversed_bounds() {
        assert_eq!(
            OrderDateFilter::ordered_at_range(None, None),
            Err(NetPurchaseInputError::InvalidTime)
        );
        assert_eq!(
            OrderDateFilter::ordered_at_range(Some(date(10)), Some(date(9))),
            Err(NetPurchaseInputError::InvalidTime)
        );
        let equal = OrderedAtDateBounds::try_new(Some(date(10)), Some(date(10))).expect("동일일");
        assert_eq!(equal.from(), Some(date(10)));
        assert_eq!(equal.to(), Some(date(10)));
        assert_eq!(
            OrderDateFilter::ordered_at_range(Some(date(10)), Some(date(10))),
            Ok(OrderDateFilter::OrderedAtRange(equal))
        );
        let from_only = OrderedAtDateBounds::try_new(Some(date(10)), None).expect("시작일");
        assert_eq!(from_only.from(), Some(date(10)));
        assert_eq!(from_only.to(), None);
        assert_eq!(
            OrderDateFilter::ordered_at_range(Some(date(10)), None),
            Ok(OrderDateFilter::OrderedAtRange(from_only))
        );
        let to_only = OrderedAtDateBounds::try_new(None, Some(date(10))).expect("종료일");
        assert_eq!(to_only.from(), None);
        assert_eq!(to_only.to(), Some(date(10)));
        assert_eq!(
            OrderDateFilter::ordered_at_range(None, Some(date(10))),
            Ok(OrderDateFilter::OrderedAtRange(to_only))
        );
    }

    #[test]
    fn closed_scope_and_proof_accessors_and_redacted_debug() {
        let scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(0, 300));
        assert_eq!(scope.account_id(), &AccountId::new(7));
        assert_eq!(scope.evaluation_window(), interval(0, 300));
        assert_eq!(scope.date_filter(), OrderDateFilter::Unfiltered);
        assert_eq!(scope.symbol_filter(), &OrderSymbolFilter::AllSymbols);
        assert_eq!(scope.filter_state(), OrderQueryFilterState::Unfiltered);
        assert_eq!(format!("{scope:?}"), "ClosedOrderQueryScope { .. }");

        let mut collector = ClosedOrderPageCollector::start(scope);
        assert_eq!(format!("{collector:?}"), "ClosedOrderPageCollector { .. }");
        assert_eq!(
            collector.accept_row(row("b-1", OrderSide::Buy, "41", 150)),
            OrderRowIntake::Accepted
        );
        collector.record_page_boundary(OrderPageBoundary::HasNext);
        assert_eq!(
            collector.accept_row(row("s-1", OrderSide::Sell, "11", 200)),
            OrderRowIntake::Accepted
        );
        collector.record_page_boundary(OrderPageBoundary::LastPage);
        let proven = collector.finish(time(300)).expect("증거 집합");
        assert_eq!(format!("{proven:?}"), "ProvenClosedOrderSet { .. }");
        assert_eq!(
            format!("{:?}", proven.proof()),
            "ClosedOrderCompletenessProof { .. }"
        );
        assert_eq!(proven.proof().scope().account_id(), &AccountId::new(7));
        assert_eq!(proven.proof().exhaustion(), OrderPageExhaustion::Exhausted);
        assert_eq!(
            proven.proof().completeness(),
            ClosedOrderCompleteness::Complete
        );
        assert_eq!(proven.orders().orders().len(), 2);
        let closed = proven.into_closed_order_set();
        assert_eq!(
            closed.completeness_proof().expect("증거").completeness(),
            ClosedOrderCompleteness::Complete
        );
        assert_eq!(
            net_purchase_amount(
                &closed,
                interval(0, 300),
                NetPurchaseScope::Currency(Currency::Krw)
            ),
            Ok(money("30", Currency::Krw))
        );
    }

    #[test]
    fn closed_collector_forbids_complete_on_filters() {
        let filtered_date = ClosedOrderQueryScope::filtered(
            AccountId::new(7),
            interval(0, 300),
            OrderDateFilter::ordered_at_range(Some(date(1)), None).expect("범위"),
            OrderSymbolFilter::AllSymbols,
        );
        assert_eq!(
            filtered_date.filter_state(),
            OrderQueryFilterState::Filtered
        );
        let mut date_collector = ClosedOrderPageCollector::start(filtered_date);
        date_collector.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            date_collector
                .finish(time(300))
                .expect("불완전")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );

        let filtered_symbol = ClosedOrderQueryScope::filtered(
            AccountId::new(7),
            interval(0, 300),
            OrderDateFilter::Unfiltered,
            OrderSymbolFilter::SingleSymbol(symbol("005930")),
        );
        assert_eq!(
            filtered_symbol.filter_state(),
            OrderQueryFilterState::Filtered
        );
        let mut sym_collector = ClosedOrderPageCollector::start(filtered_symbol);
        sym_collector.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            sym_collector
                .finish(time(300))
                .expect("불완전")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );
    }

    #[test]
    fn closed_collector_forbids_complete_on_limits_and_request_failure() {
        let base_scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(0, 300));
        let never_ended = ClosedOrderPageCollector::start(base_scope.clone());
        assert_eq!(
            never_ended
                .finish(time(300))
                .expect("미종료")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );

        let mut page_limit = ClosedOrderPageCollector::start(base_scope.clone());
        page_limit.record_page_limit_reached();
        page_limit.record_page_boundary(OrderPageBoundary::LastPage);
        let page_limit_set = page_limit.finish(time(300)).expect("상한");
        assert_eq!(
            page_limit_set.proof().exhaustion(),
            OrderPageExhaustion::MorePages
        );
        assert_eq!(
            page_limit_set.proof().completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );

        let mut row_limit = ClosedOrderPageCollector::start(base_scope.clone());
        row_limit.record_row_limit_reached();
        row_limit.record_page_boundary(OrderPageBoundary::HasNext);
        assert_eq!(
            row_limit
                .finish(time(300))
                .expect("행 상한")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );

        let mut double_last = ClosedOrderPageCollector::start(base_scope.clone());
        double_last.record_page_boundary(OrderPageBoundary::LastPage);
        double_last.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            double_last
                .finish(time(300))
                .expect("모순")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );

        let mut request_failed = ClosedOrderPageCollector::start(base_scope);
        request_failed.accept_row(row("ok", OrderSide::Buy, "10", 150));
        request_failed.record_request_failed();
        request_failed.record_page_boundary(OrderPageBoundary::LastPage);
        let failed_set = request_failed.finish(time(300)).expect("실패");
        assert_eq!(
            failed_set.proof().exhaustion(),
            OrderPageExhaustion::NotReached
        );
        assert_eq!(
            failed_set.proof().completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RequestFailed)
        );
    }

    fn feed_invalid_closed_rows(collector: &mut ClosedOrderPageCollector) {
        let mut wrong_account = row("acc", OrderSide::Buy, "10", 150);
        wrong_account.account_id = AccountId::new(8);
        assert_eq!(
            collector.accept_row(wrong_account),
            OrderRowIntake::Rejected
        );

        let mut partial = row("partial", OrderSide::Buy, "10", 150);
        partial.status = BrokerOrderStatus::PartialFilled;
        assert_eq!(collector.accept_row(partial), OrderRowIntake::Rejected);

        let mut bad_order = row("bad-fill", OrderSide::Buy, "10", 150);
        bad_order.filled_quantity = Quantity::try_new(Decimal::from(99_i32)).expect("초과");
        assert_eq!(collector.accept_row(bad_order), OrderRowIntake::Rejected);

        assert_eq!(
            collector.accept_row(row("valid", OrderSide::Buy, "25", 150)),
            OrderRowIntake::Rejected
        );

        let mut reversed_fill = row("rev-fill", OrderSide::Buy, "10", 100);
        reversed_fill.ordered_at = time(101);
        assert_eq!(
            collector.accept_row(reversed_fill),
            OrderRowIntake::Rejected
        );

        let mut zero_with_time = row("zero-time", OrderSide::Buy, "0", 150);
        zero_with_time.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        zero_with_time.average_filled_price = None;
        assert_eq!(
            collector.accept_row(zero_with_time),
            OrderRowIntake::Rejected
        );

        let mut zero_with_amount = row("zero-amt", OrderSide::Buy, "1", 150);
        zero_with_amount.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        zero_with_amount.filled_at = None;
        zero_with_amount.average_filled_price = None;
        assert_eq!(
            collector.accept_row(zero_with_amount),
            OrderRowIntake::Rejected
        );
    }

    #[test]
    fn closed_collector_rejects_bad_rows_without_forging_complete() {
        let scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(0, 300));
        let mut collector = ClosedOrderPageCollector::start(scope);
        assert_eq!(
            collector.accept_row(row("valid", OrderSide::Buy, "25", 150)),
            OrderRowIntake::Accepted
        );
        feed_invalid_closed_rows(&mut collector);

        let mut zero_clean = row("zero-ok", OrderSide::Buy, "0", 150);
        zero_clean.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        zero_clean.filled_at = None;
        zero_clean.average_filled_price = None;
        zero_clean.filled_amount = BrokerOrderMoney::Absent;
        assert_eq!(collector.accept_row(zero_clean), OrderRowIntake::Accepted);

        let mut zero_zero_amt = row("zero-zero-amt", OrderSide::Buy, "0", 150);
        zero_zero_amt.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        zero_zero_amt.filled_at = None;
        zero_zero_amt.average_filled_price = None;
        assert_eq!(
            collector.accept_row(zero_zero_amt),
            OrderRowIntake::Accepted
        );

        let mut late_obs = row("late-obs", OrderSide::Buy, "10", 150);
        late_obs.observed_at = time(299);
        assert_eq!(collector.accept_row(late_obs), OrderRowIntake::Accepted);

        let mut future_ordered = row("future-ord", OrderSide::Buy, "10", 301);
        future_ordered.ordered_at = time(301);
        assert_eq!(
            collector.accept_row(future_ordered),
            OrderRowIntake::Accepted
        );

        let mut future_filled = row("future-fill", OrderSide::Buy, "10", 301);
        future_filled.ordered_at = time(200);
        assert_eq!(
            collector.accept_row(future_filled),
            OrderRowIntake::Accepted
        );

        collector.record_unidentified_row();
        collector.record_page_boundary(OrderPageBoundary::LastPage);
        let proven = collector.finish(time(300)).expect("불완전 집합");
        assert_eq!(
            proven.proof().completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RejectedRows)
        );
        assert_eq!(proven.orders().orders().len(), 3);
        assert_eq!(
            net_purchase_amount(
                proven.orders(),
                interval(0, 300),
                NetPurchaseScope::Currency(Currency::Krw)
            ),
            Err(NetPurchaseError::Incomplete(NetPurchaseIncomplete::Query(
                ClosedOrderIncomplete::RejectedRows
            )))
        );
    }

    #[test]
    fn closed_collector_truncates_or_rejects_early_observation() {
        let scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(0, 300));
        let mut early = ClosedOrderPageCollector::start(scope.clone());
        early.record_page_boundary(OrderPageBoundary::LastPage);
        let early_proven = early.finish(time(250)).expect("조기 관측");
        assert_eq!(early_proven.orders().covered(), interval(0, 250));
        assert_eq!(
            early_proven.proof().completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );

        let mut too_early = ClosedOrderPageCollector::start(scope);
        too_early.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            too_early.finish(time(0)),
            Err(NetPurchaseInputError::InvalidTime)
        );
    }

    fn pending_open_row() -> crate::BrokerOrderRow {
        let mut pending = row("open-1", OrderSide::Buy, "0", 150);
        pending.status = BrokerOrderStatus::Pending;
        pending.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        pending.filled_at = None;
        pending.filled_amount = BrokerOrderMoney::Absent;
        pending
    }

    #[test]
    fn open_collector_enforces_unfiltered_scope_and_closed_mix() {
        let scope = OpenOrderQueryScope::unfiltered(AccountId::new(7));
        assert_eq!(scope.account_id(), &AccountId::new(7));
        assert_eq!(scope.date_filter(), OrderDateFilter::Unfiltered);
        assert_eq!(scope.symbol_filter(), &OrderSymbolFilter::AllSymbols);
        assert_eq!(scope.filter_state(), OrderQueryFilterState::Unfiltered);
        assert_eq!(format!("{scope:?}"), "OpenOrderQueryScope { .. }");

        let pending = pending_open_row();
        let mut open = OpenOrderCollector::start(scope.clone());
        assert_eq!(format!("{open:?}"), "OpenOrderCollector { .. }");
        assert_eq!(open.accept_row(pending.clone()), OrderRowIntake::Accepted);
        open.record_all_pages_received();
        let set = open.finish(time(300)).expect("완전 미체결");
        assert_eq!(set.coverage(), OpenOrderCoverage::Complete);
        assert_eq!(set.orders().len(), 1);

        let filtered_sym = OpenOrderQueryScope::filtered(
            AccountId::new(7),
            OrderDateFilter::Unfiltered,
            OrderSymbolFilter::SingleSymbol(symbol("005930")),
        );
        assert_eq!(filtered_sym.filter_state(), OrderQueryFilterState::Filtered);
        let mut sym_open = OpenOrderCollector::start(filtered_sym);
        sym_open.accept_row(pending);
        sym_open.record_all_pages_received();
        assert_eq!(
            sym_open.finish(time(300)).expect("필터").coverage(),
            OpenOrderCoverage::Incomplete
        );

        let filtered_date = OpenOrderQueryScope::filtered(
            AccountId::new(7),
            OrderDateFilter::ordered_at_range(Some(date(1)), Some(date(5))).expect("날짜"),
            OrderSymbolFilter::AllSymbols,
        );
        assert_eq!(
            filtered_date.filter_state(),
            OrderQueryFilterState::Filtered
        );
        let mut date_open = OpenOrderCollector::start(filtered_date);
        date_open.record_all_pages_received();
        assert_eq!(
            date_open.finish(time(300)).expect("날짜 필터").coverage(),
            OpenOrderCoverage::Incomplete
        );

        let mut with_closed = OpenOrderCollector::start(scope);
        assert_eq!(
            with_closed.accept_row(row("closed-in-open", OrderSide::Sell, "10", 150)),
            OrderRowIntake::Accepted
        );
        with_closed.record_all_pages_received();
        let closed_set = with_closed.finish(time(300)).expect("종료 혼입");
        assert_eq!(closed_set.coverage(), OpenOrderCoverage::Incomplete);
        assert_eq!(closed_set.orders().len(), 1);
    }

    #[test]
    fn open_collector_marks_incomplete_on_faults_and_double_completion() {
        let scope = OpenOrderQueryScope::unfiltered(AccountId::new(7));
        let pending = pending_open_row();
        let mut with_faults = OpenOrderCollector::start(scope.clone());
        let mut wrong_acc = pending.clone();
        wrong_acc.account_id = AccountId::new(9);
        assert_eq!(with_faults.accept_row(wrong_acc), OrderRowIntake::Rejected);
        let mut bad_order = pending.clone();
        bad_order.filled_quantity = Quantity::try_new(Decimal::from(99_i32)).expect("초과");
        assert_eq!(with_faults.accept_row(bad_order), OrderRowIntake::Rejected);
        assert_eq!(
            with_faults.accept_row(pending.clone()),
            OrderRowIntake::Accepted
        );
        assert_eq!(
            with_faults.accept_row(pending.clone()),
            OrderRowIntake::Rejected
        );
        let mut wrong_obs = pending;
        wrong_obs.order_id = crate::BrokerOrderId::try_new("obs-diff").expect("id");
        wrong_obs.observed_at = time(299);
        assert_eq!(with_faults.accept_row(wrong_obs), OrderRowIntake::Accepted);
        with_faults.record_unidentified_row();
        with_faults.record_incomplete_page();
        with_faults.record_all_pages_received();
        let fault_set = with_faults.finish(time(300)).expect("결함");
        assert_eq!(fault_set.coverage(), OpenOrderCoverage::Incomplete);
        assert_eq!(fault_set.orders().len(), 1);

        let mut double_done = OpenOrderCollector::start(scope);
        double_done.record_all_pages_received();
        double_done.record_all_pages_received();
        assert_eq!(
            double_done.finish(time(300)).expect("중복 완료").coverage(),
            OpenOrderCoverage::Incomplete
        );
    }

    #[test]
    fn proven_d38_verifies_proof_period_and_rejects_post_completion_rows() {
        let scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(100, 200));
        let mut ok_collector = ClosedOrderPageCollector::start(scope.clone());
        let mut with_cancel = row("c-ok", OrderSide::Buy, "0", 120);
        with_cancel.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        with_cancel.filled_at = None;
        with_cancel.average_filled_price = None;
        with_cancel.filled_amount = BrokerOrderMoney::Absent;
        with_cancel.canceled_at = crate::BrokerOrderTime::Known(time(150));
        assert_eq!(
            ok_collector.accept_row(with_cancel),
            OrderRowIntake::Accepted
        );
        assert_eq!(
            ok_collector.accept_row(row("b-ok", OrderSide::Buy, "5000", 140)),
            OrderRowIntake::Accepted
        );
        ok_collector.record_page_boundary(OrderPageBoundary::LastPage);
        let proven = ok_collector.finish(time(300)).expect("완전");
        assert_eq!(
            crate::proven_net_purchases(&proven, interval(100, 200)).currency(Currency::Krw),
            Ok(money("5000", Currency::Krw))
        );
        assert_eq!(
            crate::proven_net_purchase_amount(
                &proven,
                interval(50, 200),
                NetPurchaseScope::Currency(Currency::Krw)
            ),
            Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::OutsideQueryRange
            ))
        );
        assert_eq!(
            crate::proven_net_purchase_amount(
                &proven,
                interval(100, 250),
                NetPurchaseScope::Currency(Currency::Krw)
            ),
            Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::OutsideQueryRange
            ))
        );
        let mut late_closed = ClosedOrderPageCollector::start(scope);
        late_closed.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            late_closed.accept_row(row("late", OrderSide::Buy, "100", 140)),
            OrderRowIntake::Rejected
        );
        let late_proven = late_closed.finish(time(300)).expect("완료 뒤 주입");
        assert_eq!(
            crate::proven_net_purchase_amount(
                &late_proven,
                interval(100, 200),
                NetPurchaseScope::Currency(Currency::Krw)
            ),
            Err(NetPurchaseError::Incomplete(NetPurchaseIncomplete::Query(
                ClosedOrderIncomplete::MissingPages
            )))
        );
    }

    #[test]
    fn collectors_reject_zero_fill_avg_price_and_future_timestamps() {
        let closed_scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(0, 200));
        let mut closed = ClosedOrderPageCollector::start(closed_scope);
        let mut zero_avg = row("z-avg", OrderSide::Buy, "0", 120);
        zero_avg.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        zero_avg.filled_at = None;
        zero_avg.filled_amount = BrokerOrderMoney::Absent;
        zero_avg.average_filled_price =
            Some(crate::Price::try_new(money("100", Currency::Krw)).expect("p"));
        assert_eq!(closed.accept_row(zero_avg), OrderRowIntake::Rejected);
        let mut future_cancel = row("f-cancel", OrderSide::Buy, "0", 120);
        future_cancel.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        future_cancel.filled_at = None;
        future_cancel.average_filled_price = None;
        future_cancel.filled_amount = BrokerOrderMoney::Absent;
        future_cancel.canceled_at = crate::BrokerOrderTime::Known(time(301));
        assert_eq!(closed.accept_row(future_cancel), OrderRowIntake::Accepted);
        closed.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            closed
                .finish(time(300))
                .expect("미래 취소")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RejectedRows)
        );

        let open_scope = OpenOrderQueryScope::unfiltered(AccountId::new(7));
        let mut open = OpenOrderCollector::start(open_scope.clone());
        let mut future_open = pending_open_row();
        future_open.ordered_at = time(301);
        assert_eq!(open.accept_row(future_open), OrderRowIntake::Accepted);
        open.record_all_pages_received();
        let set = open.finish(time(300)).expect("미래 미체결");
        assert_eq!(set.coverage(), OpenOrderCoverage::Incomplete);
        assert!(set.orders().is_empty());

        let mut late_open = OpenOrderCollector::start(open_scope);
        late_open.record_all_pages_received();
        assert_eq!(
            late_open.accept_row(pending_open_row()),
            OrderRowIntake::Rejected
        );
        assert_eq!(
            late_open
                .finish(time(300))
                .expect("종료 뒤 미체결")
                .coverage(),
            OpenOrderCoverage::Incomplete
        );
    }

    #[test]
    fn isolated_fault_signals_individually_prevent_complete() {
        let closed_scope = ClosedOrderQueryScope::unfiltered(AccountId::new(7), interval(0, 200));
        let mut only_unid = ClosedOrderPageCollector::start(closed_scope.clone());
        only_unid.record_unidentified_row();
        only_unid.record_page_boundary(OrderPageBoundary::LastPage);
        assert_eq!(
            only_unid
                .finish(time(300))
                .expect("미식별")
                .proof()
                .completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RejectedRows)
        );

        let mut only_row_limit = ClosedOrderPageCollector::start(closed_scope.clone());
        only_row_limit.record_row_limit_reached();
        only_row_limit.record_page_boundary(OrderPageBoundary::LastPage);
        let limited = only_row_limit.finish(time(300)).expect("행 상한");
        assert_eq!(
            limited.proof().completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );
        assert_eq!(limited.proof().exhaustion(), OrderPageExhaustion::MorePages);

        let mut only_future_cancel = ClosedOrderPageCollector::start(closed_scope);
        let mut future_cancel = row("fc-only", OrderSide::Buy, "0", 120);
        future_cancel.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("0");
        future_cancel.filled_at = None;
        future_cancel.average_filled_price = None;
        future_cancel.filled_amount = BrokerOrderMoney::Absent;
        future_cancel.canceled_at = crate::BrokerOrderTime::Known(time(301));
        assert_eq!(
            only_future_cancel.accept_row(future_cancel),
            OrderRowIntake::Accepted
        );
        only_future_cancel.record_page_boundary(OrderPageBoundary::LastPage);
        let cancel_proven = only_future_cancel.finish(time(300)).expect("미래 취소");
        assert_eq!(
            cancel_proven.proof().completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RejectedRows)
        );
        assert!(cancel_proven.orders().orders().is_empty());

        let open_scope = OpenOrderQueryScope::unfiltered(AccountId::new(7));
        let mut open_unid = OpenOrderCollector::start(open_scope.clone());
        open_unid.record_unidentified_row();
        open_unid.record_all_pages_received();
        assert_eq!(
            open_unid
                .finish(time(300))
                .expect("미체결 미식별")
                .coverage(),
            OpenOrderCoverage::Incomplete
        );

        let mut open_inc = OpenOrderCollector::start(open_scope);
        open_inc.record_incomplete_page();
        open_inc.record_all_pages_received();
        assert_eq!(
            open_inc.finish(time(300)).expect("미체결 잔여").coverage(),
            OpenOrderCoverage::Incomplete
        );
    }
}
