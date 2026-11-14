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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AccountAccountType {
    #[serde(rename = "BROKERAGE")]
    Brokerage,
    #[serde(rename = "OVERSEAS_DERIVATIVES")]
    OverseasDerivatives,
    #[serde(rename = "PENSION_SAVINGS")]
    PensionSavings,
    #[serde(rename = "RESHORING_INVESTMENT")]
    ReshoringInvestment,
}
impl ::std::fmt::Display for AccountAccountType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Brokerage => f.write_str("BROKERAGE"),
            Self::OverseasDerivatives => f.write_str("OVERSEAS_DERIVATIVES"),
            Self::PensionSavings => f.write_str("PENSION_SAVINGS"),
            Self::ReshoringInvestment => f.write_str("RESHORING_INVESTMENT"),
        }
    }
}
impl ::std::str::FromStr for AccountAccountType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BROKERAGE" => Ok(Self::Brokerage),
            "OVERSEAS_DERIVATIVES" => Ok(Self::OverseasDerivatives),
            "PENSION_SAVINGS" => Ok(Self::PensionSavings),
            "RESHORING_INVESTMENT" => Ok(Self::ReshoringInvestment),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AccountAccountType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AccountAccountType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AssetClass {
    #[serde(rename = "STOCK")]
    Stock,
    #[serde(rename = "BOND")]
    Bond,
    #[serde(rename = "OPTION")]
    Option,
}
impl ::std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Stock => f.write_str("STOCK"),
            Self::Bond => f.write_str("BOND"),
            Self::Option => f.write_str("OPTION"),
        }
    }
}
impl ::std::str::FromStr for AssetClass {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "STOCK" => Ok(Self::Stock),
            "BOND" => Ok(Self::Bond),
            "OPTION" => Ok(Self::Option),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AssetClass {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AssetClass {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionRequestOrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}
impl ::std::fmt::Display for ConditionRequestOrderSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
        }
    }
}
impl ::std::str::FromStr for ConditionRequestOrderSide {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionRequestOrderSide {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionRequestOrderSide {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderConditionOrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}
impl ::std::fmt::Display for ConditionalOrderConditionOrderSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderConditionOrderSide {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderConditionOrderSide {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderConditionOrderSide {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for ConditionalOrderConditionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Watching => f.write_str("WATCHING"),
            Self::Holding => f.write_str("HOLDING"),
            Self::Paused => f.write_str("PAUSED"),
            Self::Ordering => f.write_str("ORDERING"),
            Self::Ordered => f.write_str("ORDERED"),
            Self::Completed => f.write_str("COMPLETED"),
            Self::Expired => f.write_str("EXPIRED"),
            Self::Canceled => f.write_str("CANCELED"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderConditionStatus {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "WATCHING" => Ok(Self::Watching),
            "HOLDING" => Ok(Self::Holding),
            "PAUSED" => Ok(Self::Paused),
            "ORDERING" => Ok(Self::Ordering),
            "ORDERED" => Ok(Self::Ordered),
            "COMPLETED" => Ok(Self::Completed),
            "EXPIRED" => Ok(Self::Expired),
            "CANCELED" => Ok(Self::Canceled),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderConditionStatus {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderConditionStatus {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderConditionType {
    #[serde(rename = "STOP")]
    Stop,
    #[serde(rename = "PROFIT_RATE")]
    ProfitRate,
}
impl ::std::fmt::Display for ConditionalOrderConditionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Stop => f.write_str("STOP"),
            Self::ProfitRate => f.write_str("PROFIT_RATE"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderConditionType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "STOP" => Ok(Self::Stop),
            "PROFIT_RATE" => Ok(Self::ProfitRate),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderConditionType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderConditionType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderCreateRequestOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for ConditionalOrderCreateRequestOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderCreateRequestOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIMIT" => Ok(Self::Limit),
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderCreateRequestOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderCreateRequestOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderCreateRequestType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "OCO")]
    Oco,
    #[serde(rename = "OTO")]
    Oto,
}
impl ::std::fmt::Display for ConditionalOrderCreateRequestType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Single => f.write_str("SINGLE"),
            Self::Oco => f.write_str("OCO"),
            Self::Oto => f.write_str("OTO"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderCreateRequestType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "SINGLE" => Ok(Self::Single),
            "OCO" => Ok(Self::Oco),
            "OTO" => Ok(Self::Oto),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderCreateRequestType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderCreateRequestType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderDetailResponseMarket {
    #[serde(rename = "KR")]
    Kr,
    #[serde(rename = "US")]
    Us,
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseMarket {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Kr => f.write_str("KR"),
            Self::Us => f.write_str("US"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseMarket {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "KR" => Ok(Self::Kr),
            "US" => Ok(Self::Us),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderDetailResponseMarket {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderDetailResponseMarket {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderDetailResponseOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIMIT" => Ok(Self::Limit),
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderDetailResponseOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderDetailResponseOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Watching => f.write_str("WATCHING"),
            Self::Paused => f.write_str("PAUSED"),
            Self::Ordering => f.write_str("ORDERING"),
            Self::Ordered => f.write_str("ORDERED"),
            Self::Completed => f.write_str("COMPLETED"),
            Self::Expired => f.write_str("EXPIRED"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseStatus {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "WATCHING" => Ok(Self::Watching),
            "PAUSED" => Ok(Self::Paused),
            "ORDERING" => Ok(Self::Ordering),
            "ORDERED" => Ok(Self::Ordered),
            "COMPLETED" => Ok(Self::Completed),
            "EXPIRED" => Ok(Self::Expired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderDetailResponseStatus {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderDetailResponseStatus {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderDetailResponseType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "OCO")]
    Oco,
    #[serde(rename = "OTO")]
    Oto,
}
impl ::std::fmt::Display for ConditionalOrderDetailResponseType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Single => f.write_str("SINGLE"),
            Self::Oco => f.write_str("OCO"),
            Self::Oto => f.write_str("OTO"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderDetailResponseType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "SINGLE" => Ok(Self::Single),
            "OCO" => Ok(Self::Oco),
            "OTO" => Ok(Self::Oto),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderDetailResponseType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderDetailResponseType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderModifyRequestOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for ConditionalOrderModifyRequestOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderModifyRequestOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIMIT" => Ok(Self::Limit),
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderModifyRequestOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderModifyRequestOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConditionalOrderModifyRequestType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "OCO")]
    Oco,
    #[serde(rename = "OTO")]
    Oto,
}
impl ::std::fmt::Display for ConditionalOrderModifyRequestType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Single => f.write_str("SINGLE"),
            Self::Oco => f.write_str("OCO"),
            Self::Oto => f.write_str("OTO"),
        }
    }
}
impl ::std::str::FromStr for ConditionalOrderModifyRequestType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "SINGLE" => Ok(Self::Single),
            "OCO" => Ok(Self::Oco),
            "OTO" => Ok(Self::Oto),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConditionalOrderModifyRequestType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ConditionalOrderModifyRequestType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum Currency {
    #[serde(rename = "KRW")]
    Krw,
    #[serde(rename = "USD")]
    Usd,
}
impl ::std::fmt::Display for Currency {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Krw => f.write_str("KRW"),
            Self::Usd => f.write_str("USD"),
        }
    }
}
impl ::std::str::FromStr for Currency {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "KRW" => Ok(Self::Krw),
            "USD" => Ok(Self::Usd),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Currency {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Currency {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ExchangeRateResponseRateChangeType {
    #[serde(rename = "UP")]
    Up,
    #[serde(rename = "EQUAL")]
    Equal,
    #[serde(rename = "DOWN")]
    Down,
}
impl ::std::fmt::Display for ExchangeRateResponseRateChangeType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Up => f.write_str("UP"),
            Self::Equal => f.write_str("EQUAL"),
            Self::Down => f.write_str("DOWN"),
        }
    }
}
impl ::std::str::FromStr for ExchangeRateResponseRateChangeType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "UP" => Ok(Self::Up),
            "EQUAL" => Ok(Self::Equal),
            "DOWN" => Ok(Self::Down),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ExchangeRateResponseRateChangeType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExchangeRateResponseRateChangeType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    #[serde(rename = "rankedAt")]
    pub ranked_at: ::std::string::String,
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for ListedStockSecurityType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Stock => f.write_str("STOCK"),
            Self::ForeignStock => f.write_str("FOREIGN_STOCK"),
            Self::DepositaryReceipt => f.write_str("DEPOSITARY_RECEIPT"),
            Self::InfrastructureFund => f.write_str("INFRASTRUCTURE_FUND"),
            Self::Reit => f.write_str("REIT"),
            Self::Etf => f.write_str("ETF"),
            Self::ForeignEtf => f.write_str("FOREIGN_ETF"),
            Self::Etn => f.write_str("ETN"),
            Self::StockWarrants => f.write_str("STOCK_WARRANTS"),
        }
    }
}
impl ::std::str::FromStr for ListedStockSecurityType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "STOCK" => Ok(Self::Stock),
            "FOREIGN_STOCK" => Ok(Self::ForeignStock),
            "DEPOSITARY_RECEIPT" => Ok(Self::DepositaryReceipt),
            "INFRASTRUCTURE_FUND" => Ok(Self::InfrastructureFund),
            "REIT" => Ok(Self::Reit),
            "ETF" => Ok(Self::Etf),
            "FOREIGN_ETF" => Ok(Self::ForeignEtf),
            "ETN" => Ok(Self::Etn),
            "STOCK_WARRANTS" => Ok(Self::StockWarrants),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ListedStockSecurityType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ListedStockSecurityType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum MarketCountry {
    #[serde(rename = "KR")]
    Kr,
    #[serde(rename = "US")]
    Us,
}
impl ::std::fmt::Display for MarketCountry {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Kr => f.write_str("KR"),
            Self::Us => f.write_str("US"),
        }
    }
}
impl ::std::str::FromStr for MarketCountry {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "KR" => Ok(Self::Kr),
            "US" => Ok(Self::Us),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for MarketCountry {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MarketCountry {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for OAuth2ErrorResponseError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::InvalidRequest => f.write_str("invalid_request"),
            Self::InvalidClient => f.write_str("invalid_client"),
            Self::InvalidGrant => f.write_str("invalid_grant"),
            Self::UnauthorizedClient => f.write_str("unauthorized_client"),
            Self::UnsupportedGrantType => f.write_str("unsupported_grant_type"),
            Self::AccessDenied => f.write_str("access_denied"),
        }
    }
}
impl ::std::str::FromStr for OAuth2ErrorResponseError {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "invalid_request" => Ok(Self::InvalidRequest),
            "invalid_client" => Ok(Self::InvalidClient),
            "invalid_grant" => Ok(Self::InvalidGrant),
            "unauthorized_client" => Ok(Self::UnauthorizedClient),
            "unsupported_grant_type" => Ok(Self::UnsupportedGrantType),
            "access_denied" => Ok(Self::AccessDenied),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OAuth2ErrorResponseError {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OAuth2ErrorResponseError {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OAuth2TokenRequest {
    pub client_id: ::std::string::String,
    pub client_secret: ::std::string::String,
    pub grant_type: OAuth2TokenRequestGrantType,
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OAuth2TokenRequestGrantType {
    #[serde(rename = "client_credentials")]
    ClientCredentials,
}
impl ::std::fmt::Display for OAuth2TokenRequestGrantType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ClientCredentials => f.write_str("client_credentials"),
        }
    }
}
impl ::std::str::FromStr for OAuth2TokenRequestGrantType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "client_credentials" => Ok(Self::ClientCredentials),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OAuth2TokenRequestGrantType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OAuth2TokenRequestGrantType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OAuth2TokenResponse {
    pub access_token: ::std::string::String,
    pub expires_in: i64,
    pub token_type: OAuth2TokenResponseTokenType,
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OAuth2TokenResponseTokenType {
    Bearer,
}
impl ::std::fmt::Display for OAuth2TokenResponseTokenType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Bearer => f.write_str("Bearer"),
        }
    }
}
impl ::std::str::FromStr for OAuth2TokenResponseTokenType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "Bearer" => Ok(Self::Bearer),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OAuth2TokenResponseTokenType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OAuth2TokenResponseTokenType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderCreateAmountBasedOrderType {
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for OrderCreateAmountBasedOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for OrderCreateAmountBasedOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateAmountBasedOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateAmountBasedOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderCreateAmountBasedSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}
impl ::std::fmt::Display for OrderCreateAmountBasedSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
        }
    }
}
impl ::std::str::FromStr for OrderCreateAmountBasedSide {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateAmountBasedSide {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateAmountBasedSide {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderCreateQuantityBasedOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for OrderCreateQuantityBasedOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIMIT" => Ok(Self::Limit),
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateQuantityBasedOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateQuantityBasedOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderCreateQuantityBasedSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}
impl ::std::fmt::Display for OrderCreateQuantityBasedSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
        }
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedSide {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateQuantityBasedSide {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateQuantityBasedSide {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Default,
)]
pub enum OrderCreateQuantityBasedTimeInForce {
    #[serde(rename = "DAY")]
    #[default]
    Day,
    #[serde(rename = "CLS")]
    Cls,
    #[serde(rename = "OPG")]
    Opg,
}
impl ::std::fmt::Display for OrderCreateQuantityBasedTimeInForce {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Day => f.write_str("DAY"),
            Self::Cls => f.write_str("CLS"),
            Self::Opg => f.write_str("OPG"),
        }
    }
}
impl ::std::str::FromStr for OrderCreateQuantityBasedTimeInForce {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "DAY" => Ok(Self::Day),
            "CLS" => Ok(Self::Cls),
            "OPG" => Ok(Self::Opg),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderCreateQuantityBasedTimeInForce {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderCreateQuantityBasedTimeInForce {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum OrderCreateRequest {
    QuantityBased {
        #[doc = "클라이언트 지정 주문 식별자. 멱등성 키로 사용됩니다.\n- 미전달: 멱등성 미적용. 매 요청을 별개 주문으로 처리합니다.\n- 전달: 동일 값으로 재요청 시 이전 주문 결과를 그대로 재반환합니다.\n서버는 자동 생성하지 않습니다.\n최대 36자, 영숫자 및 `-`, `_` 허용.\n멱등성 키는 10분간 유효하며, 이후 동일 값으로 재요청 시 새 주문으로 처리됩니다.\n"]
        #[serde(
            rename = "clientOrderId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        client_order_id: ::std::option::Option<OrderCreateQuantityBasedClientOrderId>,
        #[doc = "착오주문 방지를 위한 주문 확인 플래그. 기본값 `false`.\n1억원 이상의 주문 시 `true`가 아니면 `400 confirm-high-value-required` 에러를 반환합니다.\n사용자가 해당 주문의 금액을 인지하고 있음을 표시하기 위한 필드입니다.\n"]
        #[serde(rename = "confirmHighValueOrder", default)]
        confirm_high_value_order: bool,
        #[doc = "호가 유형.\n- `LIMIT`: 지정가\n- `MARKET`: 시장가\n"]
        #[serde(rename = "orderType")]
        order_type: OrderCreateQuantityBasedOrderType,
        #[doc = "주문 가격. `orderType`이 `LIMIT` 일 때만 사용합니다.\n- `LIMIT`: 필수. 미전달 시 `400 invalid-request`.\n- `MARKET`: 전달 불가. 전달 시 `400 invalid-request`.\n- KR: 정수 (원 단위). 호가 단위에 맞아야 합니다 (예: 50,000~200,000원 구간은 100원 단위). 맞지 않으면 `400 invalid-request` 에러와 함께 올바른 호가 단위가 `data`에 포함됩니다.\n- US: 소수점 (달러 단위).\n  - $1 미만: 소수점 넷째 자리까지 (그 이하 자릿수는 절삭).\n  - $1 이상: 소수점 둘째 자리까지 (그 이하 자릿수는 절삭).\n"]
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        price: ::std::option::Option<OrderCreateQuantityBasedPrice>,
        #[doc = "주문 수량 (주 단위). 지정한 수량만큼 주문합니다.\n- 기본: 양의 정수만 가능합니다.\n- 소수점 수량: 미국 주식 시장가 매도(`orderType=MARKET` + `side=SELL`) 주문에만 허용됩니다.\n  그 외(매수/지정가/국내) 소수점 수량은 `400 invalid-request` 를 반환합니다.\n  소수점 매수는 `orderAmount` 를 사용하세요.\n  소수점 수량 매도는 정규장 시작부터 정규장 종료 1시간 전까지만 접수 가능하며, 그 외 시간 요청 시 `422 fractional-quantity-outside-regular-hours` 를 반환합니다.\n  소수점 수량은 소수점 6자리까지 지원하며, 초과 시 `400 invalid-request` (`fractional-quantity-scale-exceeded`) 를 반환합니다.\n"]
        quantity: OrderCreateQuantityBasedQuantity,
        #[doc = "주문 방향"]
        side: OrderCreateQuantityBasedSide,
        #[doc = "종목 심볼. KRX: 6자리 종목코드 (숫자 또는 영문·숫자 조합), US: 영문 티커"]
        symbol: ::std::string::String,
        #[doc = "주문 유효 조건 (Time In Force). 미전달 시 `DAY`. `orderType` 과 결합되어 주문 방식이 결정됩니다 (예: `LIMIT` + `CLS` = LOC).\n- `DAY`: 당일 유효 (Day). 정규장 종료까지 미체결분은 자동 취소됩니다.\n- `CLS`: 장 마감 주문 (At the Close). 현재 미국 주식 + `orderType=LIMIT` 조합만 지원합니다.\n- `OPG`: 장 개시 주문 (At the Opening, 국내 시가단일가). 현재 국내 주식 전용이며 `orderType` 은 `LIMIT`/`MARKET` 모두 지원합니다. 세션 시간(장전 사전접수) 외 접수는 원장에서 거절될 수 있습니다.\n"]
        #[serde(
            rename = "timeInForce",
            default = "defaults::order_create_request_quantity_based_time_in_force"
        )]
        time_in_force: OrderCreateQuantityBasedTimeInForce,
    },
    AmountBased {
        #[doc = "클라이언트 지정 주문 식별자. 멱등성 키로 사용됩니다.\n- 미전달: 멱등성 미적용. 매 요청을 별개 주문으로 처리합니다.\n- 전달: 동일 값으로 재요청 시 이전 주문 결과를 그대로 재반환합니다.\n서버는 자동 생성하지 않습니다.\n최대 36자, 영숫자 및 `-`, `_` 허용.\n멱등성 키는 10분간 유효하며, 이후 동일 값으로 재요청 시 새 주문으로 처리됩니다.\n"]
        #[serde(
            rename = "clientOrderId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        client_order_id: ::std::option::Option<OrderCreateAmountBasedClientOrderId>,
        #[doc = "착오주문 방지를 위한 주문 확인 플래그. 기본값 `false`.\n1억원 이상의 주문 시 `true`가 아니면 `400 confirm-high-value-required` 에러를 반환합니다.\n사용자가 해당 주문의 금액을 인지하고 있음을 표시하기 위한 필드입니다.\n"]
        #[serde(rename = "confirmHighValueOrder", default)]
        confirm_high_value_order: bool,
        #[doc = "주문 금액 (달러). 지정한 금액만큼 주문합니다.\n체결 수량은 체결 시점의 시장가에 따라 결정됩니다.\n\nQuantity-based 와의 차이: quantity 는 수량을 확정하고 비용이 변동하며,\norderAmount 는 금액을 확정하고 수량이 변동합니다.\n\n정규장 시작부터 정규장 종료 1시간 전까지만 접수 가능합니다.\n그 외 시간 요청 시 `422 amount-order-outside-regular-hours` 에러를 반환합니다.\n"]
        #[serde(rename = "orderAmount")]
        order_amount: OrderCreateAmountBasedOrderAmount,
        #[doc = "호가 유형. 금액 기반 주문은 `MARKET` 만 허용합니다."]
        #[serde(rename = "orderType")]
        order_type: OrderCreateAmountBasedOrderType,
        #[doc = "주문 방향"]
        side: OrderCreateAmountBasedSide,
        #[doc = "US 종목 심볼 (영문 티커). 금액 기반 주문은 US MARKET 전용입니다."]
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
    #[serde(rename = "filledAt")]
    pub filled_at: ::std::string::String,
    #[serde(rename = "filledQuantity")]
    pub filled_quantity: OrderExecutionFilledQuantity,
    #[serde(rename = "settlementDate")]
    pub settlement_date: ::std::string::String,
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderModifyRequestOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for OrderModifyRequestOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for OrderModifyRequestOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIMIT" => Ok(Self::Limit),
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderModifyRequestOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderModifyRequestOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderOrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
}
impl ::std::fmt::Display for OrderOrderType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Limit => f.write_str("LIMIT"),
            Self::Market => f.write_str("MARKET"),
        }
    }
}
impl ::std::str::FromStr for OrderOrderType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIMIT" => Ok(Self::Limit),
            "MARKET" => Ok(Self::Market),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderOrderType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderOrderType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}
