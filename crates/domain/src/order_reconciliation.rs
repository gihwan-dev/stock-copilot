use crate::{
    BrokerOrder, BrokerOrderId, ClientOrderId, Nanosecond, OrderTerms, Quantity,
    ResolvedOrderInstrument, SpanSeconds, TimeSpan, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalSubmissionResult {
    Accepted(BrokerOrderId),
    Rejected,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSubmission {
    pub client_order_id: ClientOrderId,
    pub instrument: ResolvedOrderInstrument,
    pub terms: OrderTerms,
    pub prepared_at: Timestamp,
    pub result: LocalSubmissionResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderCoverage {
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationIssue {
    AcceptedButMissing {
        client_order_id: ClientOrderId,
        broker_order_id: BrokerOrderId,
    },
    ContentMismatch {
        client_order_id: ClientOrderId,
        broker_order_id: BrokerOrderId,
    },
    UnknownWithoutBrokerOrder {
        client_order_id: ClientOrderId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationStatus {
    Consistent,
    Mismatched,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationReport {
    checked_at: Timestamp,
    coverage: BrokerOrderCoverage,
    issues: Vec<ReconciliationIssue>,
}

impl ReconciliationReport {
    #[must_use]
    pub const fn checked_at(&self) -> Timestamp {
        self.checked_at
    }

    #[must_use]
    pub const fn coverage(&self) -> BrokerOrderCoverage {
        self.coverage
    }

    #[must_use]
    pub fn issues(&self) -> &[ReconciliationIssue] {
        &self.issues
    }

    #[must_use]
    pub fn status(&self) -> ReconciliationStatus {
        if !self.issues.is_empty() {
            return ReconciliationStatus::Mismatched;
        }
        match self.coverage {
            BrokerOrderCoverage::Complete => ReconciliationStatus::Consistent,
            BrokerOrderCoverage::Incomplete => ReconciliationStatus::Incomplete,
        }
    }
}

const UNKNOWN_MATCH_BEFORE_SECONDS: u64 = 60;
const UNKNOWN_MATCH_AFTER_SECONDS: u64 = 600;

fn ordered_quantity(terms: &OrderTerms) -> Option<Quantity> {
    match terms {
        OrderTerms::KrLimit { quantity, .. }
        | OrderTerms::KrMarket { quantity, .. }
        | OrderTerms::UsLimit { quantity, .. }
        | OrderTerms::UsMarketBuy { quantity } => Some(quantity.quantity()),
        OrderTerms::UsMarketSell { quantity } => Some(quantity.quantity()),
        OrderTerms::UsMarketAmountBuy { .. } => None,
    }
}

fn content_matches(local: &LocalSubmission, order: &BrokerOrder) -> bool {
    if order.instrument() != &local.instrument || order.side() != local.terms.side() {
        return false;
    }
    ordered_quantity(&local.terms).is_none_or(|quantity| {
        order.quantity().as_decimal().normalize() == quantity.as_decimal().normalize()
    })
}

fn span(seconds: u64) -> Option<TimeSpan> {
    TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).ok()
}

fn within_unknown_window(local: &LocalSubmission, order: &BrokerOrder) -> bool {
    let (Some(before), Some(after)) = (
        span(UNKNOWN_MATCH_BEFORE_SECONDS),
        span(UNKNOWN_MATCH_AFTER_SECONDS),
    ) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (
        local.prepared_at.checked_sub(before),
        local.prepared_at.checked_add(after),
    ) else {
        return false;
    };
    order.ordered_at() >= start && order.ordered_at() <= end
}

fn check_submission(
    local: &LocalSubmission,
    broker_orders: &[BrokerOrder],
    coverage: BrokerOrderCoverage,
) -> Option<ReconciliationIssue> {
    let complete = coverage == BrokerOrderCoverage::Complete;
    match &local.result {
        LocalSubmissionResult::Accepted(broker_order_id) => {
            let found = broker_orders
                .iter()
                .find(|order| order.order_id() == broker_order_id);
            match found {
                Some(order) if content_matches(local, order) => None,
                Some(_) => Some(ReconciliationIssue::ContentMismatch {
                    client_order_id: local.client_order_id.clone(),
                    broker_order_id: broker_order_id.clone(),
                }),
                None => complete.then(|| ReconciliationIssue::AcceptedButMissing {
                    client_order_id: local.client_order_id.clone(),
                    broker_order_id: broker_order_id.clone(),
                }),
            }
        }
        LocalSubmissionResult::Unknown => {
            let found = broker_orders
                .iter()
                .any(|order| content_matches(local, order) && within_unknown_window(local, order));
            (!found && complete).then(|| ReconciliationIssue::UnknownWithoutBrokerOrder {
                client_order_id: local.client_order_id.clone(),
            })
        }
        LocalSubmissionResult::Rejected => None,
    }
}

#[must_use]
pub fn reconcile_orders(
    submissions: &[LocalSubmission],
    broker_orders: &[BrokerOrder],
    coverage: BrokerOrderCoverage,
    checked_at: Timestamp,
) -> ReconciliationReport {
    let issues = submissions
        .iter()
        .filter_map(|local| check_submission(local, broker_orders, coverage))
        .collect();
    ReconciliationReport {
        checked_at,
        coverage,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_purchase::tests::{row, time};
    use crate::{Currency, KrOrderPrice, NumericInput, OrderSide, Price, WholeOrderQuantity};
    use rust_decimal::Decimal;

    fn order(id: &str, side: OrderSide, ordered_at: i64) -> BrokerOrder {
        let mut input = row(id, side, "39", 150);
        input.ordered_at = time(ordered_at);
        BrokerOrder::from_lookup_result(input).expect("주문")
    }

    fn terms(side: OrderSide, quantity: i64) -> OrderTerms {
        OrderTerms::KrLimit {
            side,
            quantity: WholeOrderQuantity::try_new(
                Quantity::try_new(Decimal::from(quantity)).expect("수량"),
            )
            .expect("주문 수량"),
            price: KrOrderPrice::try_new(
                Price::parse(&NumericInput::try_new("13").expect("입력"), Currency::Krw)
                    .expect("가격"),
            )
            .expect("지정가"),
        }
    }

    fn local(
        id: &str,
        side: OrderSide,
        quantity: i64,
        result: LocalSubmissionResult,
    ) -> LocalSubmission {
        LocalSubmission {
            client_order_id: ClientOrderId::try_new(id).expect("키"),
            instrument: order("probe", side, 110).instrument().clone(),
            terms: terms(side, quantity),
            prepared_at: time(100),
            result,
        }
    }

    fn accepted(id: &str) -> LocalSubmissionResult {
        LocalSubmissionResult::Accepted(BrokerOrderId::try_new(id).expect("주문번호"))
    }

    #[test]
    fn matching_accepted_order_is_consistent() {
        let report = reconcile_orders(
            &[local("c1", OrderSide::Buy, 17, accepted("b1"))],
            &[order("b1", OrderSide::Buy, 110)],
            BrokerOrderCoverage::Complete,
            time(300),
        );
        assert_eq!(report.status(), ReconciliationStatus::Consistent);
        assert_eq!(report.issues(), &[]);
        assert_eq!(report.checked_at(), time(300));
    }

    #[test]
    fn accepted_order_missing_at_broker_is_reported_only_with_complete_coverage() {
        let submissions = [local("c1", OrderSide::Buy, 17, accepted("b1"))];
        let complete =
            reconcile_orders(&submissions, &[], BrokerOrderCoverage::Complete, time(300));
        assert_eq!(complete.status(), ReconciliationStatus::Mismatched);
        assert_eq!(
            complete.issues(),
            &[ReconciliationIssue::AcceptedButMissing {
                client_order_id: ClientOrderId::try_new("c1").expect("키"),
                broker_order_id: BrokerOrderId::try_new("b1").expect("주문번호"),
            }]
        );
        let partial = reconcile_orders(
            &submissions,
            &[],
            BrokerOrderCoverage::Incomplete,
            time(300),
        );
        assert_eq!(partial.status(), ReconciliationStatus::Incomplete);
    }

    #[test]
    fn different_side_or_quantity_is_a_content_mismatch_even_when_incomplete() {
        for submission in [
            local("c1", OrderSide::Sell, 17, accepted("b1")),
            local("c1", OrderSide::Buy, 18, accepted("b1")),
        ] {
            let report = reconcile_orders(
                &[submission],
                &[order("b1", OrderSide::Buy, 110)],
                BrokerOrderCoverage::Incomplete,
                time(300),
            );
            assert_eq!(report.status(), ReconciliationStatus::Mismatched);
            assert_eq!(
                report.issues(),
                &[ReconciliationIssue::ContentMismatch {
                    client_order_id: ClientOrderId::try_new("c1").expect("키"),
                    broker_order_id: BrokerOrderId::try_new("b1").expect("주문번호"),
                }]
            );
        }
    }

    #[test]
    fn unknown_outcome_matches_by_content_inside_the_window() {
        let submission = [local(
            "c1",
            OrderSide::Buy,
            17,
            LocalSubmissionResult::Unknown,
        )];
        let inside = reconcile_orders(
            &submission,
            &[order("b9", OrderSide::Buy, 700)],
            BrokerOrderCoverage::Complete,
            time(800),
        );
        assert_eq!(inside.status(), ReconciliationStatus::Consistent);
        let outside = reconcile_orders(
            &submission,
            &[order("b9", OrderSide::Buy, 701)],
            BrokerOrderCoverage::Complete,
            time(800),
        );
        assert_eq!(
            outside.issues(),
            &[ReconciliationIssue::UnknownWithoutBrokerOrder {
                client_order_id: ClientOrderId::try_new("c1").expect("키"),
            }]
        );
    }

    #[test]
    fn rejected_submissions_and_untracked_broker_orders_are_not_issues() {
        let report = reconcile_orders(
            &[local(
                "c1",
                OrderSide::Buy,
                17,
                LocalSubmissionResult::Rejected,
            )],
            &[order("manual", OrderSide::Sell, 110)],
            BrokerOrderCoverage::Complete,
            time(300),
        );
        assert_eq!(report.status(), ReconciliationStatus::Consistent);
    }
}
