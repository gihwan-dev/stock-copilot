#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Account {
    #[serde(rename = "accountNo")]
    pub no: ::std::string::String,
    #[serde(rename = "accountSeq")]
    pub seq: i64,
    #[serde(rename = "accountType")]
    pub kind: AccountAccountType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum AccountAccountType {
    #[serde(rename = "BROKERAGE")]
    Brokerage,
    #[serde(rename = "OVERSEAS_DERIVATIVES")]
    OverseasDerivatives,
    #[serde(rename = "PENSION_SAVINGS")]
    PensionSavings,
    #[serde(rename = "RESHORING_INVESTMENT")]
    ReshoringInvestment,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AfterMarketSession {
    #[serde(rename = "endTime")]
    pub end: ::std::string::String,
    #[serde(
        rename = "singlePriceAuctionEndTime",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub single_price_auction_end: ::std::option::Option<::std::string::String>,
    #[serde(rename = "startTime")]
    pub start: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ApiError {
    pub code: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub data: ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    pub message: ::std::string::String,
    #[serde(rename = "requestId")]
    pub request_id: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ApiResponse {
    pub result: ::serde_json::Value,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum AssetClass {
    #[serde(rename = "STOCK")]
    Stock,
    #[serde(rename = "BOND")]
    Bond,
    #[serde(rename = "OPTION")]
    Option,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct BuyingPowerResponse {
    #[serde(rename = "cashBuyingPower")]
    pub cash_buying_power: BuyingPowerResponseCashBuyingPower,
    pub currency: Currency,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct BuyingPowerResponseCashBuyingPower(::std::string::String);
impl ::std::ops::Deref for BuyingPowerResponseCashBuyingPower {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<BuyingPowerResponseCashBuyingPower> for ::std::string::String {
    fn from(value: BuyingPowerResponseCashBuyingPower) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for BuyingPowerResponseCashBuyingPower {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for BuyingPowerResponseCashBuyingPower {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BuyingPowerResponseCashBuyingPower {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for BuyingPowerResponseCashBuyingPower {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Candle {
    #[serde(rename = "closePrice")]
    pub close_price: CandleClosePrice,
    pub currency: Currency,
    #[serde(rename = "highPrice")]
    pub high_price: CandleHighPrice,
    #[serde(rename = "lowPrice")]
    pub low_price: CandleLowPrice,
    #[serde(rename = "openPrice")]
    pub open_price: CandleOpenPrice,
    pub timestamp: ::std::string::String,
    pub volume: CandleVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CandleClosePrice(::std::string::String);
impl ::std::ops::Deref for CandleClosePrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CandleClosePrice> for ::std::string::String {
    fn from(value: CandleClosePrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CandleClosePrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CandleClosePrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CandleClosePrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CandleClosePrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CandleHighPrice(::std::string::String);
impl ::std::ops::Deref for CandleHighPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CandleHighPrice> for ::std::string::String {
    fn from(value: CandleHighPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CandleHighPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CandleHighPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CandleHighPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CandleHighPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CandleLowPrice(::std::string::String);
impl ::std::ops::Deref for CandleLowPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CandleLowPrice> for ::std::string::String {
    fn from(value: CandleLowPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CandleLowPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CandleLowPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CandleLowPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CandleLowPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CandleOpenPrice(::std::string::String);
impl ::std::ops::Deref for CandleOpenPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CandleOpenPrice> for ::std::string::String {
    fn from(value: CandleOpenPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CandleOpenPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CandleOpenPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CandleOpenPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CandleOpenPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CandlePageResponse {
    pub candles: ::std::vec::Vec<Candle>,
    #[serde(
        rename = "nextBefore",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_before: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CandleVolume(::std::string::String);
impl ::std::ops::Deref for CandleVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CandleVolume> for ::std::string::String {
    fn from(value: CandleVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CandleVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CandleVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CandleVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CandleVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CfdBalance {
    #[serde(rename = "buyBalanceQuantity")]
    pub buy_balance_quantity: CfdBalanceBuyBalanceQuantity,
    #[serde(rename = "buyBalanceRate")]
    pub buy_balance_rate: CfdBalanceBuyBalanceRate,
    #[serde(rename = "sellBalanceQuantity")]
    pub sell_balance_quantity: CfdBalanceSellBalanceQuantity,
    #[serde(rename = "sellBalanceRate")]
    pub sell_balance_rate: CfdBalanceSellBalanceRate,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CfdBalanceBuyBalanceQuantity(::std::string::String);
impl ::std::ops::Deref for CfdBalanceBuyBalanceQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CfdBalanceBuyBalanceQuantity> for ::std::string::String {
    fn from(value: CfdBalanceBuyBalanceQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CfdBalanceBuyBalanceQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CfdBalanceBuyBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CfdBalanceBuyBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CfdBalanceBuyBalanceQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CfdBalanceBuyBalanceRate(::std::string::String);
impl ::std::ops::Deref for CfdBalanceBuyBalanceRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CfdBalanceBuyBalanceRate> for ::std::string::String {
    fn from(value: CfdBalanceBuyBalanceRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CfdBalanceBuyBalanceRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CfdBalanceBuyBalanceRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CfdBalanceBuyBalanceRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CfdBalanceBuyBalanceRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CfdBalanceSellBalanceQuantity(::std::string::String);
impl ::std::ops::Deref for CfdBalanceSellBalanceQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CfdBalanceSellBalanceQuantity> for ::std::string::String {
    fn from(value: CfdBalanceSellBalanceQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CfdBalanceSellBalanceQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CfdBalanceSellBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CfdBalanceSellBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CfdBalanceSellBalanceQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CfdBalanceSellBalanceRate(::std::string::String);
impl ::std::ops::Deref for CfdBalanceSellBalanceRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CfdBalanceSellBalanceRate> for ::std::string::String {
    fn from(value: CfdBalanceSellBalanceRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CfdBalanceSellBalanceRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CfdBalanceSellBalanceRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CfdBalanceSellBalanceRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CfdBalanceSellBalanceRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Commission {
    #[serde(rename = "commissionRate")]
    pub rate: CommissionCommissionRate,
    #[serde(
        rename = "endDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub end_date: ::std::option::Option<::std::string::String>,
    #[serde(rename = "marketCountry")]
    pub market_country: MarketCountry,
    #[serde(
        rename = "startDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub start_date: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CommissionCommissionRate(::std::string::String);
impl ::std::ops::Deref for CommissionCommissionRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CommissionCommissionRate> for ::std::string::String {
    fn from(value: CommissionCommissionRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CommissionCommissionRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CommissionCommissionRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommissionCommissionRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CommissionCommissionRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionRequest {
    #[serde(
        rename = "orderPrice",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub order_price: ::std::option::Option<ConditionRequestOrderPrice>,
    #[serde(rename = "orderSide")]
    pub order_side: ConditionRequestOrderSide,
    #[serde(rename = "triggerPrice")]
    pub trigger_price: ConditionRequestTriggerPrice,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ConditionRequestOrderPrice(::std::string::String);
impl ::std::ops::Deref for ConditionRequestOrderPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ConditionRequestOrderPrice> for ::std::string::String {
    fn from(value: ConditionRequestOrderPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ConditionRequestOrderPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ConditionRequestOrderPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionRequestOrderPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ConditionRequestOrderPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionRequestOrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ConditionRequestTriggerPrice(::std::string::String);
impl ::std::ops::Deref for ConditionRequestTriggerPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ConditionRequestTriggerPrice> for ::std::string::String {
    fn from(value: ConditionRequestTriggerPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ConditionRequestTriggerPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ConditionRequestTriggerPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionRequestTriggerPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ConditionRequestTriggerPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionalOrderCondition {
    #[serde(
        rename = "orderPrice",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub order_price: ::std::option::Option<::std::string::String>,
    #[serde(rename = "orderSide")]
    pub order_side: ConditionalOrderConditionOrderSide,
    pub status: ConditionalOrderConditionStatus,
    #[serde(
        rename = "targetProfitRate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub target_profit_rate: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "triggerPrice",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub trigger_price: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "triggeredOrderId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub triggered_order_id: ::std::option::Option<::std::string::String>,
    #[serde(rename = "type")]
    pub type_: ConditionalOrderConditionType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderConditionOrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderConditionStatus {
    #[serde(rename = "WATCHING")]
    Watching,
    #[serde(rename = "HOLDING")]
    Holding,
    #[serde(rename = "PAUSED")]
    Paused,
    #[serde(rename = "ORDERING")]
    Ordering,
    #[serde(rename = "ORDERED")]
    Ordered,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "EXPIRED")]
    Expired,
    #[serde(rename = "CANCELED")]
    Canceled,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderConditionType {
    #[serde(rename = "STOP")]
    Stop,
    #[serde(rename = "PROFIT_RATE")]
    ProfitRate,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionalOrderCreateRequest {
    #[serde(
        rename = "clientOrderId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub client_order_id: ::std::option::Option<ConditionalOrderCreateRequestClientOrderId>,
    #[serde(rename = "confirmHighValueOrder", default)]
    pub confirm_high_value_order: bool,
    #[serde(rename = "expireDate")]
    pub expire_date: ::std::string::String,
    pub first: ConditionRequest,
    #[serde(rename = "orderType")]
    pub order_type: ConditionalOrderCreateRequestOrderType,
    pub quantity: ConditionalOrderCreateRequestQuantity,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub second: ::std::option::Option<ConditionRequest>,
    pub symbol: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ConditionalOrderCreateRequestType,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ConditionalOrderCreateRequestClientOrderId(::std::string::String);
impl ::std::ops::Deref for ConditionalOrderCreateRequestClientOrderId {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ConditionalOrderCreateRequestClientOrderId> for ::std::string::String {
    fn from(value: ConditionalOrderCreateRequestClientOrderId) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ConditionalOrderCreateRequestClientOrderId {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 36_usize {
            return Err("longer than 36 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^[a-zA-Z0-9\\-_]+$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-zA-Z0-9\\-_]+$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderCreateRequestClientOrderId {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderCreateRequestClientOrderId {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ConditionalOrderCreateRequestClientOrderId {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderCreateRequestOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ConditionalOrderCreateRequestQuantity(::std::string::String);
impl ::std::ops::Deref for ConditionalOrderCreateRequestQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ConditionalOrderCreateRequestQuantity> for ::std::string::String {
    fn from(value: ConditionalOrderCreateRequestQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ConditionalOrderCreateRequestQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderCreateRequestQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderCreateRequestQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ConditionalOrderCreateRequestQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderCreateRequestType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "OCO")]
    Oco,
    #[serde(rename = "OTO")]
    Oto,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionalOrderCreateResponse {
    #[serde(
        rename = "clientOrderId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub client_order_id: ::std::option::Option<::std::string::String>,
    #[serde(rename = "conditionalOrderId")]
    pub conditional_order_id: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionalOrderDetailResponse {
    #[serde(rename = "conditionalOrderId")]
    pub conditional_order_id: ::std::string::String,
    #[serde(rename = "createdAt")]
    pub created_at: ::std::string::String,
    #[serde(
        rename = "expireDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub expire_date: ::std::option::Option<::std::string::String>,
    pub first: ConditionalOrderCondition,
    pub market: ConditionalOrderDetailResponseMarket,
    #[serde(rename = "orderType")]
    pub order_type: ConditionalOrderDetailResponseOrderType,
    pub quantity: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub second: ::std::option::Option<ConditionalOrderCondition>,
    pub status: ConditionalOrderDetailResponseStatus,
    pub symbol: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ConditionalOrderDetailResponseType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderDetailResponseMarket {
    #[serde(rename = "KR")]
    Kr,
    #[serde(rename = "US")]
    Us,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderDetailResponseOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderDetailResponseStatus {
    #[serde(rename = "WATCHING")]
    Watching,
    #[serde(rename = "PAUSED")]
    Paused,
    #[serde(rename = "ORDERING")]
    Ordering,
    #[serde(rename = "ORDERED")]
    Ordered,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "EXPIRED")]
    Expired,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderDetailResponseType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "OCO")]
    Oco,
    #[serde(rename = "OTO")]
    Oto,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionalOrderModifyRequest {
    #[serde(rename = "confirmHighValueOrder", default)]
    pub confirm_high_value_order: bool,
    #[serde(rename = "expireDate")]
    pub expire_date: ::std::string::String,
    pub first: ConditionRequest,
    #[serde(rename = "orderType")]
    pub order_type: ConditionalOrderModifyRequestOrderType,
    pub quantity: ConditionalOrderModifyRequestQuantity,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub second: ::std::option::Option<ConditionRequest>,
    #[serde(rename = "type")]
    pub type_: ConditionalOrderModifyRequestType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderModifyRequestOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ConditionalOrderModifyRequestQuantity(::std::string::String);
impl ::std::ops::Deref for ConditionalOrderModifyRequestQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ConditionalOrderModifyRequestQuantity> for ::std::string::String {
    fn from(value: ConditionalOrderModifyRequestQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ConditionalOrderModifyRequestQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderModifyRequestQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderModifyRequestQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ConditionalOrderModifyRequestQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ConditionalOrderModifyRequestType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "OCO")]
    Oco,
    #[serde(rename = "OTO")]
    Oto,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConditionalOrderResponse {
    #[serde(rename = "conditionalOrderId")]
    pub conditional_order_id: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Cost {
    pub commission: CostCommission,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tax: ::std::option::Option<CostTax>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CostCommission(::std::string::String);
impl ::std::ops::Deref for CostCommission {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CostCommission> for ::std::string::String {
    fn from(value: CostCommission) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CostCommission {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CostCommission {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CostCommission {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CostCommission {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CostTax(::std::string::String);
impl ::std::ops::Deref for CostTax {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CostTax> for ::std::string::String {
    fn from(value: CostTax) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CostTax {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CostTax {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CostTax {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CostTax {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CreditTradeDetail {
    #[serde(rename = "balanceQuantity")]
    pub balance_quantity: CreditTradeDetailBalanceQuantity,
    #[serde(rename = "balanceRate")]
    pub balance_rate: CreditTradeDetailBalanceRate,
    #[serde(rename = "newQuantity")]
    pub new_quantity: CreditTradeDetailNewQuantity,
    #[serde(rename = "returnQuantity")]
    pub return_quantity: CreditTradeDetailReturnQuantity,
    #[serde(rename = "tradingRate")]
    pub trading_rate: CreditTradeDetailTradingRate,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CreditTradeDetailBalanceQuantity(::std::string::String);
impl ::std::ops::Deref for CreditTradeDetailBalanceQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CreditTradeDetailBalanceQuantity> for ::std::string::String {
    fn from(value: CreditTradeDetailBalanceQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CreditTradeDetailBalanceQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CreditTradeDetailBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CreditTradeDetailBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CreditTradeDetailBalanceQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CreditTradeDetailBalanceRate(::std::string::String);
impl ::std::ops::Deref for CreditTradeDetailBalanceRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CreditTradeDetailBalanceRate> for ::std::string::String {
    fn from(value: CreditTradeDetailBalanceRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CreditTradeDetailBalanceRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CreditTradeDetailBalanceRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CreditTradeDetailBalanceRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CreditTradeDetailBalanceRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CreditTradeDetailNewQuantity(::std::string::String);
impl ::std::ops::Deref for CreditTradeDetailNewQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CreditTradeDetailNewQuantity> for ::std::string::String {
    fn from(value: CreditTradeDetailNewQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CreditTradeDetailNewQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CreditTradeDetailNewQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CreditTradeDetailNewQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CreditTradeDetailNewQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CreditTradeDetailReturnQuantity(::std::string::String);
impl ::std::ops::Deref for CreditTradeDetailReturnQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CreditTradeDetailReturnQuantity> for ::std::string::String {
    fn from(value: CreditTradeDetailReturnQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CreditTradeDetailReturnQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CreditTradeDetailReturnQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CreditTradeDetailReturnQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CreditTradeDetailReturnQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CreditTradeDetailTradingRate(::std::string::String);
impl ::std::ops::Deref for CreditTradeDetailTradingRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CreditTradeDetailTradingRate> for ::std::string::String {
    fn from(value: CreditTradeDetailTradingRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CreditTradeDetailTradingRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for CreditTradeDetailTradingRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CreditTradeDetailTradingRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for CreditTradeDetailTradingRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CreditTradeRecord {
    pub date: ::std::string::String,
    #[serde(
        rename = "marginLoan",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub margin_loan: ::std::option::Option<CreditTradeDetail>,
    #[serde(
        rename = "stockLoan",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stock_loan: ::std::option::Option<CreditTradeDetail>,
    #[serde(rename = "updatedAt")]
    pub updated_at: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CreditTradesResponse {
    #[serde(
        rename = "nextUntil",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_until: ::std::option::Option<::std::string::String>,
    pub records: ::std::vec::Vec<CreditTradeRecord>,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum Currency {
    #[serde(rename = "KRW")]
    Krw,
    #[serde(rename = "USD")]
    Usd,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DailyProfitLoss {
    pub amount: DailyProfitLossAmount,
    pub rate: DailyProfitLossRate,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DailyProfitLossAmount(::std::string::String);
impl ::std::ops::Deref for DailyProfitLossAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DailyProfitLossAmount> for ::std::string::String {
    fn from(value: DailyProfitLossAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DailyProfitLossAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for DailyProfitLossAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DailyProfitLossAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DailyProfitLossAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DailyProfitLossRate(::std::string::String);
impl ::std::ops::Deref for DailyProfitLossRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DailyProfitLossRate> for ::std::string::String {
    fn from(value: DailyProfitLossRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DailyProfitLossRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for DailyProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DailyProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DailyProfitLossRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ErrorResponse {
    pub error: ApiError,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ExchangeRateResponse {
    #[serde(rename = "baseCurrency")]
    pub base_currency: Currency,
    #[serde(rename = "basisPoint")]
    pub basis_point: ExchangeRateResponseBasisPoint,
    #[serde(rename = "midRate")]
    pub mid_rate: ExchangeRateResponseMidRate,
    #[serde(rename = "quoteCurrency")]
    pub quote_currency: Currency,
    pub rate: ExchangeRateResponseRate,
    #[serde(rename = "rateChangeType")]
    pub rate_change_type: ExchangeRateResponseRateChangeType,
    #[serde(rename = "validFrom")]
    pub valid_from: ::std::string::String,
    #[serde(rename = "validUntil")]
    pub valid_until: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ExchangeRateResponseBasisPoint(::std::string::String);
impl ::std::ops::Deref for ExchangeRateResponseBasisPoint {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ExchangeRateResponseBasisPoint> for ::std::string::String {
    fn from(value: ExchangeRateResponseBasisPoint) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ExchangeRateResponseBasisPoint {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ExchangeRateResponseBasisPoint {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExchangeRateResponseBasisPoint {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ExchangeRateResponseBasisPoint {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ExchangeRateResponseMidRate(::std::string::String);
impl ::std::ops::Deref for ExchangeRateResponseMidRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ExchangeRateResponseMidRate> for ::std::string::String {
    fn from(value: ExchangeRateResponseMidRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ExchangeRateResponseMidRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ExchangeRateResponseMidRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExchangeRateResponseMidRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ExchangeRateResponseMidRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ExchangeRateResponseRate(::std::string::String);
impl ::std::ops::Deref for ExchangeRateResponseRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ExchangeRateResponseRate> for ::std::string::String {
    fn from(value: ExchangeRateResponseRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ExchangeRateResponseRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ExchangeRateResponseRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExchangeRateResponseRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ExchangeRateResponseRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ExchangeRateResponseRateChangeType {
    #[serde(rename = "UP")]
    Up,
    #[serde(rename = "EQUAL")]
    Equal,
    #[serde(rename = "DOWN")]
    Down,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ForeignerHolding {
    #[serde(rename = "holdingQuantity")]
    pub holding_quantity: ForeignerHoldingHoldingQuantity,
    #[serde(rename = "holdingRate")]
    pub holding_rate: ForeignerHoldingHoldingRate,
    #[serde(rename = "limitQuantity")]
    pub limit_quantity: ForeignerHoldingLimitQuantity,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ForeignerHoldingHoldingQuantity(::std::string::String);
impl ::std::ops::Deref for ForeignerHoldingHoldingQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ForeignerHoldingHoldingQuantity> for ::std::string::String {
    fn from(value: ForeignerHoldingHoldingQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ForeignerHoldingHoldingQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ForeignerHoldingHoldingQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ForeignerHoldingHoldingQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ForeignerHoldingHoldingQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ForeignerHoldingHoldingRate(::std::string::String);
impl ::std::ops::Deref for ForeignerHoldingHoldingRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ForeignerHoldingHoldingRate> for ::std::string::String {
    fn from(value: ForeignerHoldingHoldingRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ForeignerHoldingHoldingRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ForeignerHoldingHoldingRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ForeignerHoldingHoldingRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ForeignerHoldingHoldingRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ForeignerHoldingLimitQuantity(::std::string::String);
impl ::std::ops::Deref for ForeignerHoldingLimitQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ForeignerHoldingLimitQuantity> for ::std::string::String {
    fn from(value: ForeignerHoldingLimitQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ForeignerHoldingLimitQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ForeignerHoldingLimitQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ForeignerHoldingLimitQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ForeignerHoldingLimitQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct HoldingsItem {
    #[serde(rename = "averagePurchasePrice")]
    pub average_purchase_price: HoldingsItemAveragePurchasePrice,
    pub cost: Cost,
    pub currency: Currency,
    #[serde(rename = "dailyProfitLoss")]
    pub daily_profit_loss: DailyProfitLoss,
    #[serde(rename = "lastPrice")]
    pub last_price: HoldingsItemLastPrice,
    #[serde(rename = "marketCountry")]
    pub market_country: MarketCountry,
    #[serde(rename = "marketValue")]
    pub market_value: MarketValue,
    pub name: ::std::string::String,
    #[serde(rename = "profitLoss")]
    pub profit_loss: ProfitLoss,
    pub quantity: HoldingsItemQuantity,
    pub symbol: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct HoldingsItemAveragePurchasePrice(::std::string::String);
impl ::std::ops::Deref for HoldingsItemAveragePurchasePrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<HoldingsItemAveragePurchasePrice> for ::std::string::String {
    fn from(value: HoldingsItemAveragePurchasePrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for HoldingsItemAveragePurchasePrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for HoldingsItemAveragePurchasePrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for HoldingsItemAveragePurchasePrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for HoldingsItemAveragePurchasePrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct HoldingsItemLastPrice(::std::string::String);
impl ::std::ops::Deref for HoldingsItemLastPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<HoldingsItemLastPrice> for ::std::string::String {
    fn from(value: HoldingsItemLastPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for HoldingsItemLastPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for HoldingsItemLastPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for HoldingsItemLastPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for HoldingsItemLastPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct HoldingsItemQuantity(::std::string::String);
impl ::std::ops::Deref for HoldingsItemQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<HoldingsItemQuantity> for ::std::string::String {
    fn from(value: HoldingsItemQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for HoldingsItemQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for HoldingsItemQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for HoldingsItemQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for HoldingsItemQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct HoldingsOverview {
    #[serde(rename = "dailyProfitLoss")]
    pub daily_profit_loss: OverviewDailyProfitLoss,
    pub items: ::std::vec::Vec<HoldingsItem>,
    #[serde(rename = "marketValue")]
    pub market_value: OverviewMarketValue,
    #[serde(rename = "profitLoss")]
    pub profit_loss: OverviewProfitLoss,
    #[serde(rename = "totalPurchaseAmount")]
    pub total_purchase_amount: Price,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InstitutionTradingAmount {
    pub breakdown: InstitutionTradingBreakdown,
    #[serde(rename = "buyAmount")]
    pub buy_amount: InstitutionTradingAmountBuyAmount,
    #[serde(rename = "sellAmount")]
    pub sell_amount: InstitutionTradingAmountSellAmount,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InstitutionTradingAmountBuyAmount(::std::string::String);
impl ::std::ops::Deref for InstitutionTradingAmountBuyAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InstitutionTradingAmountBuyAmount> for ::std::string::String {
    fn from(value: InstitutionTradingAmountBuyAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InstitutionTradingAmountBuyAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InstitutionTradingAmountBuyAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InstitutionTradingAmountBuyAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InstitutionTradingAmountBuyAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InstitutionTradingAmountSellAmount(::std::string::String);
impl ::std::ops::Deref for InstitutionTradingAmountSellAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InstitutionTradingAmountSellAmount> for ::std::string::String {
    fn from(value: InstitutionTradingAmountSellAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InstitutionTradingAmountSellAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InstitutionTradingAmountSellAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InstitutionTradingAmountSellAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InstitutionTradingAmountSellAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InstitutionTradingBreakdown {
    pub bank: InvestorTradingAmount,
    #[serde(rename = "financialInvestment")]
    pub financial_investment: InvestorTradingAmount,
    pub insurance: InvestorTradingAmount,
    #[serde(rename = "otherFinancialInstitution")]
    pub other_financial_institution: InvestorTradingAmount,
    #[serde(rename = "pensionFund")]
    pub pension_fund: InvestorTradingAmount,
    #[serde(rename = "privateEquityFund")]
    pub private_equity_fund: InvestorTradingAmount,
    pub trust: InvestorTradingAmount,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct IntegratedHour {
    #[serde(
        rename = "afterMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub after: ::std::option::Option<AfterMarketSession>,
    #[serde(
        rename = "preMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pre: ::std::option::Option<PreMarketSession>,
    #[serde(
        rename = "regularMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub regular: ::std::option::Option<RegularMarketSession>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvestorNetTradingRanking {
    #[serde(rename = "netBuyRankings")]
    pub net_buy_rankings: ::std::vec::Vec<NetBuyRankingItem>,
    #[serde(rename = "netSellRankings")]
    pub net_sell_rankings: ::std::vec::Vec<NetSellRankingItem>,
    #[serde(
        rename = "rankedAt",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub ranked_at: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvestorTradingAmount {
    #[serde(rename = "buyAmount")]
    pub buy_amount: InvestorTradingAmountBuyAmount,
    #[serde(rename = "sellAmount")]
    pub sell_amount: InvestorTradingAmountSellAmount,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InvestorTradingAmountBuyAmount(::std::string::String);
impl ::std::ops::Deref for InvestorTradingAmountBuyAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InvestorTradingAmountBuyAmount> for ::std::string::String {
    fn from(value: InvestorTradingAmountBuyAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InvestorTradingAmountBuyAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InvestorTradingAmountBuyAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InvestorTradingAmountBuyAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InvestorTradingAmountBuyAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InvestorTradingAmountSellAmount(::std::string::String);
impl ::std::ops::Deref for InvestorTradingAmountSellAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InvestorTradingAmountSellAmount> for ::std::string::String {
    fn from(value: InvestorTradingAmountSellAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InvestorTradingAmountSellAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InvestorTradingAmountSellAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InvestorTradingAmountSellAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InvestorTradingAmountSellAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvestorTradingRankingResponse {
    pub foreigner: InvestorNetTradingRanking,
    pub individual: InvestorNetTradingRanking,
    pub institution: InvestorNetTradingRanking,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvestorTradingRecord {
    pub date: ::std::string::String,
    pub foreigner: InvestorTradingAmount,
    pub individual: InvestorTradingAmount,
    pub institution: InstitutionTradingAmount,
    #[serde(rename = "otherCorporation")]
    pub other_corporation: InvestorTradingAmount,
    #[serde(rename = "updatedAt")]
    pub updated_at: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvestorTradingResponse {
    #[serde(
        rename = "nextUntil",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_until: ::std::option::Option<::std::string::String>,
    pub records: ::std::vec::Vec<InvestorTradingRecord>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvestorTradingVolume {
    #[serde(rename = "buyVolume")]
    pub buy: InvestorTradingVolumeBuyVolume,
    #[serde(rename = "netBuyVolume")]
    pub net_buy: InvestorTradingVolumeNetBuyVolume,
    #[serde(rename = "sellVolume")]
    pub sell: InvestorTradingVolumeSellVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InvestorTradingVolumeBuyVolume(::std::string::String);
impl ::std::ops::Deref for InvestorTradingVolumeBuyVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InvestorTradingVolumeBuyVolume> for ::std::string::String {
    fn from(value: InvestorTradingVolumeBuyVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InvestorTradingVolumeBuyVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InvestorTradingVolumeBuyVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InvestorTradingVolumeBuyVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InvestorTradingVolumeBuyVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InvestorTradingVolumeNetBuyVolume(::std::string::String);
impl ::std::ops::Deref for InvestorTradingVolumeNetBuyVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InvestorTradingVolumeNetBuyVolume> for ::std::string::String {
    fn from(value: InvestorTradingVolumeNetBuyVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InvestorTradingVolumeNetBuyVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InvestorTradingVolumeNetBuyVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InvestorTradingVolumeNetBuyVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InvestorTradingVolumeNetBuyVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InvestorTradingVolumeSellVolume(::std::string::String);
impl ::std::ops::Deref for InvestorTradingVolumeSellVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InvestorTradingVolumeSellVolume> for ::std::string::String {
    fn from(value: InvestorTradingVolumeSellVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InvestorTradingVolumeSellVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for InvestorTradingVolumeSellVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InvestorTradingVolumeSellVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InvestorTradingVolumeSellVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct KrMarketCalendarResponse {
    #[serde(rename = "nextBusinessDay")]
    pub next_business_day: KrMarketDay,
    #[serde(rename = "previousBusinessDay")]
    pub previous_business_day: KrMarketDay,
    pub today: KrMarketDay,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct KrMarketDay {
    pub date: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub integrated: ::std::option::Option<IntegratedHour>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct KrMarketDetail {
    #[serde(rename = "krxTradingSuspended")]
    pub krx_trading_suspended: bool,
    #[serde(rename = "liquidationTrading")]
    pub liquidation_trading: bool,
    #[serde(rename = "nxtSupported")]
    pub nxt_supported: bool,
    #[serde(
        rename = "nxtTradingSuspended",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub nxt_trading_suspended: ::std::option::Option<bool>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListedStock {
    #[serde(rename = "isCommonShare")]
    pub is_common_share: bool,
    #[serde(rename = "isinCode")]
    pub isin_code: ::std::string::String,
    pub name: ::std::string::String,
    #[serde(rename = "securityType")]
    pub security_type: ListedStockSecurityType,
    pub symbol: ::std::string::String,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum ListedStockSecurityType {
    #[serde(rename = "STOCK")]
    Stock,
    #[serde(rename = "FOREIGN_STOCK")]
    ForeignStock,
    #[serde(rename = "DEPOSITARY_RECEIPT")]
    DepositaryReceipt,
    #[serde(rename = "INFRASTRUCTURE_FUND")]
    InfrastructureFund,
    #[serde(rename = "REIT")]
    Reit,
    #[serde(rename = "ETF")]
    Etf,
    #[serde(rename = "FOREIGN_ETF")]
    ForeignEtf,
    #[serde(rename = "ETN")]
    Etn,
    #[serde(rename = "STOCK_WARRANTS")]
    StockWarrants,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum MarketCountry {
    #[serde(rename = "KR")]
    Kr,
    #[serde(rename = "US")]
    Us,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MarketIndicatorCandle {
    #[serde(rename = "closePrice")]
    pub close_price: MarketIndicatorCandleClosePrice,
    #[serde(rename = "highPrice")]
    pub high_price: MarketIndicatorCandleHighPrice,
    #[serde(rename = "lowPrice")]
    pub low_price: MarketIndicatorCandleLowPrice,
    #[serde(rename = "openPrice")]
    pub open_price: MarketIndicatorCandleOpenPrice,
    pub timestamp: ::std::string::String,
    pub volume: MarketIndicatorCandleVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketIndicatorCandleClosePrice(::std::string::String);
impl ::std::ops::Deref for MarketIndicatorCandleClosePrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketIndicatorCandleClosePrice> for ::std::string::String {
    fn from(value: MarketIndicatorCandleClosePrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketIndicatorCandleClosePrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketIndicatorCandleClosePrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketIndicatorCandleClosePrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketIndicatorCandleClosePrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketIndicatorCandleHighPrice(::std::string::String);
impl ::std::ops::Deref for MarketIndicatorCandleHighPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketIndicatorCandleHighPrice> for ::std::string::String {
    fn from(value: MarketIndicatorCandleHighPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketIndicatorCandleHighPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketIndicatorCandleHighPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketIndicatorCandleHighPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketIndicatorCandleHighPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketIndicatorCandleLowPrice(::std::string::String);
impl ::std::ops::Deref for MarketIndicatorCandleLowPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketIndicatorCandleLowPrice> for ::std::string::String {
    fn from(value: MarketIndicatorCandleLowPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketIndicatorCandleLowPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketIndicatorCandleLowPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketIndicatorCandleLowPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketIndicatorCandleLowPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketIndicatorCandleOpenPrice(::std::string::String);
impl ::std::ops::Deref for MarketIndicatorCandleOpenPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketIndicatorCandleOpenPrice> for ::std::string::String {
    fn from(value: MarketIndicatorCandleOpenPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketIndicatorCandleOpenPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketIndicatorCandleOpenPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketIndicatorCandleOpenPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketIndicatorCandleOpenPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MarketIndicatorCandlePageResponse {
    pub candles: ::std::vec::Vec<MarketIndicatorCandle>,
    #[serde(
        rename = "nextBefore",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_before: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketIndicatorCandleVolume(::std::string::String);
impl ::std::ops::Deref for MarketIndicatorCandleVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketIndicatorCandleVolume> for ::std::string::String {
    fn from(value: MarketIndicatorCandleVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketIndicatorCandleVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketIndicatorCandleVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketIndicatorCandleVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketIndicatorCandleVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MarketIndicatorPriceResponse {
    #[serde(rename = "lastPrice")]
    pub last_price: MarketIndicatorPriceResponseLastPrice,
    pub symbol: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub timestamp: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketIndicatorPriceResponseLastPrice(::std::string::String);
impl ::std::ops::Deref for MarketIndicatorPriceResponseLastPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketIndicatorPriceResponseLastPrice> for ::std::string::String {
    fn from(value: MarketIndicatorPriceResponseLastPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketIndicatorPriceResponseLastPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketIndicatorPriceResponseLastPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketIndicatorPriceResponseLastPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketIndicatorPriceResponseLastPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MarketValue {
    pub amount: MarketValueAmount,
    #[serde(rename = "amountAfterCost")]
    pub amount_after_cost: MarketValueAmountAfterCost,
    #[serde(rename = "purchaseAmount")]
    pub purchase_amount: MarketValuePurchaseAmount,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketValueAmount(::std::string::String);
impl ::std::ops::Deref for MarketValueAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketValueAmount> for ::std::string::String {
    fn from(value: MarketValueAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketValueAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketValueAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketValueAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketValueAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketValueAmountAfterCost(::std::string::String);
impl ::std::ops::Deref for MarketValueAmountAfterCost {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketValueAmountAfterCost> for ::std::string::String {
    fn from(value: MarketValueAmountAfterCost) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketValueAmountAfterCost {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketValueAmountAfterCost {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketValueAmountAfterCost {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketValueAmountAfterCost {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MarketValuePurchaseAmount(::std::string::String);
impl ::std::ops::Deref for MarketValuePurchaseAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MarketValuePurchaseAmount> for ::std::string::String {
    fn from(value: MarketValuePurchaseAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MarketValuePurchaseAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for MarketValuePurchaseAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketValuePurchaseAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MarketValuePurchaseAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct NetBuyRankingItem {
    pub currency: Currency,
    #[serde(rename = "netBuyAmount")]
    pub net_buy_amount: NetBuyRankingItemNetBuyAmount,
    pub rank: i64,
    pub symbol: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct NetBuyRankingItemNetBuyAmount(::std::string::String);
impl ::std::ops::Deref for NetBuyRankingItemNetBuyAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<NetBuyRankingItemNetBuyAmount> for ::std::string::String {
    fn from(value: NetBuyRankingItemNetBuyAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for NetBuyRankingItemNetBuyAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for NetBuyRankingItemNetBuyAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NetBuyRankingItemNetBuyAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for NetBuyRankingItemNetBuyAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct NetSellRankingItem {
    pub currency: Currency,
    #[serde(rename = "netSellAmount")]
    pub net_sell_amount: NetSellRankingItemNetSellAmount,
    pub rank: i64,
    pub symbol: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct NetSellRankingItemNetSellAmount(::std::string::String);
impl ::std::ops::Deref for NetSellRankingItemNetSellAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<NetSellRankingItemNetSellAmount> for ::std::string::String {
    fn from(value: NetSellRankingItemNetSellAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for NetSellRankingItemNetSellAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for NetSellRankingItemNetSellAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NetSellRankingItemNetSellAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for NetSellRankingItemNetSellAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OAuth2ErrorResponse {
    pub error: OAuth2ErrorResponseError,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub error_description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub error_uri: ::std::option::Option<::std::string::String>,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OAuth2ErrorResponseError {
    #[serde(rename = "invalid_request")]
    InvalidRequest,
    #[serde(rename = "invalid_client")]
    InvalidClient,
    #[serde(rename = "invalid_grant")]
    InvalidGrant,
    #[serde(rename = "unauthorized_client")]
    UnauthorizedClient,
    #[serde(rename = "unsupported_grant_type")]
    UnsupportedGrantType,
    #[serde(rename = "access_denied")]
    AccessDenied,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OAuth2TokenRequest {
    pub client_id: ::std::string::String,
    pub client_secret: ::std::string::String,
    pub grant_type: OAuth2TokenRequestGrantType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OAuth2TokenRequestGrantType {
    #[serde(rename = "client_credentials")]
    ClientCredentials,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OAuth2TokenResponse {
    pub access_token: ::std::string::String,
    pub expires_in: i64,
    pub token_type: OAuth2TokenResponseTokenType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OAuth2TokenResponseTokenType {
    Bearer,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Order {
    #[serde(
        rename = "canceledAt",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub canceled_at: ::std::option::Option<::std::string::String>,
    pub currency: Currency,
    pub execution: OrderExecution,
    #[serde(
        rename = "orderAmount",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub amount: ::std::option::Option<OrderOrderAmount>,
    #[serde(rename = "orderId")]
    pub id: ::std::string::String,
    #[serde(rename = "orderType")]
    pub kind: OrderOrderType,
    #[serde(rename = "orderedAt")]
    pub ordered_at: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub price: ::std::option::Option<OrderPrice>,
    pub quantity: OrderQuantity,
    pub side: OrderSide,
    pub status: OrderStatus,
    pub symbol: ::std::string::String,
    #[serde(rename = "timeInForce")]
    pub time_in_force: OrderTimeInForce,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderCreateAmountBasedClientOrderId(::std::string::String);
impl ::std::ops::Deref for OrderCreateAmountBasedClientOrderId {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderCreateAmountBasedClientOrderId> for ::std::string::String {
    fn from(value: OrderCreateAmountBasedClientOrderId) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderCreateAmountBasedClientOrderId {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 36_usize {
            return Err("longer than 36 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^[a-zA-Z0-9\\-_]+$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-zA-Z0-9\\-_]+$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateAmountBasedClientOrderId {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateAmountBasedClientOrderId {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderCreateAmountBasedClientOrderId {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderCreateAmountBasedOrderAmount(::std::string::String);
impl ::std::ops::Deref for OrderCreateAmountBasedOrderAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderCreateAmountBasedOrderAmount> for ::std::string::String {
    fn from(value: OrderCreateAmountBasedOrderAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderCreateAmountBasedOrderAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateAmountBasedOrderAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateAmountBasedOrderAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderCreateAmountBasedOrderAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderCreateAmountBasedOrderType {
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderCreateAmountBasedSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderCreateQuantityBasedClientOrderId(::std::string::String);
impl ::std::ops::Deref for OrderCreateQuantityBasedClientOrderId {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderCreateQuantityBasedClientOrderId> for ::std::string::String {
    fn from(value: OrderCreateQuantityBasedClientOrderId) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedClientOrderId {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 36_usize {
            return Err("longer than 36 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^[a-zA-Z0-9\\-_]+$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-zA-Z0-9\\-_]+$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateQuantityBasedClientOrderId {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateQuantityBasedClientOrderId {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderCreateQuantityBasedClientOrderId {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderCreateQuantityBasedOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderCreateQuantityBasedPrice(::std::string::String);
impl ::std::ops::Deref for OrderCreateQuantityBasedPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderCreateQuantityBasedPrice> for ::std::string::String {
    fn from(value: OrderCreateQuantityBasedPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateQuantityBasedPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateQuantityBasedPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderCreateQuantityBasedPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderCreateQuantityBasedQuantity(::std::string::String);
impl ::std::ops::Deref for OrderCreateQuantityBasedQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderCreateQuantityBasedQuantity> for ::std::string::String {
    fn from(value: OrderCreateQuantityBasedQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateQuantityBasedQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateQuantityBasedQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderCreateQuantityBasedQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderCreateQuantityBasedSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
#[derive(Default)]
pub enum OrderCreateQuantityBasedTimeInForce {
    #[serde(rename = "DAY")]
    #[default]
    Day,
    #[serde(rename = "CLS")]
    Cls,
    #[serde(rename = "OPG")]
    Opg,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
pub enum OrderCreateRequest {
    QuantityBased {
        #[serde(
            rename = "clientOrderId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        client_order_id: ::std::option::Option<OrderCreateQuantityBasedClientOrderId>,
        #[serde(rename = "confirmHighValueOrder", default)]
        confirm_high_value_order: bool,
        #[serde(rename = "orderType")]
        order_type: OrderCreateQuantityBasedOrderType,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        price: ::std::option::Option<OrderCreateQuantityBasedPrice>,
        quantity: OrderCreateQuantityBasedQuantity,
        side: OrderCreateQuantityBasedSide,
        symbol: ::std::string::String,
        #[serde(
            rename = "timeInForce",
            default = "defaults::order_create_request_quantity_based_time_in_force"
        )]
        time_in_force: OrderCreateQuantityBasedTimeInForce,
    },
    AmountBased {
        #[serde(
            rename = "clientOrderId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        client_order_id: ::std::option::Option<OrderCreateAmountBasedClientOrderId>,
        #[serde(rename = "confirmHighValueOrder", default)]
        confirm_high_value_order: bool,
        #[serde(rename = "orderAmount")]
        order_amount: OrderCreateAmountBasedOrderAmount,
        #[serde(rename = "orderType")]
        order_type: OrderCreateAmountBasedOrderType,
        side: OrderCreateAmountBasedSide,
        symbol: ::std::string::String,
    },
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OrderExecution {
    #[serde(
        rename = "averageFilledPrice",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub average_filled_price: ::std::option::Option<OrderExecutionAverageFilledPrice>,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub commission: ::std::option::Option<OrderExecutionCommission>,
    #[serde(
        rename = "filledAmount",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub filled_amount: ::std::option::Option<OrderExecutionFilledAmount>,
    #[serde(
        rename = "filledAt",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub filled_at: ::std::option::Option<::std::string::String>,
    #[serde(rename = "filledQuantity")]
    pub filled_quantity: OrderExecutionFilledQuantity,
    #[serde(
        rename = "settlementDate",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub settlement_date: ::std::option::Option<::std::string::String>,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub tax: ::std::option::Option<OrderExecutionTax>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderExecutionAverageFilledPrice(::std::string::String);
impl ::std::ops::Deref for OrderExecutionAverageFilledPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderExecutionAverageFilledPrice> for ::std::string::String {
    fn from(value: OrderExecutionAverageFilledPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderExecutionAverageFilledPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderExecutionAverageFilledPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderExecutionAverageFilledPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderExecutionAverageFilledPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderExecutionCommission(::std::string::String);
impl ::std::ops::Deref for OrderExecutionCommission {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderExecutionCommission> for ::std::string::String {
    fn from(value: OrderExecutionCommission) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderExecutionCommission {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderExecutionCommission {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderExecutionCommission {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderExecutionCommission {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderExecutionFilledAmount(::std::string::String);
impl ::std::ops::Deref for OrderExecutionFilledAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderExecutionFilledAmount> for ::std::string::String {
    fn from(value: OrderExecutionFilledAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderExecutionFilledAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderExecutionFilledAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderExecutionFilledAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderExecutionFilledAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderExecutionFilledQuantity(::std::string::String);
impl ::std::ops::Deref for OrderExecutionFilledQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderExecutionFilledQuantity> for ::std::string::String {
    fn from(value: OrderExecutionFilledQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderExecutionFilledQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderExecutionFilledQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderExecutionFilledQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderExecutionFilledQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderExecutionTax(::std::string::String);
impl ::std::ops::Deref for OrderExecutionTax {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderExecutionTax> for ::std::string::String {
    fn from(value: OrderExecutionTax) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderExecutionTax {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderExecutionTax {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderExecutionTax {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderExecutionTax {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OrderModifyRequest {
    #[serde(rename = "confirmHighValueOrder", default)]
    pub confirm_high_value_order: bool,
    #[serde(rename = "orderType")]
    pub order_type: OrderModifyRequestOrderType,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub price: ::std::option::Option<OrderModifyRequestPrice>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quantity: ::std::option::Option<OrderModifyRequestQuantity>,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderModifyRequestOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderModifyRequestPrice(::std::string::String);
impl ::std::ops::Deref for OrderModifyRequestPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderModifyRequestPrice> for ::std::string::String {
    fn from(value: OrderModifyRequestPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderModifyRequestPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern =
            ::regress::Regex::new("^\\d+(\\.\\d+)?$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+(\\.\\d+)?$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderModifyRequestPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderModifyRequestPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderModifyRequestPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderModifyRequestQuantity(::std::string::String);
impl ::std::ops::Deref for OrderModifyRequestQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderModifyRequestQuantity> for ::std::string::String {
    fn from(value: OrderModifyRequestQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderModifyRequestQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        let pattern = ::regress::Regex::new("^\\d+$").map_err(|error| error.to_string())?;
        if pattern.find(value).is_none() {
            return Err("doesn't match pattern \"^\\d+$\"".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderModifyRequestQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderModifyRequestQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderModifyRequestQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OrderOperationResponse {
    #[serde(rename = "orderId")]
    pub order_id: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderOrderAmount(::std::string::String);
impl ::std::ops::Deref for OrderOrderAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderOrderAmount> for ::std::string::String {
    fn from(value: OrderOrderAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderOrderAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderOrderAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderOrderAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderOrderAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderPrice(::std::string::String);
impl ::std::ops::Deref for OrderPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderPrice> for ::std::string::String {
    fn from(value: OrderPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderQuantity(::std::string::String);
impl ::std::ops::Deref for OrderQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderQuantity> for ::std::string::String {
    fn from(value: OrderQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OrderResponse {
    #[serde(
        rename = "clientOrderId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub client_order_id: ::std::option::Option<::std::string::String>,
    #[serde(rename = "orderId")]
    pub order_id: ::std::string::String,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderStatus {
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "PENDING_CANCEL")]
    PendingCancel,
    #[serde(rename = "PENDING_REPLACE")]
    PendingReplace,
    #[serde(rename = "PARTIAL_FILLED")]
    PartialFilled,
    #[serde(rename = "FILLED")]
    Filled,
    #[serde(rename = "CANCELED")]
    Canceled,
    #[serde(rename = "REJECTED")]
    Rejected,
    #[serde(rename = "CANCEL_REJECTED")]
    CancelRejected,
    #[serde(rename = "REPLACE_REJECTED")]
    ReplaceRejected,
    #[serde(rename = "REPLACED")]
    Replaced,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum OrderTimeInForce {
    #[serde(rename = "DAY")]
    Day,
    #[serde(rename = "CLS")]
    Cls,
    #[serde(rename = "OPG")]
    Opg,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OrderbookEntry {
    pub price: OrderbookEntryPrice,
    pub volume: OrderbookEntryVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderbookEntryPrice(::std::string::String);
impl ::std::ops::Deref for OrderbookEntryPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderbookEntryPrice> for ::std::string::String {
    fn from(value: OrderbookEntryPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderbookEntryPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderbookEntryPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderbookEntryPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderbookEntryPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OrderbookEntryVolume(::std::string::String);
impl ::std::ops::Deref for OrderbookEntryVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OrderbookEntryVolume> for ::std::string::String {
    fn from(value: OrderbookEntryVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OrderbookEntryVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OrderbookEntryVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderbookEntryVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OrderbookEntryVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OrderbookResponse {
    pub asks: ::std::vec::Vec<OrderbookEntry>,
    pub bids: ::std::vec::Vec<OrderbookEntry>,
    pub currency: Currency,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub timestamp: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OverviewDailyProfitLoss {
    pub amount: Price,
    pub rate: OverviewDailyProfitLossRate,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OverviewDailyProfitLossRate(::std::string::String);
impl ::std::ops::Deref for OverviewDailyProfitLossRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OverviewDailyProfitLossRate> for ::std::string::String {
    fn from(value: OverviewDailyProfitLossRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OverviewDailyProfitLossRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OverviewDailyProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OverviewDailyProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OverviewDailyProfitLossRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OverviewMarketValue {
    pub amount: Price,
    #[serde(rename = "amountAfterCost")]
    pub amount_after_cost: Price,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OverviewProfitLoss {
    pub amount: Price,
    #[serde(rename = "amountAfterCost")]
    pub amount_after_cost: Price,
    pub rate: OverviewProfitLossRate,
    #[serde(rename = "rateAfterCost")]
    pub rate_after_cost: OverviewProfitLossRateAfterCost,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OverviewProfitLossRate(::std::string::String);
impl ::std::ops::Deref for OverviewProfitLossRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OverviewProfitLossRate> for ::std::string::String {
    fn from(value: OverviewProfitLossRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OverviewProfitLossRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OverviewProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OverviewProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OverviewProfitLossRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct OverviewProfitLossRateAfterCost(::std::string::String);
impl ::std::ops::Deref for OverviewProfitLossRateAfterCost {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OverviewProfitLossRateAfterCost> for ::std::string::String {
    fn from(value: OverviewProfitLossRateAfterCost) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for OverviewProfitLossRateAfterCost {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for OverviewProfitLossRateAfterCost {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OverviewProfitLossRateAfterCost {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for OverviewProfitLossRateAfterCost {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PaginatedConditionalOrderResponse {
    #[serde(rename = "conditionalOrders")]
    pub conditional_orders: ::std::vec::Vec<ConditionalOrderDetailResponse>,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
    #[serde(
        rename = "nextCursor",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_cursor: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PaginatedOrderResponse {
    #[serde(rename = "hasNext")]
    pub has_next: bool,
    #[serde(
        rename = "nextCursor",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub next_cursor: ::std::option::Option<::std::string::String>,
    pub orders: ::std::vec::Vec<Order>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PaginatedTransactionResponse {
    #[serde(rename = "hasNext")]
    pub has_next: bool,
    #[serde(
        rename = "nextCursor",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub next_cursor: ::std::option::Option<::std::string::String>,
    pub transactions: ::std::vec::Vec<Transaction>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PreMarketSession {
    #[serde(rename = "endTime")]
    pub end: ::std::string::String,
    #[serde(
        rename = "singlePriceAuctionStartTime",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub single_price_auction_start: ::std::option::Option<::std::string::String>,
    #[serde(rename = "startTime")]
    pub start: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Price {
    pub krw: PriceKrw,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub usd: ::std::option::Option<PriceUsd>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PriceKrw(::std::string::String);
impl ::std::ops::Deref for PriceKrw {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PriceKrw> for ::std::string::String {
    fn from(value: PriceKrw) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PriceKrw {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for PriceKrw {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PriceKrw {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PriceKrw {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PriceLimitResponse {
    pub currency: Currency,
    #[serde(
        rename = "lowerLimitPrice",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub lower_limit_price: ::std::option::Option<PriceLimitResponseLowerLimitPrice>,
    pub timestamp: ::std::string::String,
    #[serde(
        rename = "upperLimitPrice",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub upper_limit_price: ::std::option::Option<PriceLimitResponseUpperLimitPrice>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PriceLimitResponseLowerLimitPrice(::std::string::String);
impl ::std::ops::Deref for PriceLimitResponseLowerLimitPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PriceLimitResponseLowerLimitPrice> for ::std::string::String {
    fn from(value: PriceLimitResponseLowerLimitPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PriceLimitResponseLowerLimitPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for PriceLimitResponseLowerLimitPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PriceLimitResponseLowerLimitPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PriceLimitResponseLowerLimitPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PriceLimitResponseUpperLimitPrice(::std::string::String);
impl ::std::ops::Deref for PriceLimitResponseUpperLimitPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PriceLimitResponseUpperLimitPrice> for ::std::string::String {
    fn from(value: PriceLimitResponseUpperLimitPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PriceLimitResponseUpperLimitPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for PriceLimitResponseUpperLimitPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PriceLimitResponseUpperLimitPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PriceLimitResponseUpperLimitPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PriceResponse {
    pub currency: Currency,
    #[serde(rename = "lastPrice")]
    pub last_price: PriceResponseLastPrice,
    pub symbol: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub timestamp: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PriceResponseLastPrice(::std::string::String);
impl ::std::ops::Deref for PriceResponseLastPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PriceResponseLastPrice> for ::std::string::String {
    fn from(value: PriceResponseLastPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PriceResponseLastPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for PriceResponseLastPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PriceResponseLastPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PriceResponseLastPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PriceUsd(::std::string::String);
impl ::std::ops::Deref for PriceUsd {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PriceUsd> for ::std::string::String {
    fn from(value: PriceUsd) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PriceUsd {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for PriceUsd {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PriceUsd {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PriceUsd {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ProfitLoss {
    pub amount: ProfitLossAmount,
    #[serde(rename = "amountAfterCost")]
    pub amount_after_cost: ProfitLossAmountAfterCost,
    pub rate: ProfitLossRate,
    #[serde(rename = "rateAfterCost")]
    pub rate_after_cost: ProfitLossRateAfterCost,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProfitLossAmount(::std::string::String);
impl ::std::ops::Deref for ProfitLossAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProfitLossAmount> for ::std::string::String {
    fn from(value: ProfitLossAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProfitLossAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProfitLossAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProfitLossAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProfitLossAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProfitLossAmountAfterCost(::std::string::String);
impl ::std::ops::Deref for ProfitLossAmountAfterCost {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProfitLossAmountAfterCost> for ::std::string::String {
    fn from(value: ProfitLossAmountAfterCost) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProfitLossAmountAfterCost {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProfitLossAmountAfterCost {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProfitLossAmountAfterCost {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProfitLossAmountAfterCost {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProfitLossRate(::std::string::String);
impl ::std::ops::Deref for ProfitLossRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProfitLossRate> for ::std::string::String {
    fn from(value: ProfitLossRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProfitLossRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProfitLossRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProfitLossRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProfitLossRateAfterCost(::std::string::String);
impl ::std::ops::Deref for ProfitLossRateAfterCost {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProfitLossRateAfterCost> for ::std::string::String {
    fn from(value: ProfitLossRateAfterCost) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProfitLossRateAfterCost {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProfitLossRateAfterCost {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProfitLossRateAfterCost {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProfitLossRateAfterCost {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ProgramTradeRecord {
    pub arbitrage: ProgramTradingVolume,
    pub date: ::std::string::String,
    #[serde(rename = "nonArbitrage")]
    pub non_arbitrage: ProgramTradingVolume,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ProgramTradesResponse {
    #[serde(
        rename = "nextUntil",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_until: ::std::option::Option<::std::string::String>,
    pub records: ::std::vec::Vec<ProgramTradeRecord>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ProgramTradingVolume {
    #[serde(rename = "buyVolume")]
    pub buy: ProgramTradingVolumeBuyVolume,
    #[serde(rename = "netBuyVolume")]
    pub net_buy: ProgramTradingVolumeNetBuyVolume,
    #[serde(rename = "sellVolume")]
    pub sell: ProgramTradingVolumeSellVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProgramTradingVolumeBuyVolume(::std::string::String);
impl ::std::ops::Deref for ProgramTradingVolumeBuyVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProgramTradingVolumeBuyVolume> for ::std::string::String {
    fn from(value: ProgramTradingVolumeBuyVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProgramTradingVolumeBuyVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProgramTradingVolumeBuyVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProgramTradingVolumeBuyVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProgramTradingVolumeBuyVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProgramTradingVolumeNetBuyVolume(::std::string::String);
impl ::std::ops::Deref for ProgramTradingVolumeNetBuyVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProgramTradingVolumeNetBuyVolume> for ::std::string::String {
    fn from(value: ProgramTradingVolumeNetBuyVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProgramTradingVolumeNetBuyVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProgramTradingVolumeNetBuyVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProgramTradingVolumeNetBuyVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProgramTradingVolumeNetBuyVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ProgramTradingVolumeSellVolume(::std::string::String);
impl ::std::ops::Deref for ProgramTradingVolumeSellVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ProgramTradingVolumeSellVolume> for ::std::string::String {
    fn from(value: ProgramTradingVolumeSellVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ProgramTradingVolumeSellVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ProgramTradingVolumeSellVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProgramTradingVolumeSellVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ProgramTradingVolumeSellVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RankingItem {
    pub currency: Currency,
    pub price: RankingPrice,
    pub rank: i64,
    pub symbol: ::std::string::String,
    #[serde(rename = "tradingAmount")]
    pub trading_amount: RankingItemTradingAmount,
    #[serde(rename = "tradingVolume")]
    pub trading_volume: RankingItemTradingVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct RankingItemTradingAmount(::std::string::String);
impl ::std::ops::Deref for RankingItemTradingAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<RankingItemTradingAmount> for ::std::string::String {
    fn from(value: RankingItemTradingAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for RankingItemTradingAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for RankingItemTradingAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RankingItemTradingAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for RankingItemTradingAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct RankingItemTradingVolume(::std::string::String);
impl ::std::ops::Deref for RankingItemTradingVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<RankingItemTradingVolume> for ::std::string::String {
    fn from(value: RankingItemTradingVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for RankingItemTradingVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for RankingItemTradingVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RankingItemTradingVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for RankingItemTradingVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RankingPrice {
    #[serde(rename = "basePrice")]
    pub base_price: RankingPriceBasePrice,
    #[serde(
        rename = "changeRate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub change_rate: ::std::option::Option<RankingPriceChangeRate>,
    #[serde(rename = "lastPrice")]
    pub last_price: RankingPriceLastPrice,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct RankingPriceBasePrice(::std::string::String);
impl ::std::ops::Deref for RankingPriceBasePrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<RankingPriceBasePrice> for ::std::string::String {
    fn from(value: RankingPriceBasePrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for RankingPriceBasePrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for RankingPriceBasePrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RankingPriceBasePrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for RankingPriceBasePrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct RankingPriceChangeRate(::std::string::String);
impl ::std::ops::Deref for RankingPriceChangeRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<RankingPriceChangeRate> for ::std::string::String {
    fn from(value: RankingPriceChangeRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for RankingPriceChangeRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for RankingPriceChangeRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RankingPriceChangeRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for RankingPriceChangeRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct RankingPriceLastPrice(::std::string::String);
impl ::std::ops::Deref for RankingPriceLastPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<RankingPriceLastPrice> for ::std::string::String {
    fn from(value: RankingPriceLastPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for RankingPriceLastPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for RankingPriceLastPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RankingPriceLastPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for RankingPriceLastPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RankingResponse {
    #[serde(
        rename = "rankedAt",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ranked_at: ::std::option::Option<::std::string::String>,
    pub rankings: ::std::vec::Vec<RankingItem>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RegularMarketSession {
    #[serde(rename = "endTime")]
    pub end: ::std::string::String,
    #[serde(
        rename = "singlePriceAuctionStartTime",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub single_price_auction_start: ::std::option::Option<::std::string::String>,
    #[serde(rename = "startTime")]
    pub start: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Sector {
    pub depth: i64,
    pub name: ::std::string::String,
    #[serde(
        rename = "parentSectorId",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub parent_sector_id: ::std::option::Option<::std::string::String>,
    #[serde(rename = "sectorId")]
    pub id: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorChangeRate {
    #[serde(
        rename = "changeRate",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub change_rate: ::std::option::Option<SectorChangeRateChangeRate>,
    pub duration: SectorChangeRateDuration,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorChangeRateChangeRate(::std::string::String);
impl ::std::ops::Deref for SectorChangeRateChangeRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorChangeRateChangeRate> for ::std::string::String {
    fn from(value: SectorChangeRateChangeRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorChangeRateChangeRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorChangeRateChangeRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorChangeRateChangeRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorChangeRateChangeRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum SectorChangeRateDuration {
    #[serde(rename = "1d")]
    X1d,
    #[serde(rename = "1w")]
    X1w,
    #[serde(rename = "1mo")]
    X1mo,
    #[serde(rename = "3mo")]
    X3mo,
    #[serde(rename = "6mo")]
    X6mo,
    #[serde(rename = "1y")]
    X1y,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorDetailResponse {
    pub depth: i64,
    pub name: ::std::string::String,
    #[serde(
        rename = "parentSectorId",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub parent_sector_id: ::std::option::Option<::std::string::String>,
    pub performance: SectorPerformance,
    #[serde(rename = "sectorId")]
    pub sector_id: ::std::string::String,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub summary: ::std::option::Option<::std::string::String>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorEtf {
    pub name: ::std::string::String,
    pub symbol: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorEtfsResponse {
    pub etfs: ::std::vec::Vec<SectorEtf>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorPerformance {
    #[serde(rename = "changeRates")]
    pub change_rates: ::std::vec::Vec<SectorChangeRate>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorRankingItem {
    #[serde(
        rename = "changeRate",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub change_rate: ::std::option::Option<SectorRankingItemChangeRate>,
    pub currency: Currency,
    #[serde(
        rename = "marketCapitalization",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub market_capitalization: ::std::option::Option<SectorRankingItemMarketCapitalization>,
    pub name: ::std::string::String,
    pub rank: i64,
    #[serde(rename = "sectorId")]
    pub sector_id: ::std::string::String,
    #[serde(
        rename = "tradingAmount",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub trading_amount: ::std::option::Option<SectorRankingItemTradingAmount>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorRankingItemChangeRate(::std::string::String);
impl ::std::ops::Deref for SectorRankingItemChangeRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorRankingItemChangeRate> for ::std::string::String {
    fn from(value: SectorRankingItemChangeRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorRankingItemChangeRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorRankingItemChangeRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorRankingItemChangeRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorRankingItemChangeRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorRankingItemMarketCapitalization(::std::string::String);
impl ::std::ops::Deref for SectorRankingItemMarketCapitalization {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorRankingItemMarketCapitalization> for ::std::string::String {
    fn from(value: SectorRankingItemMarketCapitalization) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorRankingItemMarketCapitalization {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorRankingItemMarketCapitalization {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorRankingItemMarketCapitalization {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorRankingItemMarketCapitalization {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorRankingItemTradingAmount(::std::string::String);
impl ::std::ops::Deref for SectorRankingItemTradingAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorRankingItemTradingAmount> for ::std::string::String {
    fn from(value: SectorRankingItemTradingAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorRankingItemTradingAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorRankingItemTradingAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorRankingItemTradingAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorRankingItemTradingAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorRankingResponse {
    pub rankings: ::std::vec::Vec<SectorRankingItem>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorStock {
    pub currency: Currency,
    #[serde(
        rename = "marketCapitalization",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub market_capitalization: ::std::option::Option<SectorStockMarketCapitalization>,
    pub name: ::std::string::String,
    pub symbol: ::std::string::String,
    #[serde(
        rename = "tradingAmount",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub trading_amount: ::std::option::Option<SectorStockTradingAmount>,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub volume: ::std::option::Option<SectorStockVolume>,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorStockMarketCapitalization(::std::string::String);
impl ::std::ops::Deref for SectorStockMarketCapitalization {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorStockMarketCapitalization> for ::std::string::String {
    fn from(value: SectorStockMarketCapitalization) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorStockMarketCapitalization {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorStockMarketCapitalization {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorStockMarketCapitalization {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorStockMarketCapitalization {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorStockTradingAmount(::std::string::String);
impl ::std::ops::Deref for SectorStockTradingAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorStockTradingAmount> for ::std::string::String {
    fn from(value: SectorStockTradingAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorStockTradingAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorStockTradingAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorStockTradingAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorStockTradingAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SectorStockVolume(::std::string::String);
impl ::std::ops::Deref for SectorStockVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SectorStockVolume> for ::std::string::String {
    fn from(value: SectorStockVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SectorStockVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SectorStockVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorStockVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SectorStockVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorStocksResponse {
    pub stocks: ::std::vec::Vec<SectorStock>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SectorsResponse {
    pub sectors: ::std::vec::Vec<Sector>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SecuritiesLendingRecord {
    #[serde(rename = "balanceAmount")]
    pub balance_amount: SecuritiesLendingRecordBalanceAmount,
    #[serde(rename = "balanceQuantity")]
    pub balance_quantity: SecuritiesLendingRecordBalanceQuantity,
    pub date: ::std::string::String,
    #[serde(rename = "executionQuantity")]
    pub execution_quantity: SecuritiesLendingRecordExecutionQuantity,
    #[serde(rename = "repaymentQuantity")]
    pub repayment_quantity: SecuritiesLendingRecordRepaymentQuantity,
    #[serde(rename = "updatedAt")]
    pub updated_at: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SecuritiesLendingRecordBalanceAmount(::std::string::String);
impl ::std::ops::Deref for SecuritiesLendingRecordBalanceAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SecuritiesLendingRecordBalanceAmount> for ::std::string::String {
    fn from(value: SecuritiesLendingRecordBalanceAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SecuritiesLendingRecordBalanceAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SecuritiesLendingRecordBalanceAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SecuritiesLendingRecordBalanceAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SecuritiesLendingRecordBalanceAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SecuritiesLendingRecordBalanceQuantity(::std::string::String);
impl ::std::ops::Deref for SecuritiesLendingRecordBalanceQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SecuritiesLendingRecordBalanceQuantity> for ::std::string::String {
    fn from(value: SecuritiesLendingRecordBalanceQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SecuritiesLendingRecordBalanceQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SecuritiesLendingRecordBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SecuritiesLendingRecordBalanceQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SecuritiesLendingRecordBalanceQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SecuritiesLendingRecordExecutionQuantity(::std::string::String);
impl ::std::ops::Deref for SecuritiesLendingRecordExecutionQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SecuritiesLendingRecordExecutionQuantity> for ::std::string::String {
    fn from(value: SecuritiesLendingRecordExecutionQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SecuritiesLendingRecordExecutionQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SecuritiesLendingRecordExecutionQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SecuritiesLendingRecordExecutionQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SecuritiesLendingRecordExecutionQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SecuritiesLendingRecordRepaymentQuantity(::std::string::String);
impl ::std::ops::Deref for SecuritiesLendingRecordRepaymentQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SecuritiesLendingRecordRepaymentQuantity> for ::std::string::String {
    fn from(value: SecuritiesLendingRecordRepaymentQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SecuritiesLendingRecordRepaymentQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SecuritiesLendingRecordRepaymentQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SecuritiesLendingRecordRepaymentQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SecuritiesLendingRecordRepaymentQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SecuritiesLendingResponse {
    #[serde(
        rename = "nextUntil",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_until: ::std::option::Option<::std::string::String>,
    pub records: ::std::vec::Vec<SecuritiesLendingRecord>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SellableQuantityResponse {
    #[serde(rename = "sellableQuantity")]
    pub sellable_quantity: SellableQuantityResponseSellableQuantity,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct SellableQuantityResponseSellableQuantity(::std::string::String);
impl ::std::ops::Deref for SellableQuantityResponseSellableQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SellableQuantityResponseSellableQuantity> for ::std::string::String {
    fn from(value: SellableQuantityResponseSellableQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for SellableQuantityResponseSellableQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for SellableQuantityResponseSellableQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SellableQuantityResponseSellableQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for SellableQuantityResponseSellableQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ShortSellingRecord {
    pub date: ::std::string::String,
    #[serde(rename = "shortSellingAmount")]
    pub short_selling_amount: ShortSellingRecordShortSellingAmount,
    #[serde(
        rename = "shortSellingAmountRate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub short_selling_amount_rate: ::std::option::Option<ShortSellingRecordShortSellingAmountRate>,
    #[serde(rename = "shortSellingVolume")]
    pub short_selling_volume: ShortSellingRecordShortSellingVolume,
    #[serde(
        rename = "shortSellingVolumeRate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub short_selling_volume_rate: ::std::option::Option<ShortSellingRecordShortSellingVolumeRate>,
    #[serde(rename = "updatedAt")]
    pub updated_at: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ShortSellingRecordShortSellingAmount(::std::string::String);
impl ::std::ops::Deref for ShortSellingRecordShortSellingAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ShortSellingRecordShortSellingAmount> for ::std::string::String {
    fn from(value: ShortSellingRecordShortSellingAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ShortSellingRecordShortSellingAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ShortSellingRecordShortSellingAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ShortSellingRecordShortSellingAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ShortSellingRecordShortSellingAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ShortSellingRecordShortSellingAmountRate(::std::string::String);
impl ::std::ops::Deref for ShortSellingRecordShortSellingAmountRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ShortSellingRecordShortSellingAmountRate> for ::std::string::String {
    fn from(value: ShortSellingRecordShortSellingAmountRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ShortSellingRecordShortSellingAmountRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ShortSellingRecordShortSellingAmountRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ShortSellingRecordShortSellingAmountRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ShortSellingRecordShortSellingAmountRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ShortSellingRecordShortSellingVolume(::std::string::String);
impl ::std::ops::Deref for ShortSellingRecordShortSellingVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ShortSellingRecordShortSellingVolume> for ::std::string::String {
    fn from(value: ShortSellingRecordShortSellingVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ShortSellingRecordShortSellingVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ShortSellingRecordShortSellingVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ShortSellingRecordShortSellingVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ShortSellingRecordShortSellingVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ShortSellingRecordShortSellingVolumeRate(::std::string::String);
impl ::std::ops::Deref for ShortSellingRecordShortSellingVolumeRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ShortSellingRecordShortSellingVolumeRate> for ::std::string::String {
    fn from(value: ShortSellingRecordShortSellingVolumeRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ShortSellingRecordShortSellingVolumeRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for ShortSellingRecordShortSellingVolumeRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ShortSellingRecordShortSellingVolumeRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ShortSellingRecordShortSellingVolumeRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ShortSellingResponse {
    #[serde(
        rename = "nextUntil",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_until: ::std::option::Option<::std::string::String>,
    pub records: ::std::vec::Vec<ShortSellingRecord>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct StockInfo {
    pub currency: Currency,
    #[serde(
        rename = "delistDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub delist_date: ::std::option::Option<::std::string::String>,
    #[serde(rename = "englishName")]
    pub english_name: ::std::string::String,
    #[serde(rename = "isCommonShare")]
    pub is_common_share: bool,
    #[serde(rename = "isinCode")]
    pub isin_code: ::std::string::String,
    #[serde(
        rename = "koreanMarketDetail",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub korean_market_detail: ::std::option::Option<KrMarketDetail>,
    #[serde(
        rename = "leverageFactor",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub leverage_factor: ::std::option::Option<StockInfoLeverageFactor>,
    #[serde(
        rename = "listDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub list_date: ::std::option::Option<::std::string::String>,
    pub market: StockInfoMarket,
    pub name: ::std::string::String,
    #[serde(rename = "securityType")]
    pub security_type: StockInfoSecurityType,
    #[serde(rename = "sharesOutstanding")]
    pub shares_outstanding: StockInfoSharesOutstanding,
    pub status: StockInfoStatus,
    pub symbol: ::std::string::String,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct StockInfoLeverageFactor(::std::string::String);
impl ::std::ops::Deref for StockInfoLeverageFactor {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<StockInfoLeverageFactor> for ::std::string::String {
    fn from(value: StockInfoLeverageFactor) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for StockInfoLeverageFactor {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for StockInfoLeverageFactor {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInfoLeverageFactor {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for StockInfoLeverageFactor {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum StockInfoMarket {
    #[serde(rename = "KOSPI")]
    Kospi,
    #[serde(rename = "KOSDAQ")]
    Kosdaq,
    #[serde(rename = "NYSE")]
    Nyse,
    #[serde(rename = "NASDAQ")]
    Nasdaq,
    #[serde(rename = "AMEX")]
    Amex,
    #[serde(rename = "KR_ETC")]
    KrEtc,
    #[serde(rename = "US_ETC")]
    UsEtc,
    Unknown(::std::string::String),
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum StockInfoSecurityType {
    #[serde(rename = "STOCK")]
    Stock,
    #[serde(rename = "FOREIGN_STOCK")]
    ForeignStock,
    #[serde(rename = "DEPOSITARY_RECEIPT")]
    DepositaryReceipt,
    #[serde(rename = "INFRASTRUCTURE_FUND")]
    InfrastructureFund,
    #[serde(rename = "REIT")]
    Reit,
    #[serde(rename = "ETF")]
    Etf,
    #[serde(rename = "FOREIGN_ETF")]
    ForeignEtf,
    #[serde(rename = "ETN")]
    Etn,
    #[serde(rename = "STOCK_WARRANTS")]
    StockWarrants,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct StockInfoSharesOutstanding(::std::string::String);
impl ::std::ops::Deref for StockInfoSharesOutstanding {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<StockInfoSharesOutstanding> for ::std::string::String {
    fn from(value: StockInfoSharesOutstanding) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for StockInfoSharesOutstanding {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for StockInfoSharesOutstanding {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInfoSharesOutstanding {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for StockInfoSharesOutstanding {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum StockInfoStatus {
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DELISTED")]
    Delisted,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct StockInstitutionTradingBreakdown {
    pub bank: InvestorTradingVolume,
    #[serde(rename = "financialInvestment")]
    pub financial_investment: InvestorTradingVolume,
    pub insurance: InvestorTradingVolume,
    #[serde(rename = "otherFinancialInstitution")]
    pub other_financial_institution: InvestorTradingVolume,
    #[serde(rename = "pensionFund")]
    pub pension_fund: InvestorTradingVolume,
    #[serde(rename = "privateEquityFund")]
    pub private_equity_fund: InvestorTradingVolume,
    pub trust: InvestorTradingVolume,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct StockInstitutionTradingVolume {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub breakdown: ::std::option::Option<StockInstitutionTradingBreakdown>,
    #[serde(rename = "buyVolume")]
    pub buy_volume: StockInstitutionTradingVolumeBuyVolume,
    #[serde(rename = "netBuyVolume")]
    pub net_buy_volume: StockInstitutionTradingVolumeNetBuyVolume,
    #[serde(rename = "sellVolume")]
    pub sell_volume: StockInstitutionTradingVolumeSellVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct StockInstitutionTradingVolumeBuyVolume(::std::string::String);
impl ::std::ops::Deref for StockInstitutionTradingVolumeBuyVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<StockInstitutionTradingVolumeBuyVolume> for ::std::string::String {
    fn from(value: StockInstitutionTradingVolumeBuyVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for StockInstitutionTradingVolumeBuyVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for StockInstitutionTradingVolumeBuyVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInstitutionTradingVolumeBuyVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for StockInstitutionTradingVolumeBuyVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct StockInstitutionTradingVolumeNetBuyVolume(::std::string::String);
impl ::std::ops::Deref for StockInstitutionTradingVolumeNetBuyVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<StockInstitutionTradingVolumeNetBuyVolume> for ::std::string::String {
    fn from(value: StockInstitutionTradingVolumeNetBuyVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for StockInstitutionTradingVolumeNetBuyVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for StockInstitutionTradingVolumeNetBuyVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInstitutionTradingVolumeNetBuyVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for StockInstitutionTradingVolumeNetBuyVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct StockInstitutionTradingVolumeSellVolume(::std::string::String);
impl ::std::ops::Deref for StockInstitutionTradingVolumeSellVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<StockInstitutionTradingVolumeSellVolume> for ::std::string::String {
    fn from(value: StockInstitutionTradingVolumeSellVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for StockInstitutionTradingVolumeSellVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for StockInstitutionTradingVolumeSellVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInstitutionTradingVolumeSellVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for StockInstitutionTradingVolumeSellVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct StockInvestorTradingRecord {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub cfd: ::std::option::Option<CfdBalance>,
    pub date: ::std::string::String,
    pub foreigner: InvestorTradingVolume,
    #[serde(
        rename = "foreignerHolding",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub foreigner_holding: ::std::option::Option<ForeignerHolding>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub individual: ::std::option::Option<InvestorTradingVolume>,
    pub institution: StockInstitutionTradingVolume,
    #[serde(
        rename = "otherCorporation",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub other_corporation: ::std::option::Option<InvestorTradingVolume>,
    #[serde(rename = "updatedAt")]
    pub updated_at: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct StockInvestorTradingResponse {
    #[serde(
        rename = "nextUntil",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub next_until: ::std::option::Option<::std::string::String>,
    pub records: ::std::vec::Vec<StockInvestorTradingRecord>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct StockWarning {
    #[serde(
        rename = "endDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub end_date: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub exchange: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "startDate",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub start_date: ::std::option::Option<::std::string::String>,
    #[serde(rename = "warningType")]
    pub warning_type: StockWarningWarningType,
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum StockWarningWarningType {
    #[serde(rename = "LIQUIDATION_TRADING")]
    LiquidationTrading,
    #[serde(rename = "OVERHEATED")]
    Overheated,
    #[serde(rename = "INVESTMENT_WARNING")]
    InvestmentWarning,
    #[serde(rename = "INVESTMENT_RISK")]
    InvestmentRisk,
    #[serde(rename = "VI_STATIC_AND_DYNAMIC")]
    ViStaticAndDynamic,
    #[serde(rename = "VI_STATIC")]
    ViStatic,
    #[serde(rename = "VI_DYNAMIC")]
    ViDynamic,
    #[serde(rename = "STOCK_WARRANTS")]
    StockWarrants,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Trade {
    pub currency: Currency,
    pub price: TradePrice,
    pub timestamp: ::std::string::String,
    pub volume: TradeVolume,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TradePrice(::std::string::String);
impl ::std::ops::Deref for TradePrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TradePrice> for ::std::string::String {
    fn from(value: TradePrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TradePrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TradePrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TradePrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TradePrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TradeVolume(::std::string::String);
impl ::std::ops::Deref for TradeVolume {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TradeVolume> for ::std::string::String {
    fn from(value: TradeVolume) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TradeVolume {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TradeVolume {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TradeVolume {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TradeVolume {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Transaction {
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub amount: ::std::option::Option<TransactionAmount>,
    #[serde(
        rename = "assetClass",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub asset_class: ::std::option::Option<AssetClass>,
    pub commission: TransactionCommission,
    pub currency: Currency,
    #[serde(
        rename = "exchangeRate",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub exchange_rate: ::std::option::Option<TransactionExchangeRate>,
    #[serde(
        rename = "filledPrice",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub filled_price: ::std::option::Option<TransactionFilledPrice>,
    #[serde(rename = "isCancellation")]
    pub is_cancellation: bool,
    #[serde(
        rename = "isinCode",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub isin_code: ::std::option::Option<::std::string::String>,
    #[serde(rename = "marketCountry")]
    pub market_country: MarketCountry,
    #[serde(
        rename = "netAmount",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub net_amount: ::std::option::Option<TransactionNetAmount>,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub quantity: ::std::option::Option<TransactionQuantity>,
    pub settled: bool,
    #[serde(rename = "settlementDate")]
    pub settlement_date: ::std::string::String,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub symbol: ::std::option::Option<::std::string::String>,
    pub tax: TransactionTax,
    #[serde(
        rename = "transactionTime",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub time: ::std::option::Option<::std::string::String>,
    #[serde(rename = "transactionType")]
    pub kind: TransactionType,
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionAmount(::std::string::String);
impl ::std::ops::Deref for TransactionAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionAmount> for ::std::string::String {
    fn from(value: TransactionAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionCommission(::std::string::String);
impl ::std::ops::Deref for TransactionCommission {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionCommission> for ::std::string::String {
    fn from(value: TransactionCommission) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionCommission {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionCommission {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionCommission {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionCommission {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionExchangeRate(::std::string::String);
impl ::std::ops::Deref for TransactionExchangeRate {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionExchangeRate> for ::std::string::String {
    fn from(value: TransactionExchangeRate) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionExchangeRate {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionExchangeRate {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionExchangeRate {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionExchangeRate {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionFilledPrice(::std::string::String);
impl ::std::ops::Deref for TransactionFilledPrice {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionFilledPrice> for ::std::string::String {
    fn from(value: TransactionFilledPrice) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionFilledPrice {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionFilledPrice {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionFilledPrice {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionFilledPrice {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionNetAmount(::std::string::String);
impl ::std::ops::Deref for TransactionNetAmount {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionNetAmount> for ::std::string::String {
    fn from(value: TransactionNetAmount) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionNetAmount {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionNetAmount {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionNetAmount {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionNetAmount {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionQuantity(::std::string::String);
impl ::std::ops::Deref for TransactionQuantity {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionQuantity> for ::std::string::String {
    fn from(value: TransactionQuantity) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionQuantity {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionQuantity {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionQuantity {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionQuantity {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TransactionTax(::std::string::String);
impl ::std::ops::Deref for TransactionTax {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TransactionTax> for ::std::string::String {
    fn from(value: TransactionTax) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TransactionTax {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        if value.chars().count() > 30_usize {
            return Err("longer than 30 characters".into());
        }
        Ok(Self(value.to_owned()))
    }
}
impl ::std::convert::TryFrom<&str> for TransactionTax {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionTax {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TransactionTax {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|error: error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(error.to_string())
            })
    }
}
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    :: serde :: Deserialize,
    :: serde :: Serialize,
)]
#[serde(from = "String", into = "String")]
pub enum TransactionType {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    #[serde(rename = "SECURITIES_TRANSFER_IN")]
    SecuritiesTransferIn,
    #[serde(rename = "SECURITIES_TRANSFER_OUT")]
    SecuritiesTransferOut,
    #[serde(rename = "DEPOSIT")]
    Deposit,
    #[serde(rename = "WITHDRAWAL")]
    Withdrawal,
    #[serde(rename = "EXCHANGE_IN")]
    ExchangeIn,
    #[serde(rename = "EXCHANGE_OUT")]
    ExchangeOut,
    #[serde(rename = "DIVIDEND")]
    Dividend,
    #[serde(rename = "INTEREST")]
    Interest,
    #[serde(rename = "FEE")]
    Fee,
    Unknown(::std::string::String),
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UsAfterMarketSession {
    #[serde(rename = "endTime")]
    pub end_time: ::std::string::String,
    #[serde(rename = "startTime")]
    pub start_time: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UsDayMarketSession {
    #[serde(rename = "endTime")]
    pub end_time: ::std::string::String,
    #[serde(rename = "startTime")]
    pub start_time: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UsMarketCalendarResponse {
    #[serde(rename = "nextBusinessDay")]
    pub next_business_day: UsMarketDay,
    #[serde(rename = "previousBusinessDay")]
    pub previous_business_day: UsMarketDay,
    pub today: UsMarketDay,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UsMarketDay {
    #[serde(
        rename = "afterMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub after_market: ::std::option::Option<UsAfterMarketSession>,
    pub date: ::std::string::String,
    #[serde(
        rename = "dayMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub day_market: ::std::option::Option<UsDayMarketSession>,
    #[serde(
        rename = "preMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pre_market: ::std::option::Option<UsPreMarketSession>,
    #[serde(
        rename = "regularMarket",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub regular_market: ::std::option::Option<UsRegularMarketSession>,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UsPreMarketSession {
    #[serde(rename = "endTime")]
    pub end_time: ::std::string::String,
    #[serde(rename = "startTime")]
    pub start_time: ::std::string::String,
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UsRegularMarketSession {
    #[serde(rename = "endTime")]
    pub end_time: ::std::string::String,
    #[serde(rename = "startTime")]
    pub start_time: ::std::string::String,
}
pub mod defaults {
    pub(super) fn order_create_request_quantity_based_time_in_force()
    -> super::OrderCreateQuantityBasedTimeInForce {
        super::OrderCreateQuantityBasedTimeInForce::Day
    }
}
pub mod error {
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
impl From<String> for AccountAccountType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "BROKERAGE" => Self::Brokerage,
            "OVERSEAS_DERIVATIVES" => Self::OverseasDerivatives,
            "PENSION_SAVINGS" => Self::PensionSavings,
            "RESHORING_INVESTMENT" => Self::ReshoringInvestment,
            _ => Self::Unknown(value),
        }
    }
}
impl From<AccountAccountType> for String {
    fn from(value: AccountAccountType) -> Self {
        match value {
            AccountAccountType::Brokerage => Self::from("BROKERAGE"),
            AccountAccountType::OverseasDerivatives => Self::from("OVERSEAS_DERIVATIVES"),
            AccountAccountType::PensionSavings => Self::from("PENSION_SAVINGS"),
            AccountAccountType::ReshoringInvestment => Self::from("RESHORING_INVESTMENT"),
            AccountAccountType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for AccountAccountType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Brokerage => f.write_str("BROKERAGE"),
            Self::OverseasDerivatives => f.write_str("OVERSEAS_DERIVATIVES"),
            Self::PensionSavings => f.write_str("PENSION_SAVINGS"),
            Self::ReshoringInvestment => f.write_str("RESHORING_INVESTMENT"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for AccountAccountType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for AssetClass {
    fn from(value: String) -> Self {
        match value.as_str() {
            "STOCK" => Self::Stock,
            "BOND" => Self::Bond,
            "OPTION" => Self::Option,
            _ => Self::Unknown(value),
        }
    }
}
impl From<AssetClass> for String {
    fn from(value: AssetClass) -> Self {
        match value {
            AssetClass::Stock => Self::from("STOCK"),
            AssetClass::Bond => Self::from("BOND"),
            AssetClass::Option => Self::from("OPTION"),
            AssetClass::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Stock => f.write_str("STOCK"),
            Self::Bond => f.write_str("BOND"),
            Self::Option => f.write_str("OPTION"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for AssetClass {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionRequestOrderSide {
    fn from(value: String) -> Self {
        match value.as_str() {
            "BUY" => Self::Buy,
            "SELL" => Self::Sell,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionRequestOrderSide> for String {
    fn from(value: ConditionRequestOrderSide) -> Self {
        match value {
            ConditionRequestOrderSide::Buy => Self::from("BUY"),
            ConditionRequestOrderSide::Sell => Self::from("SELL"),
            ConditionRequestOrderSide::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionRequestOrderSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionRequestOrderSide {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderConditionOrderSide {
    fn from(value: String) -> Self {
        match value.as_str() {
            "BUY" => Self::Buy,
            "SELL" => Self::Sell,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderConditionOrderSide> for String {
    fn from(value: ConditionalOrderConditionOrderSide) -> Self {
        match value {
            ConditionalOrderConditionOrderSide::Buy => Self::from("BUY"),
            ConditionalOrderConditionOrderSide::Sell => Self::from("SELL"),
            ConditionalOrderConditionOrderSide::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderConditionOrderSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderConditionOrderSide {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderConditionStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "WATCHING" => Self::Watching,
            "HOLDING" => Self::Holding,
            "PAUSED" => Self::Paused,
            "ORDERING" => Self::Ordering,
            "ORDERED" => Self::Ordered,
            "COMPLETED" => Self::Completed,
            "EXPIRED" => Self::Expired,
            "CANCELED" => Self::Canceled,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderConditionStatus> for String {
    fn from(value: ConditionalOrderConditionStatus) -> Self {
        match value {
            ConditionalOrderConditionStatus::Watching => Self::from("WATCHING"),
            ConditionalOrderConditionStatus::Holding => Self::from("HOLDING"),
            ConditionalOrderConditionStatus::Paused => Self::from("PAUSED"),
            ConditionalOrderConditionStatus::Ordering => Self::from("ORDERING"),
            ConditionalOrderConditionStatus::Ordered => Self::from("ORDERED"),
            ConditionalOrderConditionStatus::Completed => Self::from("COMPLETED"),
            ConditionalOrderConditionStatus::Expired => Self::from("EXPIRED"),
            ConditionalOrderConditionStatus::Canceled => Self::from("CANCELED"),
            ConditionalOrderConditionStatus::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderConditionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Watching => f.write_str("WATCHING"),
            Self::Holding => f.write_str("HOLDING"),
            Self::Paused => f.write_str("PAUSED"),
            Self::Ordering => f.write_str("ORDERING"),
            Self::Ordered => f.write_str("ORDERED"),
            Self::Completed => f.write_str("COMPLETED"),
            Self::Expired => f.write_str("EXPIRED"),
            Self::Canceled => f.write_str("CANCELED"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderConditionStatus {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderConditionType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "STOP" => Self::Stop,
            "PROFIT_RATE" => Self::ProfitRate,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderConditionType> for String {
    fn from(value: ConditionalOrderConditionType) -> Self {
        match value {
            ConditionalOrderConditionType::Stop => Self::from("STOP"),
            ConditionalOrderConditionType::ProfitRate => Self::from("PROFIT_RATE"),
            ConditionalOrderConditionType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderConditionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Stop => f.write_str("STOP"),
            Self::ProfitRate => f.write_str("PROFIT_RATE"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderConditionType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderCreateRequestOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderCreateRequestOrderType> for String {
    fn from(value: ConditionalOrderCreateRequestOrderType) -> Self {
        match value {
            ConditionalOrderCreateRequestOrderType::Limit => Self::from("LIMIT"),
            ConditionalOrderCreateRequestOrderType::Market => Self::from("MARKET"),
            ConditionalOrderCreateRequestOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderCreateRequestOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderCreateRequestOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderCreateRequestType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "SINGLE" => Self::Single,
            "OCO" => Self::Oco,
            "OTO" => Self::Oto,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderCreateRequestType> for String {
    fn from(value: ConditionalOrderCreateRequestType) -> Self {
        match value {
            ConditionalOrderCreateRequestType::Single => Self::from("SINGLE"),
            ConditionalOrderCreateRequestType::Oco => Self::from("OCO"),
            ConditionalOrderCreateRequestType::Oto => Self::from("OTO"),
            ConditionalOrderCreateRequestType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderCreateRequestType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Single => f.write_str("SINGLE"),
            Self::Oco => f.write_str("OCO"),
            Self::Oto => f.write_str("OTO"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderCreateRequestType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderDetailResponseMarket {
    fn from(value: String) -> Self {
        match value.as_str() {
            "KR" => Self::Kr,
            "US" => Self::Us,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderDetailResponseMarket> for String {
    fn from(value: ConditionalOrderDetailResponseMarket) -> Self {
        match value {
            ConditionalOrderDetailResponseMarket::Kr => Self::from("KR"),
            ConditionalOrderDetailResponseMarket::Us => Self::from("US"),
            ConditionalOrderDetailResponseMarket::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseMarket {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Kr => f.write_str("KR"),
            Self::Us => f.write_str("US"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseMarket {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderDetailResponseOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderDetailResponseOrderType> for String {
    fn from(value: ConditionalOrderDetailResponseOrderType) -> Self {
        match value {
            ConditionalOrderDetailResponseOrderType::Limit => Self::from("LIMIT"),
            ConditionalOrderDetailResponseOrderType::Market => Self::from("MARKET"),
            ConditionalOrderDetailResponseOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderDetailResponseStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "WATCHING" => Self::Watching,
            "PAUSED" => Self::Paused,
            "ORDERING" => Self::Ordering,
            "ORDERED" => Self::Ordered,
            "COMPLETED" => Self::Completed,
            "EXPIRED" => Self::Expired,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderDetailResponseStatus> for String {
    fn from(value: ConditionalOrderDetailResponseStatus) -> Self {
        match value {
            ConditionalOrderDetailResponseStatus::Watching => Self::from("WATCHING"),
            ConditionalOrderDetailResponseStatus::Paused => Self::from("PAUSED"),
            ConditionalOrderDetailResponseStatus::Ordering => Self::from("ORDERING"),
            ConditionalOrderDetailResponseStatus::Ordered => Self::from("ORDERED"),
            ConditionalOrderDetailResponseStatus::Completed => Self::from("COMPLETED"),
            ConditionalOrderDetailResponseStatus::Expired => Self::from("EXPIRED"),
            ConditionalOrderDetailResponseStatus::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Watching => f.write_str("WATCHING"),
            Self::Paused => f.write_str("PAUSED"),
            Self::Ordering => f.write_str("ORDERING"),
            Self::Ordered => f.write_str("ORDERED"),
            Self::Completed => f.write_str("COMPLETED"),
            Self::Expired => f.write_str("EXPIRED"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseStatus {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderDetailResponseType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "SINGLE" => Self::Single,
            "OCO" => Self::Oco,
            "OTO" => Self::Oto,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderDetailResponseType> for String {
    fn from(value: ConditionalOrderDetailResponseType) -> Self {
        match value {
            ConditionalOrderDetailResponseType::Single => Self::from("SINGLE"),
            ConditionalOrderDetailResponseType::Oco => Self::from("OCO"),
            ConditionalOrderDetailResponseType::Oto => Self::from("OTO"),
            ConditionalOrderDetailResponseType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Single => f.write_str("SINGLE"),
            Self::Oco => f.write_str("OCO"),
            Self::Oto => f.write_str("OTO"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderModifyRequestOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderModifyRequestOrderType> for String {
    fn from(value: ConditionalOrderModifyRequestOrderType) -> Self {
        match value {
            ConditionalOrderModifyRequestOrderType::Limit => Self::from("LIMIT"),
            ConditionalOrderModifyRequestOrderType::Market => Self::from("MARKET"),
            ConditionalOrderModifyRequestOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderModifyRequestOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderModifyRequestOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ConditionalOrderModifyRequestType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "SINGLE" => Self::Single,
            "OCO" => Self::Oco,
            "OTO" => Self::Oto,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ConditionalOrderModifyRequestType> for String {
    fn from(value: ConditionalOrderModifyRequestType) -> Self {
        match value {
            ConditionalOrderModifyRequestType::Single => Self::from("SINGLE"),
            ConditionalOrderModifyRequestType::Oco => Self::from("OCO"),
            ConditionalOrderModifyRequestType::Oto => Self::from("OTO"),
            ConditionalOrderModifyRequestType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ConditionalOrderModifyRequestType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Single => f.write_str("SINGLE"),
            Self::Oco => f.write_str("OCO"),
            Self::Oto => f.write_str("OTO"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderModifyRequestType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for Currency {
    fn from(value: String) -> Self {
        match value.as_str() {
            "KRW" => Self::Krw,
            "USD" => Self::Usd,
            _ => Self::Unknown(value),
        }
    }
}
impl From<Currency> for String {
    fn from(value: Currency) -> Self {
        match value {
            Currency::Krw => Self::from("KRW"),
            Currency::Usd => Self::from("USD"),
            Currency::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for Currency {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Krw => f.write_str("KRW"),
            Self::Usd => f.write_str("USD"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for Currency {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ExchangeRateResponseRateChangeType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "UP" => Self::Up,
            "EQUAL" => Self::Equal,
            "DOWN" => Self::Down,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ExchangeRateResponseRateChangeType> for String {
    fn from(value: ExchangeRateResponseRateChangeType) -> Self {
        match value {
            ExchangeRateResponseRateChangeType::Up => Self::from("UP"),
            ExchangeRateResponseRateChangeType::Equal => Self::from("EQUAL"),
            ExchangeRateResponseRateChangeType::Down => Self::from("DOWN"),
            ExchangeRateResponseRateChangeType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ExchangeRateResponseRateChangeType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Up => f.write_str("UP"),
            Self::Equal => f.write_str("EQUAL"),
            Self::Down => f.write_str("DOWN"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ExchangeRateResponseRateChangeType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for ListedStockSecurityType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "STOCK" => Self::Stock,
            "FOREIGN_STOCK" => Self::ForeignStock,
            "DEPOSITARY_RECEIPT" => Self::DepositaryReceipt,
            "INFRASTRUCTURE_FUND" => Self::InfrastructureFund,
            "REIT" => Self::Reit,
            "ETF" => Self::Etf,
            "FOREIGN_ETF" => Self::ForeignEtf,
            "ETN" => Self::Etn,
            "STOCK_WARRANTS" => Self::StockWarrants,
            _ => Self::Unknown(value),
        }
    }
}
impl From<ListedStockSecurityType> for String {
    fn from(value: ListedStockSecurityType) -> Self {
        match value {
            ListedStockSecurityType::Stock => Self::from("STOCK"),
            ListedStockSecurityType::ForeignStock => Self::from("FOREIGN_STOCK"),
            ListedStockSecurityType::DepositaryReceipt => Self::from("DEPOSITARY_RECEIPT"),
            ListedStockSecurityType::InfrastructureFund => Self::from("INFRASTRUCTURE_FUND"),
            ListedStockSecurityType::Reit => Self::from("REIT"),
            ListedStockSecurityType::Etf => Self::from("ETF"),
            ListedStockSecurityType::ForeignEtf => Self::from("FOREIGN_ETF"),
            ListedStockSecurityType::Etn => Self::from("ETN"),
            ListedStockSecurityType::StockWarrants => Self::from("STOCK_WARRANTS"),
            ListedStockSecurityType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for ListedStockSecurityType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Stock => f.write_str("STOCK"),
            Self::ForeignStock => f.write_str("FOREIGN_STOCK"),
            Self::DepositaryReceipt => f.write_str("DEPOSITARY_RECEIPT"),
            Self::InfrastructureFund => f.write_str("INFRASTRUCTURE_FUND"),
            Self::Reit => f.write_str("REIT"),
            Self::Etf => f.write_str("ETF"),
            Self::ForeignEtf => f.write_str("FOREIGN_ETF"),
            Self::Etn => f.write_str("ETN"),
            Self::StockWarrants => f.write_str("STOCK_WARRANTS"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for ListedStockSecurityType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for MarketCountry {
    fn from(value: String) -> Self {
        match value.as_str() {
            "KR" => Self::Kr,
            "US" => Self::Us,
            _ => Self::Unknown(value),
        }
    }
}
impl From<MarketCountry> for String {
    fn from(value: MarketCountry) -> Self {
        match value {
            MarketCountry::Kr => Self::from("KR"),
            MarketCountry::Us => Self::from("US"),
            MarketCountry::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for MarketCountry {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Kr => f.write_str("KR"),
            Self::Us => f.write_str("US"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for MarketCountry {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OAuth2ErrorResponseError {
    fn from(value: String) -> Self {
        match value.as_str() {
            "invalid_request" => Self::InvalidRequest,
            "invalid_client" => Self::InvalidClient,
            "invalid_grant" => Self::InvalidGrant,
            "unauthorized_client" => Self::UnauthorizedClient,
            "unsupported_grant_type" => Self::UnsupportedGrantType,
            "access_denied" => Self::AccessDenied,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OAuth2ErrorResponseError> for String {
    fn from(value: OAuth2ErrorResponseError) -> Self {
        match value {
            OAuth2ErrorResponseError::InvalidRequest => Self::from("invalid_request"),
            OAuth2ErrorResponseError::InvalidClient => Self::from("invalid_client"),
            OAuth2ErrorResponseError::InvalidGrant => Self::from("invalid_grant"),
            OAuth2ErrorResponseError::UnauthorizedClient => Self::from("unauthorized_client"),
            OAuth2ErrorResponseError::UnsupportedGrantType => Self::from("unsupported_grant_type"),
            OAuth2ErrorResponseError::AccessDenied => Self::from("access_denied"),
            OAuth2ErrorResponseError::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OAuth2ErrorResponseError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::InvalidRequest => f.write_str("invalid_request"),
            Self::InvalidClient => f.write_str("invalid_client"),
            Self::InvalidGrant => f.write_str("invalid_grant"),
            Self::UnauthorizedClient => f.write_str("unauthorized_client"),
            Self::UnsupportedGrantType => f.write_str("unsupported_grant_type"),
            Self::AccessDenied => f.write_str("access_denied"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OAuth2ErrorResponseError {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OAuth2TokenRequestGrantType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "client_credentials" => Self::ClientCredentials,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OAuth2TokenRequestGrantType> for String {
    fn from(value: OAuth2TokenRequestGrantType) -> Self {
        match value {
            OAuth2TokenRequestGrantType::ClientCredentials => Self::from("client_credentials"),
            OAuth2TokenRequestGrantType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OAuth2TokenRequestGrantType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::ClientCredentials => f.write_str("client_credentials"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OAuth2TokenRequestGrantType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OAuth2TokenResponseTokenType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Bearer" => Self::Bearer,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OAuth2TokenResponseTokenType> for String {
    fn from(value: OAuth2TokenResponseTokenType) -> Self {
        match value {
            OAuth2TokenResponseTokenType::Bearer => Self::from("Bearer"),
            OAuth2TokenResponseTokenType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OAuth2TokenResponseTokenType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Bearer => f.write_str("Bearer"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OAuth2TokenResponseTokenType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderCreateAmountBasedOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderCreateAmountBasedOrderType> for String {
    fn from(value: OrderCreateAmountBasedOrderType) -> Self {
        match value {
            OrderCreateAmountBasedOrderType::Market => Self::from("MARKET"),
            OrderCreateAmountBasedOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderCreateAmountBasedOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderCreateAmountBasedOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderCreateAmountBasedSide {
    fn from(value: String) -> Self {
        match value.as_str() {
            "BUY" => Self::Buy,
            "SELL" => Self::Sell,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderCreateAmountBasedSide> for String {
    fn from(value: OrderCreateAmountBasedSide) -> Self {
        match value {
            OrderCreateAmountBasedSide::Buy => Self::from("BUY"),
            OrderCreateAmountBasedSide::Sell => Self::from("SELL"),
            OrderCreateAmountBasedSide::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderCreateAmountBasedSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderCreateAmountBasedSide {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderCreateQuantityBasedOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderCreateQuantityBasedOrderType> for String {
    fn from(value: OrderCreateQuantityBasedOrderType) -> Self {
        match value {
            OrderCreateQuantityBasedOrderType::Limit => Self::from("LIMIT"),
            OrderCreateQuantityBasedOrderType::Market => Self::from("MARKET"),
            OrderCreateQuantityBasedOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderCreateQuantityBasedOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderCreateQuantityBasedSide {
    fn from(value: String) -> Self {
        match value.as_str() {
            "BUY" => Self::Buy,
            "SELL" => Self::Sell,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderCreateQuantityBasedSide> for String {
    fn from(value: OrderCreateQuantityBasedSide) -> Self {
        match value {
            OrderCreateQuantityBasedSide::Buy => Self::from("BUY"),
            OrderCreateQuantityBasedSide::Sell => Self::from("SELL"),
            OrderCreateQuantityBasedSide::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderCreateQuantityBasedSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedSide {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderCreateQuantityBasedTimeInForce {
    fn from(value: String) -> Self {
        match value.as_str() {
            "DAY" => Self::Day,
            "CLS" => Self::Cls,
            "OPG" => Self::Opg,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderCreateQuantityBasedTimeInForce> for String {
    fn from(value: OrderCreateQuantityBasedTimeInForce) -> Self {
        match value {
            OrderCreateQuantityBasedTimeInForce::Day => Self::from("DAY"),
            OrderCreateQuantityBasedTimeInForce::Cls => Self::from("CLS"),
            OrderCreateQuantityBasedTimeInForce::Opg => Self::from("OPG"),
            OrderCreateQuantityBasedTimeInForce::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderCreateQuantityBasedTimeInForce {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Day => f.write_str("DAY"),
            Self::Cls => f.write_str("CLS"),
            Self::Opg => f.write_str("OPG"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedTimeInForce {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderModifyRequestOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderModifyRequestOrderType> for String {
    fn from(value: OrderModifyRequestOrderType) -> Self {
        match value {
            OrderModifyRequestOrderType::Limit => Self::from("LIMIT"),
            OrderModifyRequestOrderType::Market => Self::from("MARKET"),
            OrderModifyRequestOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderModifyRequestOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderModifyRequestOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderOrderType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderOrderType> for String {
    fn from(value: OrderOrderType) -> Self {
        match value {
            OrderOrderType::Limit => Self::from("LIMIT"),
            OrderOrderType::Market => Self::from("MARKET"),
            OrderOrderType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderOrderType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderSide {
    fn from(value: String) -> Self {
        match value.as_str() {
            "BUY" => Self::Buy,
            "SELL" => Self::Sell,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderSide> for String {
    fn from(value: OrderSide) -> Self {
        match value {
            OrderSide::Buy => Self::from("BUY"),
            OrderSide::Sell => Self::from("SELL"),
            OrderSide::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderSide {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
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
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderStatus> for String {
    fn from(value: OrderStatus) -> Self {
        match value {
            OrderStatus::Pending => Self::from("PENDING"),
            OrderStatus::PendingCancel => Self::from("PENDING_CANCEL"),
            OrderStatus::PendingReplace => Self::from("PENDING_REPLACE"),
            OrderStatus::PartialFilled => Self::from("PARTIAL_FILLED"),
            OrderStatus::Filled => Self::from("FILLED"),
            OrderStatus::Canceled => Self::from("CANCELED"),
            OrderStatus::Rejected => Self::from("REJECTED"),
            OrderStatus::CancelRejected => Self::from("CANCEL_REJECTED"),
            OrderStatus::ReplaceRejected => Self::from("REPLACE_REJECTED"),
            OrderStatus::Replaced => Self::from("REPLACED"),
            OrderStatus::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Pending => f.write_str("PENDING"),
            Self::PendingCancel => f.write_str("PENDING_CANCEL"),
            Self::PendingReplace => f.write_str("PENDING_REPLACE"),
            Self::PartialFilled => f.write_str("PARTIAL_FILLED"),
            Self::Filled => f.write_str("FILLED"),
            Self::Canceled => f.write_str("CANCELED"),
            Self::Rejected => f.write_str("REJECTED"),
            Self::CancelRejected => f.write_str("CANCEL_REJECTED"),
            Self::ReplaceRejected => f.write_str("REPLACE_REJECTED"),
            Self::Replaced => f.write_str("REPLACED"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderStatus {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for OrderTimeInForce {
    fn from(value: String) -> Self {
        match value.as_str() {
            "DAY" => Self::Day,
            "CLS" => Self::Cls,
            "OPG" => Self::Opg,
            _ => Self::Unknown(value),
        }
    }
}
impl From<OrderTimeInForce> for String {
    fn from(value: OrderTimeInForce) -> Self {
        match value {
            OrderTimeInForce::Day => Self::from("DAY"),
            OrderTimeInForce::Cls => Self::from("CLS"),
            OrderTimeInForce::Opg => Self::from("OPG"),
            OrderTimeInForce::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for OrderTimeInForce {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Day => f.write_str("DAY"),
            Self::Cls => f.write_str("CLS"),
            Self::Opg => f.write_str("OPG"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for OrderTimeInForce {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for SectorChangeRateDuration {
    fn from(value: String) -> Self {
        match value.as_str() {
            "1d" => Self::X1d,
            "1w" => Self::X1w,
            "1mo" => Self::X1mo,
            "3mo" => Self::X3mo,
            "6mo" => Self::X6mo,
            "1y" => Self::X1y,
            _ => Self::Unknown(value),
        }
    }
}
impl From<SectorChangeRateDuration> for String {
    fn from(value: SectorChangeRateDuration) -> Self {
        match value {
            SectorChangeRateDuration::X1d => Self::from("1d"),
            SectorChangeRateDuration::X1w => Self::from("1w"),
            SectorChangeRateDuration::X1mo => Self::from("1mo"),
            SectorChangeRateDuration::X3mo => Self::from("3mo"),
            SectorChangeRateDuration::X6mo => Self::from("6mo"),
            SectorChangeRateDuration::X1y => Self::from("1y"),
            SectorChangeRateDuration::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for SectorChangeRateDuration {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::X1d => f.write_str("1d"),
            Self::X1w => f.write_str("1w"),
            Self::X1mo => f.write_str("1mo"),
            Self::X3mo => f.write_str("3mo"),
            Self::X6mo => f.write_str("6mo"),
            Self::X1y => f.write_str("1y"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for SectorChangeRateDuration {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for StockInfoMarket {
    fn from(value: String) -> Self {
        match value.as_str() {
            "KOSPI" => Self::Kospi,
            "KOSDAQ" => Self::Kosdaq,
            "NYSE" => Self::Nyse,
            "NASDAQ" => Self::Nasdaq,
            "AMEX" => Self::Amex,
            "KR_ETC" => Self::KrEtc,
            "US_ETC" => Self::UsEtc,
            _ => Self::Unknown(value),
        }
    }
}
impl From<StockInfoMarket> for String {
    fn from(value: StockInfoMarket) -> Self {
        match value {
            StockInfoMarket::Kospi => Self::from("KOSPI"),
            StockInfoMarket::Kosdaq => Self::from("KOSDAQ"),
            StockInfoMarket::Nyse => Self::from("NYSE"),
            StockInfoMarket::Nasdaq => Self::from("NASDAQ"),
            StockInfoMarket::Amex => Self::from("AMEX"),
            StockInfoMarket::KrEtc => Self::from("KR_ETC"),
            StockInfoMarket::UsEtc => Self::from("US_ETC"),
            StockInfoMarket::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for StockInfoMarket {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Kospi => f.write_str("KOSPI"),
            Self::Kosdaq => f.write_str("KOSDAQ"),
            Self::Nyse => f.write_str("NYSE"),
            Self::Nasdaq => f.write_str("NASDAQ"),
            Self::Amex => f.write_str("AMEX"),
            Self::KrEtc => f.write_str("KR_ETC"),
            Self::UsEtc => f.write_str("US_ETC"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for StockInfoMarket {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for StockInfoSecurityType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "STOCK" => Self::Stock,
            "FOREIGN_STOCK" => Self::ForeignStock,
            "DEPOSITARY_RECEIPT" => Self::DepositaryReceipt,
            "INFRASTRUCTURE_FUND" => Self::InfrastructureFund,
            "REIT" => Self::Reit,
            "ETF" => Self::Etf,
            "FOREIGN_ETF" => Self::ForeignEtf,
            "ETN" => Self::Etn,
            "STOCK_WARRANTS" => Self::StockWarrants,
            _ => Self::Unknown(value),
        }
    }
}
impl From<StockInfoSecurityType> for String {
    fn from(value: StockInfoSecurityType) -> Self {
        match value {
            StockInfoSecurityType::Stock => Self::from("STOCK"),
            StockInfoSecurityType::ForeignStock => Self::from("FOREIGN_STOCK"),
            StockInfoSecurityType::DepositaryReceipt => Self::from("DEPOSITARY_RECEIPT"),
            StockInfoSecurityType::InfrastructureFund => Self::from("INFRASTRUCTURE_FUND"),
            StockInfoSecurityType::Reit => Self::from("REIT"),
            StockInfoSecurityType::Etf => Self::from("ETF"),
            StockInfoSecurityType::ForeignEtf => Self::from("FOREIGN_ETF"),
            StockInfoSecurityType::Etn => Self::from("ETN"),
            StockInfoSecurityType::StockWarrants => Self::from("STOCK_WARRANTS"),
            StockInfoSecurityType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for StockInfoSecurityType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Stock => f.write_str("STOCK"),
            Self::ForeignStock => f.write_str("FOREIGN_STOCK"),
            Self::DepositaryReceipt => f.write_str("DEPOSITARY_RECEIPT"),
            Self::InfrastructureFund => f.write_str("INFRASTRUCTURE_FUND"),
            Self::Reit => f.write_str("REIT"),
            Self::Etf => f.write_str("ETF"),
            Self::ForeignEtf => f.write_str("FOREIGN_ETF"),
            Self::Etn => f.write_str("ETN"),
            Self::StockWarrants => f.write_str("STOCK_WARRANTS"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for StockInfoSecurityType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for StockInfoStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "SCHEDULED" => Self::Scheduled,
            "ACTIVE" => Self::Active,
            "DELISTED" => Self::Delisted,
            _ => Self::Unknown(value),
        }
    }
}
impl From<StockInfoStatus> for String {
    fn from(value: StockInfoStatus) -> Self {
        match value {
            StockInfoStatus::Scheduled => Self::from("SCHEDULED"),
            StockInfoStatus::Active => Self::from("ACTIVE"),
            StockInfoStatus::Delisted => Self::from("DELISTED"),
            StockInfoStatus::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for StockInfoStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Scheduled => f.write_str("SCHEDULED"),
            Self::Active => f.write_str("ACTIVE"),
            Self::Delisted => f.write_str("DELISTED"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for StockInfoStatus {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for StockWarningWarningType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LIQUIDATION_TRADING" => Self::LiquidationTrading,
            "OVERHEATED" => Self::Overheated,
            "INVESTMENT_WARNING" => Self::InvestmentWarning,
            "INVESTMENT_RISK" => Self::InvestmentRisk,
            "VI_STATIC_AND_DYNAMIC" => Self::ViStaticAndDynamic,
            "VI_STATIC" => Self::ViStatic,
            "VI_DYNAMIC" => Self::ViDynamic,
            "STOCK_WARRANTS" => Self::StockWarrants,
            _ => Self::Unknown(value),
        }
    }
}
impl From<StockWarningWarningType> for String {
    fn from(value: StockWarningWarningType) -> Self {
        match value {
            StockWarningWarningType::LiquidationTrading => Self::from("LIQUIDATION_TRADING"),
            StockWarningWarningType::Overheated => Self::from("OVERHEATED"),
            StockWarningWarningType::InvestmentWarning => Self::from("INVESTMENT_WARNING"),
            StockWarningWarningType::InvestmentRisk => Self::from("INVESTMENT_RISK"),
            StockWarningWarningType::ViStaticAndDynamic => Self::from("VI_STATIC_AND_DYNAMIC"),
            StockWarningWarningType::ViStatic => Self::from("VI_STATIC"),
            StockWarningWarningType::ViDynamic => Self::from("VI_DYNAMIC"),
            StockWarningWarningType::StockWarrants => Self::from("STOCK_WARRANTS"),
            StockWarningWarningType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for StockWarningWarningType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::LiquidationTrading => f.write_str("LIQUIDATION_TRADING"),
            Self::Overheated => f.write_str("OVERHEATED"),
            Self::InvestmentWarning => f.write_str("INVESTMENT_WARNING"),
            Self::InvestmentRisk => f.write_str("INVESTMENT_RISK"),
            Self::ViStaticAndDynamic => f.write_str("VI_STATIC_AND_DYNAMIC"),
            Self::ViStatic => f.write_str("VI_STATIC"),
            Self::ViDynamic => f.write_str("VI_DYNAMIC"),
            Self::StockWarrants => f.write_str("STOCK_WARRANTS"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for StockWarningWarningType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
impl From<String> for TransactionType {
    fn from(value: String) -> Self {
        match value.as_str() {
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
            _ => Self::Unknown(value),
        }
    }
}
impl From<TransactionType> for String {
    fn from(value: TransactionType) -> Self {
        match value {
            TransactionType::Buy => Self::from("BUY"),
            TransactionType::Sell => Self::from("SELL"),
            TransactionType::SecuritiesTransferIn => Self::from("SECURITIES_TRANSFER_IN"),
            TransactionType::SecuritiesTransferOut => Self::from("SECURITIES_TRANSFER_OUT"),
            TransactionType::Deposit => Self::from("DEPOSIT"),
            TransactionType::Withdrawal => Self::from("WITHDRAWAL"),
            TransactionType::ExchangeIn => Self::from("EXCHANGE_IN"),
            TransactionType::ExchangeOut => Self::from("EXCHANGE_OUT"),
            TransactionType::Dividend => Self::from("DIVIDEND"),
            TransactionType::Interest => Self::from("INTEREST"),
            TransactionType::Fee => Self::from("FEE"),
            TransactionType::Unknown(raw) => raw,
        }
    }
}
impl ::std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
            Self::SecuritiesTransferIn => f.write_str("SECURITIES_TRANSFER_IN"),
            Self::SecuritiesTransferOut => f.write_str("SECURITIES_TRANSFER_OUT"),
            Self::Deposit => f.write_str("DEPOSIT"),
            Self::Withdrawal => f.write_str("WITHDRAWAL"),
            Self::ExchangeIn => f.write_str("EXCHANGE_IN"),
            Self::ExchangeOut => f.write_str("EXCHANGE_OUT"),
            Self::Dividend => f.write_str("DIVIDEND"),
            Self::Interest => f.write_str("INTEREST"),
            Self::Fee => f.write_str("FEE"),
            Self::Unknown(raw) => f.write_str(raw),
        }
    }
}
impl ::std::str::FromStr for TransactionType {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(value)))
    }
}
