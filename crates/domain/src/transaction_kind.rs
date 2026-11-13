#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionCode(String);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionCodeError {
    InvalidCode,
}
impl TransactionCode {
    pub fn try_new(value: &str) -> Result<Self, TransactionCodeError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(TransactionCodeError::InvalidCode);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnknownTransactionCode(TransactionCode);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionKind {
    Buy,
    Sell,
    SecuritiesTransferIn,
    SecuritiesTransferOut,
    Deposit,
    Withdrawal,
    ExchangeIn,
    ExchangeOut,
    Dividend,
    Interest,
    Fee,
    Unknown(UnknownTransactionCode),
}
impl TransactionKind {
    #[must_use]
    pub fn new(code: TransactionCode) -> Self {
        match code.as_str() {
            "BUY" => Self::Buy,
            "SELL" => Self::Sell,
            "SECURITIES_TRANSFER_IN" => Self::SecuritiesTransferIn,
            "SECURITIES_TRANSFER_OUT" => Self::SecuritiesTransferOut,
            "DEPOSIT" => Self::Deposit,
            "WITHDRAWAL" => Self::Withdrawal,
            "EXCHANGE_IN" => Self::ExchangeIn,
            "EXCHANGE_OUT" => Self::ExchangeOut,
            "DIVIDEND" => Self::Dividend,
            "INTEREST" => Self::Interest,
            "FEE" => Self::Fee,
            _ => Self::Unknown(UnknownTransactionCode(code)),
        }
    }
    #[must_use]
    pub fn as_code(&self) -> TransactionCode {
        TransactionCode(match self {
            Self::Buy => String::from("BUY"),
            Self::Sell => String::from("SELL"),
            Self::SecuritiesTransferIn => String::from("SECURITIES_TRANSFER_IN"),
            Self::SecuritiesTransferOut => String::from("SECURITIES_TRANSFER_OUT"),
            Self::Deposit => String::from("DEPOSIT"),
            Self::Withdrawal => String::from("WITHDRAWAL"),
            Self::ExchangeIn => String::from("EXCHANGE_IN"),
            Self::ExchangeOut => String::from("EXCHANGE_OUT"),
            Self::Dividend => String::from("DIVIDEND"),
            Self::Interest => String::from("INTEREST"),
            Self::Fee => String::from("FEE"),
            Self::Unknown(code) => return code.0.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        IsinCode, TransactionCode, TransactionCodeError, TransactionError, TransactionKind,
    };
    #[test]
    fn kind_codes_are_sealed_and_all_codes_round_trip() {
        for code in [
            "BUY",
            "SELL",
            "SECURITIES_TRANSFER_IN",
            "SECURITIES_TRANSFER_OUT",
            "DEPOSIT",
            "WITHDRAWAL",
            "EXCHANGE_IN",
            "EXCHANGE_OUT",
            "DIVIDEND",
            "INTEREST",
            "FEE",
            "FUTURE",
            "buy",
        ] {
            let kind = TransactionKind::new(TransactionCode::try_new(code).expect("코드"));
            assert_eq!(kind.as_code().as_str(), code);
            match kind {
                TransactionKind::Unknown(_) => assert!(["FUTURE", "buy"].contains(&code)),
                TransactionKind::Buy
                | TransactionKind::Sell
                | TransactionKind::SecuritiesTransferIn
                | TransactionKind::SecuritiesTransferOut
                | TransactionKind::Deposit
                | TransactionKind::Withdrawal
                | TransactionKind::ExchangeIn
                | TransactionKind::ExchangeOut
                | TransactionKind::Dividend
                | TransactionKind::Interest
                | TransactionKind::Fee => assert!(!["FUTURE", "buy"].contains(&code)),
            }
        }
        for value in ["", " ", "A\n", "한글"] {
            assert_eq!(
                TransactionCode::try_new(value),
                Err(TransactionCodeError::InvalidCode)
            );
            assert_eq!(IsinCode::try_new(value), Err(TransactionError::InvalidIsin));
        }
        let maximum = "A".repeat(64);
        assert_eq!(
            TransactionCode::try_new(&maximum).expect("상한").as_str(),
            maximum
        );
        assert_eq!(IsinCode::try_new(&maximum).expect("상한").as_str(), maximum);
        assert_eq!(
            TransactionCode::try_new(&"A".repeat(65)),
            Err(TransactionCodeError::InvalidCode)
        );
        assert_eq!(
            IsinCode::try_new(&"A".repeat(65)),
            Err(TransactionError::InvalidIsin)
        );
    }
}
