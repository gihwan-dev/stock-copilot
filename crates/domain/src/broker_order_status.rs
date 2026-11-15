use crate::MasterCode;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerOrderStatus {
    Pending,
    PendingCancel,
    PendingReplace,
    PartialFilled,
    Filled,
    Canceled,
    Rejected,
    CancelRejected,
    ReplaceRejected,
    Replaced,
    Unknown(MasterCode),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderOpenState {
    Possible,
    Closed,
}
impl BrokerOrderStatus {
    #[must_use]
    pub fn from_code(code: MasterCode) -> Self {
        match code.as_str() {
            "PENDING" => Self::Pending,
            "PENDING_CANCEL" => Self::PendingCancel,
            "PENDING_REPLACE" => Self::PendingReplace,
            "PARTIAL_FILLED" => Self::PartialFilled,
            "FILLED" => Self::Filled,
            "CANCELED" => Self::Canceled,
            "REJECTED" => Self::Rejected,
            "CANCEL_REJECTED" => Self::CancelRejected,
            "REPLACE_REJECTED" => Self::ReplaceRejected,
            "REPLACED" => Self::Replaced,
            _ => Self::Unknown(code),
        }
    }
    #[must_use]
    pub fn open_state(&self) -> OrderOpenState {
        match self {
            Self::Pending
            | Self::PendingCancel
            | Self::PendingReplace
            | Self::PartialFilled
            | Self::Unknown(_) => OrderOpenState::Possible,
            Self::Filled
            | Self::Canceled
            | Self::Rejected
            | Self::CancelRejected
            | Self::ReplaceRejected
            | Self::Replaced => OrderOpenState::Closed,
        }
    }
}
