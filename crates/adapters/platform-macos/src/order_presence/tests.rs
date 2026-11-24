use super::{
    AuthenticationReason, DeviceOwnerAuthBackend, DeviceOwnerAuthResponse, DialogSlot,
    MacOsPresence, ORDER_DISCLAIMER, OrderDialogBackend, OrderDialogContent, OrderDialogFacts,
    OrderDialogResponse, build_authentication_reason, build_order_dialog_content,
};
use crate::risk_dialog::UnavailableRiskDialogBackend;
use application::OrderConfirmation;
use domain::{
    Currency, LargeOrderConfirmation, Market, Money, NumericInput, OrderSide, Price, Quantity,
    SummaryLimitPrice, SummaryQuantity,
};
use std::future::Future;
use std::sync::RwLock;
use std::sync::atomic::AtomicU8;
use std::task::{Context, Poll, Waker};

fn run<F: Future>(future: F) -> F::Output {
    let mut pinned = std::pin::pin!(future);
    match pinned
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("즉시 완료"),
    }
}

fn input(text: &str) -> NumericInput {
    NumericInput::try_new(text).expect("숫자")
}

fn facts(large_order: LargeOrderConfirmation) -> OrderDialogFacts {
    OrderDialogFacts {
        instrument_name: String::from("삼성전자"),
        symbol: String::from("005930"),
        market: Market::Kr,
        side: OrderSide::Buy,
        quantity: SummaryQuantity::Specified(Quantity::parse(&input("10")).expect("수량")),
        limit_price: SummaryLimitPrice::Specified(
            Price::parse(&input("70000"), Currency::Krw).expect("가격"),
        ),
        expected_amount: Money::parse(&input("700000"), Currency::Krw).expect("금액"),
        expected_fee: Some(Money::parse(&input("105"), Currency::Krw).expect("수수료")),
        account_suffix: String::from("1234"),
        client_order_prefix: String::from("abcd1234"),
        large_order,
    }
}

struct ScriptedDialog {
    replies: RwLock<Vec<OrderDialogResponse>>,
    shown: RwLock<Vec<OrderDialogContent>>,
}

impl ScriptedDialog {
    fn new(reply: OrderDialogResponse) -> Self {
        Self {
            replies: RwLock::new(vec![reply]),
            shown: RwLock::new(Vec::new()),
        }
    }

    fn shown(&self) -> Vec<OrderDialogContent> {
        match self.shown.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl OrderDialogBackend for ScriptedDialog {
    fn present(&self, content: &OrderDialogContent) -> OrderDialogResponse {
        match self.shown.write() {
            Ok(mut guard) => guard.push(content.clone()),
            Err(poisoned) => poisoned.into_inner().push(content.clone()),
        }
        let reply = match self.replies.write() {
            Ok(mut guard) => guard.pop(),
            Err(poisoned) => poisoned.into_inner().pop(),
        };
        reply.unwrap_or(OrderDialogResponse::Unavailable)
    }
}

struct ScriptedAuth {
    reply: DeviceOwnerAuthResponse,
    reasons: RwLock<Vec<AuthenticationReason>>,
}

impl ScriptedAuth {
    fn new(reply: DeviceOwnerAuthResponse) -> Self {
        Self {
            reply,
            reasons: RwLock::new(Vec::new()),
        }
    }

    fn reasons(&self) -> Vec<AuthenticationReason> {
        match self.reasons.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl DeviceOwnerAuthBackend for ScriptedAuth {
    fn evaluate(
        &self,
        reason: &AuthenticationReason,
    ) -> impl Future<Output = DeviceOwnerAuthResponse> + Send {
        match self.reasons.write() {
            Ok(mut guard) => guard.push(reason.clone()),
            Err(poisoned) => poisoned.into_inner().push(reason.clone()),
        }
        std::future::ready(self.reply)
    }
}

fn presence(
    dialog: OrderDialogResponse,
    auth: DeviceOwnerAuthResponse,
) -> MacOsPresence<UnavailableRiskDialogBackend, ScriptedDialog, ScriptedAuth> {
    MacOsPresence::new(
        UnavailableRiskDialogBackend,
        ScriptedDialog::new(dialog),
        ScriptedAuth::new(auth),
    )
}

#[test]
fn dialog_shows_core_summary_estimate_and_disclaimer() {
    let content = build_order_dialog_content(&facts(LargeOrderConfirmation::NotRequired));
    assert_eq!(content.title, "매수 주문 최종 확인");
    for expected in [
        "종목: 삼성전자 (005930, 국내)",
        "방향: 매수",
        "수량: 10주",
        "가격: 70000 원",
        "예상 금액: 700000 원",
        "예상 수수료(추정): 105 원",
        "계좌: 끝자리 1234",
        "주문 번호 앞자리: abcd1234",
        ORDER_DISCLAIMER,
    ] {
        assert!(content.body.contains(expected), "빠진 줄: {expected}");
    }
    assert!(!content.body.contains("고액 주문"));
    assert_eq!(content.large_order, LargeOrderConfirmation::NotRequired);
}

#[test]
fn large_order_is_confirmed_in_the_same_dialog() {
    let content = build_order_dialog_content(&facts(LargeOrderConfirmation::Required));
    assert!(content.body.contains("고액 주문입니다"));
    assert_eq!(content.large_order, LargeOrderConfirmation::Required);
}

#[test]
fn authentication_reason_names_instrument_quantity_and_side() {
    assert_eq!(
        build_authentication_reason(&facts(LargeOrderConfirmation::NotRequired)).as_str(),
        "삼성전자 10주 매수 주문"
    );
}

#[test]
fn authentication_runs_only_after_the_dialog_is_confirmed() {
    let cases = [
        (
            OrderDialogResponse::Cancelled,
            DeviceOwnerAuthResponse::Authenticated,
            OrderConfirmation::Denied,
            0,
        ),
        (
            OrderDialogResponse::Unavailable,
            DeviceOwnerAuthResponse::Authenticated,
            OrderConfirmation::Unavailable,
            0,
        ),
        (
            OrderDialogResponse::Confirmed,
            DeviceOwnerAuthResponse::Denied,
            OrderConfirmation::Denied,
            1,
        ),
        (
            OrderDialogResponse::Confirmed,
            DeviceOwnerAuthResponse::Unavailable,
            OrderConfirmation::Unavailable,
            1,
        ),
        (
            OrderDialogResponse::Confirmed,
            DeviceOwnerAuthResponse::Authenticated,
            OrderConfirmation::Confirmed,
            1,
        ),
    ];
    for (dialog, auth, expected, auth_calls) in cases {
        let presence = presence(dialog, auth);
        let facts = facts(LargeOrderConfirmation::NotRequired);
        assert_eq!(run(presence.confirm_facts(&facts)), expected);
        assert_eq!(presence.auth_calls(), auth_calls);
        assert_eq!(presence.auth.reasons().len(), auth_calls);
        assert_eq!(presence.dialog.shown().len(), 1);
    }
}

fn hold_and_retry(state: &AtomicU8) -> (bool, bool) {
    let first = DialogSlot::acquire(state);
    (first.is_some(), DialogSlot::acquire(state).is_none())
}

#[test]
fn only_one_dialog_slot_is_held_at_a_time() {
    let state = AtomicU8::new(0);
    assert_eq!(hold_and_retry(&state), (true, true));
    assert!(DialogSlot::acquire(&state).is_some());
}