impl ::std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Buy => f.write_str("BUY"),
            Self::Sell => f.write_str("SELL"),
        }
    }
}
impl ::std::str::FromStr for OrderSide {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderSide {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderSide {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
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
        }
    }
}
impl ::std::str::FromStr for OrderStatus {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "PENDING_CANCEL" => Ok(Self::PendingCancel),
            "PENDING_REPLACE" => Ok(Self::PendingReplace),
            "PARTIAL_FILLED" => Ok(Self::PartialFilled),
            "FILLED" => Ok(Self::Filled),
            "CANCELED" => Ok(Self::Canceled),
            "REJECTED" => Ok(Self::Rejected),
            "CANCEL_REJECTED" => Ok(Self::CancelRejected),
            "REPLACE_REJECTED" => Ok(Self::ReplaceRejected),
            "REPLACED" => Ok(Self::Replaced),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderStatus {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderStatus {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum OrderTimeInForce {
    #[serde(rename = "DAY")]
    Day,
    #[serde(rename = "CLS")]
    Cls,
    #[serde(rename = "OPG")]
    Opg,
}
impl ::std::fmt::Display for OrderTimeInForce {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Day => f.write_str("DAY"),
            Self::Cls => f.write_str("CLS"),
            Self::Opg => f.write_str("OPG"),
        }
    }
}
impl ::std::str::FromStr for OrderTimeInForce {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "DAY" => Ok(Self::Day),
            "CLS" => Ok(Self::Cls),
            "OPG" => Ok(Self::Opg),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OrderTimeInForce {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OrderTimeInForce {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for SectorChangeRateDuration {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::X1d => f.write_str("1d"),
            Self::X1w => f.write_str("1w"),
            Self::X1mo => f.write_str("1mo"),
            Self::X3mo => f.write_str("3mo"),
            Self::X6mo => f.write_str("6mo"),
            Self::X1y => f.write_str("1y"),
        }
    }
}
impl ::std::str::FromStr for SectorChangeRateDuration {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "1d" => Ok(Self::X1d),
            "1w" => Ok(Self::X1w),
            "1mo" => Ok(Self::X1mo),
            "3mo" => Ok(Self::X3mo),
            "6mo" => Ok(Self::X6mo),
            "1y" => Ok(Self::X1y),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SectorChangeRateDuration {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SectorChangeRateDuration {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for StockInfoMarket {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Kospi => f.write_str("KOSPI"),
            Self::Kosdaq => f.write_str("KOSDAQ"),
            Self::Nyse => f.write_str("NYSE"),
            Self::Nasdaq => f.write_str("NASDAQ"),
            Self::Amex => f.write_str("AMEX"),
            Self::KrEtc => f.write_str("KR_ETC"),
            Self::UsEtc => f.write_str("US_ETC"),
        }
    }
}
impl ::std::str::FromStr for StockInfoMarket {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "KOSPI" => Ok(Self::Kospi),
            "KOSDAQ" => Ok(Self::Kosdaq),
            "NYSE" => Ok(Self::Nyse),
            "NASDAQ" => Ok(Self::Nasdaq),
            "AMEX" => Ok(Self::Amex),
            "KR_ETC" => Ok(Self::KrEtc),
            "US_ETC" => Ok(Self::UsEtc),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StockInfoMarket {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInfoMarket {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for StockInfoSecurityType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Stock => f.write_str("STOCK"),
            Self::ForeignStock => f.write_str("FOREIGN_STOCK"),
            Self::DepositaryReceipt => f.write_str("DEPOSITARY_RECEIPT"),
            Self::InfrastructureFund => f.write_str("INFRASTRUCTURE_FUND"),
            Self::Reit => f.write_str("REIT"),
            Self::Etf => f.write_str("ETF"),
            Self::ForeignEtf => f.write_str("FOREIGN_ETF"),
            Self::Etn => f.write_str("ETN"),
            Self::StockWarrants => f.write_str("STOCK_WARRANTS"),
        }
    }
}
impl ::std::str::FromStr for StockInfoSecurityType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "STOCK" => Ok(Self::Stock),
            "FOREIGN_STOCK" => Ok(Self::ForeignStock),
            "DEPOSITARY_RECEIPT" => Ok(Self::DepositaryReceipt),
            "INFRASTRUCTURE_FUND" => Ok(Self::InfrastructureFund),
            "REIT" => Ok(Self::Reit),
            "ETF" => Ok(Self::Etf),
            "FOREIGN_ETF" => Ok(Self::ForeignEtf),
            "ETN" => Ok(Self::Etn),
            "STOCK_WARRANTS" => Ok(Self::StockWarrants),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StockInfoSecurityType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInfoSecurityType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum StockInfoStatus {
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DELISTED")]
    Delisted,
}
impl ::std::fmt::Display for StockInfoStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Scheduled => f.write_str("SCHEDULED"),
            Self::Active => f.write_str("ACTIVE"),
            Self::Delisted => f.write_str("DELISTED"),
        }
    }
}
impl ::std::str::FromStr for StockInfoStatus {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "SCHEDULED" => Ok(Self::Scheduled),
            "ACTIVE" => Ok(Self::Active),
            "DELISTED" => Ok(Self::Delisted),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StockInfoStatus {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockInfoStatus {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for StockWarningWarningType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::LiquidationTrading => f.write_str("LIQUIDATION_TRADING"),
            Self::Overheated => f.write_str("OVERHEATED"),
            Self::InvestmentWarning => f.write_str("INVESTMENT_WARNING"),
            Self::InvestmentRisk => f.write_str("INVESTMENT_RISK"),
            Self::ViStaticAndDynamic => f.write_str("VI_STATIC_AND_DYNAMIC"),
            Self::ViStatic => f.write_str("VI_STATIC"),
            Self::ViDynamic => f.write_str("VI_DYNAMIC"),
            Self::StockWarrants => f.write_str("STOCK_WARRANTS"),
        }
    }
}
impl ::std::str::FromStr for StockWarningWarningType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "LIQUIDATION_TRADING" => Ok(Self::LiquidationTrading),
            "OVERHEATED" => Ok(Self::Overheated),
            "INVESTMENT_WARNING" => Ok(Self::InvestmentWarning),
            "INVESTMENT_RISK" => Ok(Self::InvestmentRisk),
            "VI_STATIC_AND_DYNAMIC" => Ok(Self::ViStaticAndDynamic),
            "VI_STATIC" => Ok(Self::ViStatic),
            "VI_DYNAMIC" => Ok(Self::ViDynamic),
            "STOCK_WARRANTS" => Ok(Self::StockWarrants),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StockWarningWarningType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StockWarningWarningType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
    #[serde(rename = "transactionTime")]
    pub time: ::std::string::String,
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
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
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
}
impl ::std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
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
        }
    }
}
impl ::std::str::FromStr for TransactionType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        match value {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            "SECURITIES_TRANSFER_IN" => Ok(Self::SecuritiesTransferIn),
            "SECURITIES_TRANSFER_OUT" => Ok(Self::SecuritiesTransferOut),
            "DEPOSIT" => Ok(Self::Deposit),
            "WITHDRAWAL" => Ok(Self::Withdrawal),
            "EXCHANGE_IN" => Ok(Self::ExchangeIn),
            "EXCHANGE_OUT" => Ok(Self::ExchangeOut),
            "DIVIDEND" => Ok(Self::Dividend),
            "INTEREST" => Ok(Self::Interest),
            "FEE" => Ok(Self::Fee),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TransactionType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionType {
    type Error = error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, error::ConversionError> {
        value.parse()
    }
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
#[doc = " Generation of default values for serde."]
pub mod defaults {
    pub(super) fn order_create_request_quantity_based_time_in_force()
    -> super::OrderCreateQuantityBasedTimeInForce {
        super::OrderCreateQuantityBasedTimeInForce::Day
    }
}
#[doc = " Error types."]
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
