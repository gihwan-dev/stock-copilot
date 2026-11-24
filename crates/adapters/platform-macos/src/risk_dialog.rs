use application::user_presence::UserPresence;
use application::{OrderConfirmation, RiskChangeConfirmation};
use domain::{OrderSummary, ProfileWeight, RiskAppetite, RiskChange};
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskChangeDialogContent {
    pub title: String,
    pub body: String,
    pub confirm_label: &'static str,
    pub cancel_label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskDialogResponse {
    Confirmed,
    Cancelled,
    Unavailable,
}

pub trait RiskDialogBackend: Send + Sync {
    fn present(&self, content: &RiskChangeDialogContent) -> RiskDialogResponse;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableRiskDialogBackend;

impl RiskDialogBackend for UnavailableRiskDialogBackend {
    fn present(&self, _content: &RiskChangeDialogContent) -> RiskDialogResponse {
        RiskDialogResponse::Unavailable
    }
}

#[derive(Debug)]
pub struct MacOsRiskDialogPresence<B> {
    backend: B,
    dialog_lock: RwLock<()>,
    biometric_calls: AtomicUsize,
}

impl MacOsRiskDialogPresence<UnavailableRiskDialogBackend> {
    #[must_use]
    pub fn unavailable() -> Self {
        Self::new(UnavailableRiskDialogBackend)
    }
}

impl<B> MacOsRiskDialogPresence<B> {
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self {
            backend,
            dialog_lock: RwLock::new(()),
            biometric_calls: AtomicUsize::new(0),
        }
    }

    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    #[must_use]
    pub fn biometric_calls(&self) -> usize {
        self.biometric_calls.load(Ordering::SeqCst)
    }
}

impl<B: RiskDialogBackend> UserPresence for MacOsRiskDialogPresence<B> {
    fn confirm_order(
        &self,
        _summary: &OrderSummary,
    ) -> impl Future<Output = OrderConfirmation> + Send {
        std::future::ready(OrderConfirmation::Unavailable)
    }

    fn confirm_risk_change(
        &self,
        change: RiskChange,
    ) -> impl Future<Output = RiskChangeConfirmation> + Send {
        let content = build_risk_dialog_content(&change);
        let response = match self.dialog_lock.write() {
            Ok(_guard) => self.backend.present(&content),
            Err(poisoned) => {
                let _guard = poisoned.into_inner();
                self.backend.present(&content)
            }
        };
        let confirmation = match response {
            RiskDialogResponse::Confirmed => RiskChangeConfirmation::Confirmed,
            RiskDialogResponse::Cancelled => RiskChangeConfirmation::Denied,
            RiskDialogResponse::Unavailable => RiskChangeConfirmation::Unavailable,
        };
        std::future::ready(confirmation)
    }
}

#[must_use]
pub fn build_risk_dialog_content(change: &RiskChange) -> RiskChangeDialogContent {
    let (title, body) = match change {
        RiskChange::EnableOrders => (
            String::from("주문 기능 켜기 확인"),
            String::from(
                "주문 기능을 켜면 매수·매도 카드에 승인 버튼이 표시됩니다. 실제 주문은 매번 별도 확인을 거쳐 전송됩니다. 주문 기능을 켜시겠습니까?",
            ),
        ),
        RiskChange::AllowMarketOrders => (
            String::from("시장가 주문 허용 확인"),
            String::from(
                "시장가 주문을 허용하면 체결 가격이 예상과 달라질 수 있습니다. 시장가 주문을 허용하시겠습니까?",
            ),
        ),
        RiskChange::AllowHours => (
            String::from("짧은 투자 기간 허용 확인"),
            String::from(
                "몇 시간 단위 투자 기간을 허용하면 변동성이 큰 단기 신호가 생성될 수 있습니다. 몇 시간 단위 기간을 허용하시겠습니까?",
            ),
        ),
        RiskChange::RelaxWeightCaps(caps) => (
            String::from("비중 상한 완화 확인"),
            format!(
                "비중 상한을 종목 {}, 섹터 {}로 높입니다. 특정 종목이나 섹터에 자산이 더 많이 집중될 수 있습니다. 변경하시겠습니까?",
                format_weight_percent(caps.instrument_cap()),
                format_weight_percent(caps.sector_cap()),
            ),
        ),
        RiskChange::IncreaseRiskAppetite(appetite) => {
            let label = appetite_label(*appetite);
            (
                String::from("위험 성향 변경 확인"),
                format!(
                    "위험 성향을 '{label}'으로 높입니다. 더 큰 가격 변동과 손실 가능성을 감수하게 됩니다. 변경하시겠습니까?"
                ),
            )
        }
    };
    RiskChangeDialogContent {
        title,
        body,
        confirm_label: "확인",
        cancel_label: "취소",
    }
}

const fn appetite_label(appetite: RiskAppetite) -> &'static str {
    match appetite {
        RiskAppetite::Conservative => "보수적",
        RiskAppetite::Neutral => "중립",
        RiskAppetite::Aggressive => "공격적",
    }
}

fn format_weight_percent(weight: ProfileWeight) -> String {
    ratio_text_to_percent(&weight.as_ratio().as_decimal().to_string())
}

fn ratio_text_to_percent(raw: &str) -> String {
    let (whole_part, frac_part) = raw.split_once('.').unwrap_or((raw, ""));
    let mut whole_digits = String::from(whole_part);
    let mut frac_chars = frac_part.chars();
    for _ in 0_u8..2 {
        match frac_chars.next() {
            Some(digit) => whole_digits.push(digit),
            None => whole_digits.push('0'),
        }
    }
    let trimmed_whole = whole_digits.trim_start_matches('0');
    let whole = if trimmed_whole.is_empty() {
        "0"
    } else {
        trimmed_whole
    };
    let rest: String = frac_chars.collect();
    let trimmed_frac = rest.trim_end_matches('0');
    if trimmed_frac.is_empty() {
        format!("{whole}%")
    } else {
        format!("{whole}.{trimmed_frac}%")
    }
}

#[cfg(test)]
mod tests;
