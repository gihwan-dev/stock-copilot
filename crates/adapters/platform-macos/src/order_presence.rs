use crate::risk_dialog::{RiskDialogBackend, RiskDialogResponse, build_risk_dialog_content};
use application::user_presence::UserPresence;
use application::{OrderConfirmation, RiskChangeConfirmation};
use domain::{
    Currency, EstimatedOrderFee, LargeOrderConfirmation, Market, Money, OrderSide, OrderSummary,
    RiskChange, SummaryLimitPrice, SummaryQuantity,
};
use std::future::Future;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

pub const ORDER_DISCLAIMER: &str = "이 주문은 내가 직접 결정해 토스증권 Open API로 보내는 주문입니다. 주문 오류·중복·체결 결과에 따른 손실은 본인이 부담합니다.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDialogFacts {
    pub instrument_name: String,
    pub symbol: String,
    pub market: Market,
    pub side: OrderSide,
    pub quantity: SummaryQuantity,
    pub limit_price: SummaryLimitPrice,
    pub expected_amount: Money,
    pub expected_fee: Option<Money>,
    pub account_suffix: String,
    pub client_order_prefix: String,
    pub large_order: LargeOrderConfirmation,
}

impl OrderDialogFacts {
    #[must_use]
    pub fn from_summary(summary: &OrderSummary) -> Self {
        Self {
            instrument_name: String::from(summary.instrument_name().as_str()),
            symbol: String::from(summary.symbol().as_str()),
            market: summary.market(),
            side: summary.side(),
            quantity: summary.quantity(),
            limit_price: summary.limit_price(),
            expected_amount: summary.expected_amount().money(),
            expected_fee: match summary.expected_fee() {
                EstimatedOrderFee::Confirmed(fee) => Some(fee.money()),
                EstimatedOrderFee::Unknown => None,
            },
            account_suffix: summary.account_suffix().to_string(),
            client_order_prefix: String::from(summary.client_order_prefix().as_str()),
            large_order: summary.expected_amount().confirmation(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDialogContent {
    pub title: String,
    pub body: String,
    pub large_order: LargeOrderConfirmation,
    pub confirm_label: &'static str,
    pub cancel_label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticationReason(String);

impl AuthenticationReason {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDialogResponse {
    Confirmed,
    Cancelled,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceOwnerAuthResponse {
    Authenticated,
    Denied,
    Unavailable,
}

pub trait OrderDialogBackend: Send + Sync {
    fn present(&self, content: &OrderDialogContent) -> OrderDialogResponse;
}

pub trait DeviceOwnerAuthBackend: Send + Sync {
    fn evaluate(
        &self,
        reason: &AuthenticationReason,
    ) -> impl Future<Output = DeviceOwnerAuthResponse> + Send;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableOrderDialogBackend;

impl OrderDialogBackend for UnavailableOrderDialogBackend {
    fn present(&self, _content: &OrderDialogContent) -> OrderDialogResponse {
        OrderDialogResponse::Unavailable
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableDeviceOwnerAuth;

impl DeviceOwnerAuthBackend for UnavailableDeviceOwnerAuth {
    fn evaluate(
        &self,
        _reason: &AuthenticationReason,
    ) -> impl Future<Output = DeviceOwnerAuthResponse> + Send {
        std::future::ready(DeviceOwnerAuthResponse::Unavailable)
    }
}

const fn side_label(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "매수",
        OrderSide::Sell => "매도",
    }
}

const fn market_label(market: &Market) -> &'static str {
    match market {
        Market::Kr => "국내",
        Market::Us => "미국",
        Market::Unknown(_) => "시장 확인 불가",
    }
}

fn money_text(money: Money) -> String {
    let unit = match money.currency() {
        Currency::Krw => "원",
        Currency::Usd => "달러",
    };
    format!("{} {unit}", money.amount().as_decimal().normalize())
}

fn quantity_text(quantity: SummaryQuantity) -> String {
    match quantity {
        SummaryQuantity::Specified(value) => format!("{}주", value.as_decimal().normalize()),
        SummaryQuantity::Absent => String::from("금액 주문"),
    }
}

#[must_use]
pub fn build_order_dialog_content(facts: &OrderDialogFacts) -> OrderDialogContent {
    let price = match facts.limit_price {
        SummaryLimitPrice::Specified(value) => money_text(value.as_money()),
        SummaryLimitPrice::Absent => String::from("시장가"),
    };
    let fee = facts
        .expected_fee
        .map_or_else(|| String::from("확인 불가"), money_text);
    let mut lines = vec![
        format!(
            "종목: {} ({}, {})",
            facts.instrument_name,
            facts.symbol,
            market_label(&facts.market)
        ),
        format!("방향: {}", side_label(facts.side)),
        format!("수량: {}", quantity_text(facts.quantity)),
        format!("가격: {price}"),
        format!("예상 금액: {}", money_text(facts.expected_amount)),
        format!("예상 수수료(추정): {fee}"),
        format!("계좌: 끝자리 {}", facts.account_suffix),
        format!("주문 번호 앞자리: {}", facts.client_order_prefix),
    ];
    if facts.large_order == LargeOrderConfirmation::Required {
        lines.push(String::from(
            "고액 주문입니다(1억 원의 80% 이상). 확인하면 고액 주문 확인을 함께 보냅니다.",
        ));
    }
    lines.push(String::from(ORDER_DISCLAIMER));
    OrderDialogContent {
        title: format!("{} 주문 최종 확인", side_label(facts.side)),
        body: lines.join("\n"),
        large_order: facts.large_order,
        confirm_label: "주문",
        cancel_label: "취소",
    }
}

#[must_use]
pub fn build_authentication_reason(facts: &OrderDialogFacts) -> AuthenticationReason {
    AuthenticationReason(format!(
        "{} {} {} 주문",
        facts.instrument_name,
        quantity_text(facts.quantity),
        side_label(facts.side)
    ))
}

const IDLE: u8 = 0;
const BUSY: u8 = 1;

struct DialogSlot<'a>(&'a AtomicU8);

impl<'a> DialogSlot<'a> {
    fn acquire(state: &'a AtomicU8) -> Option<Self> {
        state
            .compare_exchange(IDLE, BUSY, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_previous| Self(state))
    }
}

impl Drop for DialogSlot<'_> {
    fn drop(&mut self) {
        self.0.store(IDLE, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub struct MacOsPresence<Risk, Dialog, Auth> {
    risk: Risk,
    dialog: Dialog,
    auth: Auth,
    slot: AtomicU8,
    auth_calls: AtomicUsize,
}

impl<Risk, Dialog, Auth> MacOsPresence<Risk, Dialog, Auth> {
    #[must_use]
    pub const fn new(risk: Risk, dialog: Dialog, auth: Auth) -> Self {
        Self {
            risk,
            dialog,
            auth,
            slot: AtomicU8::new(IDLE),
            auth_calls: AtomicUsize::new(0),
        }
    }

    #[must_use]
    pub fn auth_calls(&self) -> usize {
        self.auth_calls.load(Ordering::SeqCst)
    }
}

impl<Risk, Dialog, Auth> MacOsPresence<Risk, Dialog, Auth>
where
    Risk: Sync,
    Dialog: OrderDialogBackend,
    Auth: DeviceOwnerAuthBackend,
{
    pub async fn confirm_facts(&self, facts: &OrderDialogFacts) -> OrderConfirmation {
        let Some(_slot) = DialogSlot::acquire(&self.slot) else {
            return OrderConfirmation::Unavailable;
        };
        match self.dialog.present(&build_order_dialog_content(facts)) {
            OrderDialogResponse::Confirmed => {}
            OrderDialogResponse::Cancelled => return OrderConfirmation::Denied,
            OrderDialogResponse::Unavailable => return OrderConfirmation::Unavailable,
        }
        let _previous = self.auth_calls.fetch_add(1, Ordering::SeqCst);
        match self
            .auth
            .evaluate(&build_authentication_reason(facts))
            .await
        {
            DeviceOwnerAuthResponse::Authenticated => OrderConfirmation::Confirmed,
            DeviceOwnerAuthResponse::Denied => OrderConfirmation::Denied,
            DeviceOwnerAuthResponse::Unavailable => OrderConfirmation::Unavailable,
        }
    }
}

impl<Risk, Dialog, Auth> UserPresence for MacOsPresence<Risk, Dialog, Auth>
where
    Risk: RiskDialogBackend,
    Dialog: OrderDialogBackend,
    Auth: DeviceOwnerAuthBackend,
{
    fn confirm_order(
        &self,
        summary: &OrderSummary,
    ) -> impl Future<Output = OrderConfirmation> + Send {
        let facts = OrderDialogFacts::from_summary(summary);
        async move { self.confirm_facts(&facts).await }
    }

    fn confirm_risk_change(
        &self,
        change: RiskChange,
    ) -> impl Future<Output = RiskChangeConfirmation> + Send {
        let content = build_risk_dialog_content(&change);
        let confirmation = match DialogSlot::acquire(&self.slot) {
            None => RiskChangeConfirmation::Unavailable,
            Some(_slot) => match self.risk.present(&content) {
                RiskDialogResponse::Confirmed => RiskChangeConfirmation::Confirmed,
                RiskDialogResponse::Cancelled => RiskChangeConfirmation::Denied,
                RiskDialogResponse::Unavailable => RiskChangeConfirmation::Unavailable,
            },
        };
        std::future::ready(confirmation)
    }
}

#[cfg(test)]
mod tests;
