use super::codec::{Fields, invalid};
use application::{
    AccessDenial, BrokerCapability, HttpStatus, IncompleteReason, MissingData, ReadEndpoint,
    RequestRejection, ResponseMismatch, UnavailableCause,
};
pub(super) fn put_read_endpoint(fields: &mut Fields, value: ReadEndpoint) {
    fields.push(
        &(match value {
            ReadEndpoint::Accounts => "Accounts",
            ReadEndpoint::Holdings => "Holdings",
            ReadEndpoint::Transactions => "Transactions",
            ReadEndpoint::Quotes => "Quotes",
            ReadEndpoint::Candles => "Candles",
            ReadEndpoint::BuyingPower => "BuyingPower",
            ReadEndpoint::FxRate => "FxRate",
            ReadEndpoint::OrderFxRates => "OrderFxRates",
            ReadEndpoint::SellableQuantity => "SellableQuantity",
            ReadEndpoint::OpenOrders => "OpenOrders",
            ReadEndpoint::ClosedOrders => "ClosedOrders",
            ReadEndpoint::Order => "Order",
            ReadEndpoint::StockWarnings => "StockWarnings",
            ReadEndpoint::Instruments => "Instruments",
            ReadEndpoint::Orderbook => "Orderbook",
            ReadEndpoint::PriceLimits => "PriceLimits",
            ReadEndpoint::CommissionRate => "CommissionRate",
            ReadEndpoint::IndicatorQuotes => "IndicatorQuotes",
            ReadEndpoint::IndicatorCandles => "IndicatorCandles",
            ReadEndpoint::Rankings => "Rankings",
            ReadEndpoint::StockInvestorFlow => "StockInvestorFlow",
            ReadEndpoint::IndicatorInvestorFlow => "IndicatorInvestorFlow",
            ReadEndpoint::SectorComposition => "SectorComposition",
            ReadEndpoint::Calendar => "Calendar",
            ReadEndpoint::InvestorTradingRankings => "InvestorTradingRankings",
            ReadEndpoint::Sectors => "Sectors",
            ReadEndpoint::Sector => "Sector",
            ReadEndpoint::SectorRankings => "SectorRankings",
            ReadEndpoint::SectorEtfs => "SectorEtfs",
            ReadEndpoint::ProgramTrades => "ProgramTrades",
            ReadEndpoint::ShortSelling => "ShortSelling",
            ReadEndpoint::CreditTrades => "CreditTrades",
            ReadEndpoint::SecuritiesLending => "SecuritiesLending",
        }),
    );
}
pub(super) fn get_read_endpoint(fields: &mut Fields) -> rusqlite::Result<ReadEndpoint> {
    match fields.take()?.as_str() {
        "Accounts" => Ok(ReadEndpoint::Accounts),
        "Holdings" => Ok(ReadEndpoint::Holdings),
        "Transactions" => Ok(ReadEndpoint::Transactions),
        "Quotes" => Ok(ReadEndpoint::Quotes),
        "Candles" => Ok(ReadEndpoint::Candles),
        "BuyingPower" => Ok(ReadEndpoint::BuyingPower),
        "FxRate" => Ok(ReadEndpoint::FxRate),
        "OrderFxRates" => Ok(ReadEndpoint::OrderFxRates),
        "SellableQuantity" => Ok(ReadEndpoint::SellableQuantity),
        "OpenOrders" => Ok(ReadEndpoint::OpenOrders),
        "ClosedOrders" => Ok(ReadEndpoint::ClosedOrders),
        "Order" => Ok(ReadEndpoint::Order),
        "StockWarnings" => Ok(ReadEndpoint::StockWarnings),
        "Instruments" => Ok(ReadEndpoint::Instruments),
        "Orderbook" => Ok(ReadEndpoint::Orderbook),
        "PriceLimits" => Ok(ReadEndpoint::PriceLimits),
        "CommissionRate" => Ok(ReadEndpoint::CommissionRate),
        "IndicatorQuotes" => Ok(ReadEndpoint::IndicatorQuotes),
        "IndicatorCandles" => Ok(ReadEndpoint::IndicatorCandles),
        "Rankings" => Ok(ReadEndpoint::Rankings),
        "StockInvestorFlow" => Ok(ReadEndpoint::StockInvestorFlow),
        "IndicatorInvestorFlow" => Ok(ReadEndpoint::IndicatorInvestorFlow),
        "SectorComposition" => Ok(ReadEndpoint::SectorComposition),
        "Calendar" => Ok(ReadEndpoint::Calendar),
        "InvestorTradingRankings" => Ok(ReadEndpoint::InvestorTradingRankings),
        "Sectors" => Ok(ReadEndpoint::Sectors),
        "Sector" => Ok(ReadEndpoint::Sector),
        "SectorRankings" => Ok(ReadEndpoint::SectorRankings),
        "SectorEtfs" => Ok(ReadEndpoint::SectorEtfs),
        "ProgramTrades" => Ok(ReadEndpoint::ProgramTrades),
        "ShortSelling" => Ok(ReadEndpoint::ShortSelling),
        "CreditTrades" => Ok(ReadEndpoint::CreditTrades),
        "SecuritiesLending" => Ok(ReadEndpoint::SecuritiesLending),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_http_status(fields: &mut Fields, value: HttpStatus) {
    fields.push(
        &(match value {
            HttpStatus::NotReceived => "NotReceived",
            HttpStatus::Informational => "Informational",
            HttpStatus::Success => "Success",
            HttpStatus::Redirect => "Redirect",
            HttpStatus::BadRequest => "BadRequest",
            HttpStatus::Unauthorized => "Unauthorized",
            HttpStatus::Forbidden => "Forbidden",
            HttpStatus::NotFound => "NotFound",
            HttpStatus::Conflict => "Conflict",
            HttpStatus::UriTooLong => "UriTooLong",
            HttpStatus::UnsupportedMediaType => "UnsupportedMediaType",
            HttpStatus::Unprocessable => "Unprocessable",
            HttpStatus::RateLimited => "RateLimited",
            HttpStatus::OtherClientError => "OtherClientError",
            HttpStatus::ServerError => "ServerError",
        }),
    );
}
pub(super) fn get_http_status(fields: &mut Fields) -> rusqlite::Result<HttpStatus> {
    match fields.take()?.as_str() {
        "NotReceived" => Ok(HttpStatus::NotReceived),
        "Informational" => Ok(HttpStatus::Informational),
        "Success" => Ok(HttpStatus::Success),
        "Redirect" => Ok(HttpStatus::Redirect),
        "BadRequest" => Ok(HttpStatus::BadRequest),
        "Unauthorized" => Ok(HttpStatus::Unauthorized),
        "Forbidden" => Ok(HttpStatus::Forbidden),
        "NotFound" => Ok(HttpStatus::NotFound),
        "Conflict" => Ok(HttpStatus::Conflict),
        "UriTooLong" => Ok(HttpStatus::UriTooLong),
        "UnsupportedMediaType" => Ok(HttpStatus::UnsupportedMediaType),
        "Unprocessable" => Ok(HttpStatus::Unprocessable),
        "RateLimited" => Ok(HttpStatus::RateLimited),
        "OtherClientError" => Ok(HttpStatus::OtherClientError),
        "ServerError" => Ok(HttpStatus::ServerError),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_access_denial(fields: &mut Fields, value: AccessDenial) {
    fields.push(
        &(match value {
            AccessDenial::InvalidCredentials => "InvalidCredentials",
            AccessDenial::IpNotAllowed => "IpNotAllowed",
            AccessDenial::TokenRevokedRepeated => "TokenRevokedRepeated",
            AccessDenial::AccountNotFound => "AccountNotFound",
            AccessDenial::Forbidden => "Forbidden",
            AccessDenial::LoginUserNotFound => "LoginUserNotFound",
        }),
    );
}
pub(super) fn get_access_denial(fields: &mut Fields) -> rusqlite::Result<AccessDenial> {
    match fields.take()?.as_str() {
        "InvalidCredentials" => Ok(AccessDenial::InvalidCredentials),
        "IpNotAllowed" => Ok(AccessDenial::IpNotAllowed),
        "TokenRevokedRepeated" => Ok(AccessDenial::TokenRevokedRepeated),
        "AccountNotFound" => Ok(AccessDenial::AccountNotFound),
        "Forbidden" => Ok(AccessDenial::Forbidden),
        "LoginUserNotFound" => Ok(AccessDenial::LoginUserNotFound),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_unavailable_cause(fields: &mut Fields, value: UnavailableCause) {
    fields.push(
        &(match value {
            UnavailableCause::RateLimited => "RateLimited",
            UnavailableCause::Maintenance => "Maintenance",
            UnavailableCause::ServerError => "ServerError",
            UnavailableCause::Network => "Network",
            UnavailableCause::ExpiredToken => "ExpiredToken",
        }),
    );
}
pub(super) fn get_unavailable_cause(fields: &mut Fields) -> rusqlite::Result<UnavailableCause> {
    match fields.take()?.as_str() {
        "RateLimited" => Ok(UnavailableCause::RateLimited),
        "Maintenance" => Ok(UnavailableCause::Maintenance),
        "ServerError" => Ok(UnavailableCause::ServerError),
        "Network" => Ok(UnavailableCause::Network),
        "ExpiredToken" => Ok(UnavailableCause::ExpiredToken),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_missing_data(fields: &mut Fields, value: MissingData) {
    fields.push(
        &(match value {
            MissingData::Order => "Order",
            MissingData::Stock => "Stock",
            MissingData::ExchangeRate => "ExchangeRate",
            MissingData::Sector => "Sector",
        }),
    );
}
pub(super) fn get_missing_data(fields: &mut Fields) -> rusqlite::Result<MissingData> {
    match fields.take()?.as_str() {
        "Order" => Ok(MissingData::Order),
        "Stock" => Ok(MissingData::Stock),
        "ExchangeRate" => Ok(MissingData::ExchangeRate),
        "Sector" => Ok(MissingData::Sector),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_response_mismatch(fields: &mut Fields, value: ResponseMismatch) {
    fields.push(
        &(match value {
            ResponseMismatch::Schema => "Schema",
            ResponseMismatch::Validation => "Validation",
        }),
    );
}
pub(super) fn get_response_mismatch(fields: &mut Fields) -> rusqlite::Result<ResponseMismatch> {
    match fields.take()?.as_str() {
        "Schema" => Ok(ResponseMismatch::Schema),
        "Validation" => Ok(ResponseMismatch::Validation),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_request_rejection(fields: &mut Fields, value: RequestRejection) {
    fields.push(
        &(match value {
            RequestRejection::MalformedRequest => "MalformedRequest",
            RequestRejection::MissingAccountHeader => "MissingAccountHeader",
            RequestRejection::InvalidToken => "InvalidToken",
            RequestRejection::EdgeBlocked => "EdgeBlocked",
            RequestRejection::UnsupportedQuery => "UnsupportedQuery",
            RequestRejection::UnsupportedContentType => "UnsupportedContentType",
            RequestRejection::BatchLimitExceeded => "BatchLimitExceeded",
        }),
    );
}
pub(super) fn get_request_rejection(fields: &mut Fields) -> rusqlite::Result<RequestRejection> {
    match fields.take()?.as_str() {
        "MalformedRequest" => Ok(RequestRejection::MalformedRequest),
        "MissingAccountHeader" => Ok(RequestRejection::MissingAccountHeader),
        "InvalidToken" => Ok(RequestRejection::InvalidToken),
        "EdgeBlocked" => Ok(RequestRejection::EdgeBlocked),
        "UnsupportedQuery" => Ok(RequestRejection::UnsupportedQuery),
        "UnsupportedContentType" => Ok(RequestRejection::UnsupportedContentType),
        "BatchLimitExceeded" => Ok(RequestRejection::BatchLimitExceeded),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_broker_capability(fields: &mut Fields, value: BrokerCapability) {
    fields.push(
        &(match value {
            BrokerCapability::DailyCandles => "DailyCandles",
            BrokerCapability::MinuteCandles => "MinuteCandles",
            BrokerCapability::AdjustedHistory => "AdjustedHistory",
            BrokerCapability::RawHistory => "RawHistory",
            BrokerCapability::SellableQuantity => "SellableQuantity",
            BrokerCapability::OpenOrders => "OpenOrders",
            BrokerCapability::ClosedOrders => "ClosedOrders",
            BrokerCapability::Order => "Order",
            BrokerCapability::StockWarnings => "StockWarnings",
            BrokerCapability::Instruments => "Instruments",
            BrokerCapability::Orderbook => "Orderbook",
            BrokerCapability::PriceLimits => "PriceLimits",
            BrokerCapability::CommissionRate => "CommissionRate",
            BrokerCapability::IndicatorQuotes => "IndicatorQuotes",
            BrokerCapability::IndicatorCandles => "IndicatorCandles",
            BrokerCapability::Rankings => "Rankings",
            BrokerCapability::StockInvestorFlow => "StockInvestorFlow",
            BrokerCapability::IndicatorInvestorFlow => "IndicatorInvestorFlow",
            BrokerCapability::SectorComposition => "SectorComposition",
            BrokerCapability::Calendar => "Calendar",
            BrokerCapability::InvestorTradingRankings => "InvestorTradingRankings",
            BrokerCapability::Sectors => "Sectors",
            BrokerCapability::Sector => "Sector",
            BrokerCapability::SectorRankings => "SectorRankings",
            BrokerCapability::SectorEtfs => "SectorEtfs",
            BrokerCapability::ProgramTrades => "ProgramTrades",
            BrokerCapability::ShortSelling => "ShortSelling",
            BrokerCapability::CreditTrades => "CreditTrades",
            BrokerCapability::SecuritiesLending => "SecuritiesLending",
        }),
    );
}
pub(super) fn get_broker_capability(fields: &mut Fields) -> rusqlite::Result<BrokerCapability> {
    match fields.take()?.as_str() {
        "DailyCandles" => Ok(BrokerCapability::DailyCandles),
        "MinuteCandles" => Ok(BrokerCapability::MinuteCandles),
        "AdjustedHistory" => Ok(BrokerCapability::AdjustedHistory),
        "RawHistory" => Ok(BrokerCapability::RawHistory),
        "SellableQuantity" => Ok(BrokerCapability::SellableQuantity),
        "OpenOrders" => Ok(BrokerCapability::OpenOrders),
        "ClosedOrders" => Ok(BrokerCapability::ClosedOrders),
        "Order" => Ok(BrokerCapability::Order),
        "StockWarnings" => Ok(BrokerCapability::StockWarnings),
        "Instruments" => Ok(BrokerCapability::Instruments),
        "Orderbook" => Ok(BrokerCapability::Orderbook),
        "PriceLimits" => Ok(BrokerCapability::PriceLimits),
        "CommissionRate" => Ok(BrokerCapability::CommissionRate),
        "IndicatorQuotes" => Ok(BrokerCapability::IndicatorQuotes),
        "IndicatorCandles" => Ok(BrokerCapability::IndicatorCandles),
        "Rankings" => Ok(BrokerCapability::Rankings),
        "StockInvestorFlow" => Ok(BrokerCapability::StockInvestorFlow),
        "IndicatorInvestorFlow" => Ok(BrokerCapability::IndicatorInvestorFlow),
        "SectorComposition" => Ok(BrokerCapability::SectorComposition),
        "Calendar" => Ok(BrokerCapability::Calendar),
        "InvestorTradingRankings" => Ok(BrokerCapability::InvestorTradingRankings),
        "Sectors" => Ok(BrokerCapability::Sectors),
        "Sector" => Ok(BrokerCapability::Sector),
        "SectorRankings" => Ok(BrokerCapability::SectorRankings),
        "SectorEtfs" => Ok(BrokerCapability::SectorEtfs),
        "ProgramTrades" => Ok(BrokerCapability::ProgramTrades),
        "ShortSelling" => Ok(BrokerCapability::ShortSelling),
        "CreditTrades" => Ok(BrokerCapability::CreditTrades),
        "SecuritiesLending" => Ok(BrokerCapability::SecuritiesLending),
        _ => Err(invalid(())),
    }
}
pub(super) fn put_incomplete_reason(fields: &mut Fields, value: IncompleteReason) {
    fields.push(
        &(match value {
            IncompleteReason::QuarantinedRows => "QuarantinedRows",
            IncompleteReason::RowLimitExceeded => "RowLimitExceeded",
            IncompleteReason::PageInterrupted => "PageInterrupted",
        }),
    );
}
pub(super) fn get_incomplete_reason(fields: &mut Fields) -> rusqlite::Result<IncompleteReason> {
    match fields.take()?.as_str() {
        "QuarantinedRows" => Ok(IncompleteReason::QuarantinedRows),
        "RowLimitExceeded" => Ok(IncompleteReason::RowLimitExceeded),
        "PageInterrupted" => Ok(IncompleteReason::PageInterrupted),
        _ => Err(invalid(())),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccessDenial, BrokerCapability, Fields, HttpStatus, IncompleteReason, MissingData,
        ReadEndpoint, RequestRejection, ResponseMismatch, UnavailableCause, get_access_denial,
        get_broker_capability, get_http_status, get_incomplete_reason, get_missing_data,
        get_read_endpoint, get_request_rejection, get_response_mismatch, get_unavailable_cause,
        put_access_denial, put_broker_capability, put_http_status, put_incomplete_reason,
        put_missing_data, put_read_endpoint, put_request_rejection, put_response_mismatch,
        put_unavailable_cause,
    };
    #[test]
    fn read_endpoint_roundtrips_and_rejects_unknown() {
        for value in [
            ReadEndpoint::Accounts,
            ReadEndpoint::Holdings,
            ReadEndpoint::Transactions,
            ReadEndpoint::Quotes,
            ReadEndpoint::Candles,
            ReadEndpoint::BuyingPower,
            ReadEndpoint::FxRate,
            ReadEndpoint::OrderFxRates,
            ReadEndpoint::SellableQuantity,
            ReadEndpoint::OpenOrders,
            ReadEndpoint::ClosedOrders,
            ReadEndpoint::Order,
            ReadEndpoint::StockWarnings,
            ReadEndpoint::Instruments,
            ReadEndpoint::Orderbook,
            ReadEndpoint::PriceLimits,
            ReadEndpoint::CommissionRate,
            ReadEndpoint::IndicatorQuotes,
            ReadEndpoint::IndicatorCandles,
            ReadEndpoint::Rankings,
            ReadEndpoint::StockInvestorFlow,
            ReadEndpoint::IndicatorInvestorFlow,
            ReadEndpoint::SectorComposition,
            ReadEndpoint::Calendar,
            ReadEndpoint::InvestorTradingRankings,
            ReadEndpoint::Sectors,
            ReadEndpoint::Sector,
            ReadEndpoint::SectorRankings,
            ReadEndpoint::SectorEtfs,
            ReadEndpoint::ProgramTrades,
            ReadEndpoint::ShortSelling,
            ReadEndpoint::CreditTrades,
            ReadEndpoint::SecuritiesLending,
        ] {
            let mut fields = Fields::default();
            put_read_endpoint(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_read_endpoint(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_read_endpoint(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn http_status_roundtrips_and_rejects_unknown() {
        for value in [
            HttpStatus::NotReceived,
            HttpStatus::Informational,
            HttpStatus::Success,
            HttpStatus::Redirect,
            HttpStatus::BadRequest,
            HttpStatus::Unauthorized,
            HttpStatus::Forbidden,
            HttpStatus::NotFound,
            HttpStatus::Conflict,
            HttpStatus::UriTooLong,
            HttpStatus::UnsupportedMediaType,
            HttpStatus::Unprocessable,
            HttpStatus::RateLimited,
            HttpStatus::OtherClientError,
            HttpStatus::ServerError,
        ] {
            let mut fields = Fields::default();
            put_http_status(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_http_status(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_http_status(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn access_denial_roundtrips_and_rejects_unknown() {
        for value in [
            AccessDenial::InvalidCredentials,
            AccessDenial::IpNotAllowed,
            AccessDenial::TokenRevokedRepeated,
            AccessDenial::AccountNotFound,
            AccessDenial::Forbidden,
            AccessDenial::LoginUserNotFound,
        ] {
            let mut fields = Fields::default();
            put_access_denial(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_access_denial(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_access_denial(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn unavailable_cause_roundtrips_and_rejects_unknown() {
        for value in [
            UnavailableCause::RateLimited,
            UnavailableCause::Maintenance,
            UnavailableCause::ServerError,
            UnavailableCause::Network,
            UnavailableCause::ExpiredToken,
        ] {
            let mut fields = Fields::default();
            put_unavailable_cause(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_unavailable_cause(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_unavailable_cause(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn missing_data_roundtrips_and_rejects_unknown() {
        for value in [
            MissingData::Order,
            MissingData::Stock,
            MissingData::ExchangeRate,
            MissingData::Sector,
        ] {
            let mut fields = Fields::default();
            put_missing_data(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_missing_data(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_missing_data(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn response_mismatch_roundtrips_and_rejects_unknown() {
        for value in [ResponseMismatch::Schema, ResponseMismatch::Validation] {
            let mut fields = Fields::default();
            put_response_mismatch(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_response_mismatch(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_response_mismatch(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn request_rejection_roundtrips_and_rejects_unknown() {
        for value in [
            RequestRejection::MalformedRequest,
            RequestRejection::MissingAccountHeader,
            RequestRejection::InvalidToken,
            RequestRejection::EdgeBlocked,
            RequestRejection::UnsupportedQuery,
            RequestRejection::UnsupportedContentType,
            RequestRejection::BatchLimitExceeded,
        ] {
            let mut fields = Fields::default();
            put_request_rejection(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_request_rejection(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_request_rejection(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn broker_capability_roundtrips_and_rejects_unknown() {
        for value in [
            BrokerCapability::DailyCandles,
            BrokerCapability::MinuteCandles,
            BrokerCapability::AdjustedHistory,
            BrokerCapability::RawHistory,
            BrokerCapability::SellableQuantity,
            BrokerCapability::OpenOrders,
            BrokerCapability::ClosedOrders,
            BrokerCapability::Order,
            BrokerCapability::StockWarnings,
            BrokerCapability::Instruments,
            BrokerCapability::Orderbook,
            BrokerCapability::PriceLimits,
            BrokerCapability::CommissionRate,
            BrokerCapability::IndicatorQuotes,
            BrokerCapability::IndicatorCandles,
            BrokerCapability::Rankings,
            BrokerCapability::StockInvestorFlow,
            BrokerCapability::IndicatorInvestorFlow,
            BrokerCapability::SectorComposition,
            BrokerCapability::Calendar,
            BrokerCapability::InvestorTradingRankings,
            BrokerCapability::Sectors,
            BrokerCapability::Sector,
            BrokerCapability::SectorRankings,
            BrokerCapability::SectorEtfs,
            BrokerCapability::ProgramTrades,
            BrokerCapability::ShortSelling,
            BrokerCapability::CreditTrades,
            BrokerCapability::SecuritiesLending,
        ] {
            let mut fields = Fields::default();
            put_broker_capability(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_broker_capability(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_broker_capability(&mut invalid).expect_err("알 수 없는 태그");
    }
    #[test]
    fn incomplete_reason_roundtrips_and_rejects_unknown() {
        for value in [
            IncompleteReason::QuarantinedRows,
            IncompleteReason::RowLimitExceeded,
            IncompleteReason::PageInterrupted,
        ] {
            let mut fields = Fields::default();
            put_incomplete_reason(&mut fields, value);
            let mut decoded = Fields::decode(&fields.encode()).expect("복원");
            assert_eq!(get_incomplete_reason(&mut decoded).expect("태그"), value);
            decoded.finish().expect("끝");
        }
        let mut invalid = Fields::default();
        invalid.push("future");
        get_incomplete_reason(&mut invalid).expect_err("알 수 없는 태그");
    }
}
