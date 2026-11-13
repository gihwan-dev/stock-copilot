use crate::{AnnualVolatility, DailyVolatility};
use std::fmt::{Display, Formatter, Result};

impl Display for DailyVolatility {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(&self.as_decimal(), f)
    }
}
impl Display for AnnualVolatility {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(&self.as_decimal(), f)
    }
}

#[cfg(test)]
mod tests {
    use crate::Volatility;
    use crate::candle::tests::series;
    #[test]
    fn typed_volatility_has_display_text_without_an_exact_numeric_accessor() {
        let value = Volatility::calculate(&series(&[
            "100", "102", "99", "101", "104", "100", "98", "103",
        ]))
        .expect("예제");
        assert_eq!(value.daily().to_string(), "0.0338667707234930947423863740");
        assert!(
            value
                .annualized()
                .expect("연율")
                .to_string()
                .starts_with("0.5376183182592344918"),
            "연율 표시"
        );
        assert_eq!(value.daily().cmp(&value.daily()), std::cmp::Ordering::Equal);
        assert_eq!(
            value
                .annualized()
                .expect("연율")
                .cmp(&value.annualized().expect("연율")),
            std::cmp::Ordering::Equal
        );
    }
}
