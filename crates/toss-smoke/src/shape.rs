use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    Token,
    Accounts,
    Holdings,
    BuyingPower,
    Transactions,
    ExchangeRate,
    CalendarKr,
    CalendarUs,
    IndicatorQuotes,
    Sectors,
    Quotes,
    Candles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumOpenness {
    Closed,
    Open,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub name: &'static str,
    pub schema: &'static Schema,
}

#[derive(Debug, Clone, Copy)]
pub enum Schema {
    Object {
        required: &'static [FieldSpec],
        optional: &'static [FieldSpec],
    },
    Array {
        item: &'static Self,
    },
    String,
    NonEmptyString,
    DecimalString,
    Integer,
    MinInteger(i64),
    Boolean,
    Enum {
        label: &'static str,
        values: &'static [&'static str],
        open: EnumOpenness,
    },
    Nullable(&'static Self),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldShape {
    pub name: &'static str,
    pub kind: &'static str,
    pub count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecMatch {
    Matched,
    Mismatched(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeSummary {
    pub fields: Vec<FieldShape>,
    pub item_count: Option<usize>,
    pub known_enums: BTreeMap<&'static str, BTreeSet<&'static str>>,
    pub unknown_enum_count: usize,
    pub extra_field_count: usize,
    pub spec_match: SpecMatch,
}

const TOKEN_TYPES: [&str; 1] = ["Bearer"];
const ACCOUNT_TYPES: [&str; 4] = [
    "BROKERAGE",
    "OVERSEAS_DERIVATIVES",
    "PENSION_SAVINGS",
    "RESHORING_INVESTMENT",
];
const CURRENCIES: [&str; 2] = ["KRW", "USD"];
const MARKET_COUNTRIES: [&str; 2] = ["KR", "US"];
const TRANSACTION_TYPES: [&str; 11] = [
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
];
const ASSET_CLASSES: [&str; 3] = ["STOCK", "BOND", "OPTION"];
const RATE_CHANGES: [&str; 3] = ["UP", "EQUAL", "DOWN"];

const S_STRING: Schema = Schema::String;
const S_NON_EMPTY: Schema = Schema::NonEmptyString;
const S_DECIMAL: Schema = Schema::DecimalString;
const S_INTEGER: Schema = Schema::Integer;
const S_BOOL: Schema = Schema::Boolean;
const S_NULLABLE_STRING: Schema = Schema::Nullable(&S_STRING);
const S_NULLABLE_DECIMAL: Schema = Schema::Nullable(&S_DECIMAL);
const S_TOKEN_TYPE: Schema = Schema::Enum {
    label: "token_type",
    values: &TOKEN_TYPES,
    open: EnumOpenness::Closed,
};
const S_EXPIRES_IN: Schema = Schema::MinInteger(2);
const S_ACCOUNT_TYPE: Schema = Schema::Enum {
    label: "accountType",
    values: &ACCOUNT_TYPES,
    open: EnumOpenness::Open,
};
const S_CURRENCY: Schema = Schema::Enum {
    label: "currency",
    values: &CURRENCIES,
    open: EnumOpenness::Open,
};
const S_MARKET_COUNTRY: Schema = Schema::Enum {
    label: "marketCountry",
    values: &MARKET_COUNTRIES,
    open: EnumOpenness::Open,
};
const S_TRANSACTION_TYPE: Schema = Schema::Enum {
    label: "transactionType",
    values: &TRANSACTION_TYPES,
    open: EnumOpenness::Open,
};
const S_ASSET_CLASS: Schema = Schema::Enum {
    label: "assetClass",
    values: &ASSET_CLASSES,
    open: EnumOpenness::Open,
};
const S_NULLABLE_ASSET_CLASS: Schema = Schema::Nullable(&S_ASSET_CLASS);
const S_RATE_CHANGE: Schema = Schema::Enum {
    label: "rateChangeType",
    values: &RATE_CHANGES,
    open: EnumOpenness::Open,
};

const TOKEN_FIELDS: [FieldSpec; 3] = [
    FieldSpec {
        name: "access_token",
        schema: &S_NON_EMPTY,
    },
    FieldSpec {
        name: "token_type",
        schema: &S_TOKEN_TYPE,
    },
    FieldSpec {
        name: "expires_in",
        schema: &S_EXPIRES_IN,
    },
];
const TOKEN_SCHEMA: Schema = Schema::Object {
    required: &TOKEN_FIELDS,
    optional: &[],
};

const ACCOUNT_FIELDS: [FieldSpec; 3] = [
    FieldSpec {
        name: "accountNo",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "accountSeq",
        schema: &S_INTEGER,
    },
    FieldSpec {
        name: "accountType",
        schema: &S_ACCOUNT_TYPE,
    },
];
const ACCOUNT_SCHEMA: Schema = Schema::Object {
    required: &ACCOUNT_FIELDS,
    optional: &[],
};
const ACCOUNTS_RESULT: Schema = Schema::Array {
    item: &ACCOUNT_SCHEMA,
};

const PRICE_REQ: [FieldSpec; 1] = [FieldSpec {
    name: "krw",
    schema: &S_DECIMAL,
}];
const PRICE_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "usd",
    schema: &S_NULLABLE_DECIMAL,
}];
const PRICE_SCHEMA: Schema = Schema::Object {
    required: &PRICE_REQ,
    optional: &PRICE_OPT,
};

const OVERVIEW_MV_REQ: [FieldSpec; 2] = [
    FieldSpec {
        name: "amount",
        schema: &PRICE_SCHEMA,
    },
    FieldSpec {
        name: "amountAfterCost",
        schema: &PRICE_SCHEMA,
    },
];
const OVERVIEW_MV_SCHEMA: Schema = Schema::Object {
    required: &OVERVIEW_MV_REQ,
    optional: &[],
};

const OVERVIEW_PL_REQ: [FieldSpec; 4] = [
    FieldSpec {
        name: "amount",
        schema: &PRICE_SCHEMA,
    },
    FieldSpec {
        name: "amountAfterCost",
        schema: &PRICE_SCHEMA,
    },
    FieldSpec {
        name: "rate",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "rateAfterCost",
        schema: &S_DECIMAL,
    },
];
const OVERVIEW_PL_SCHEMA: Schema = Schema::Object {
    required: &OVERVIEW_PL_REQ,
    optional: &[],
};

const OVERVIEW_DAILY_REQ: [FieldSpec; 2] = [
    FieldSpec {
        name: "amount",
        schema: &PRICE_SCHEMA,
    },
    FieldSpec {
        name: "rate",
        schema: &S_DECIMAL,
    },
];
const OVERVIEW_DAILY_SCHEMA: Schema = Schema::Object {
    required: &OVERVIEW_DAILY_REQ,
    optional: &[],
};

const ITEM_MV_REQ: [FieldSpec; 3] = [
    FieldSpec {
        name: "purchaseAmount",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "amount",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "amountAfterCost",
        schema: &S_DECIMAL,
    },
];
const ITEM_MV_SCHEMA: Schema = Schema::Object {
    required: &ITEM_MV_REQ,
    optional: &[],
};

const ITEM_PL_REQ: [FieldSpec; 4] = [
    FieldSpec {
        name: "amount",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "amountAfterCost",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "rate",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "rateAfterCost",
        schema: &S_DECIMAL,
    },
];
const ITEM_PL_SCHEMA: Schema = Schema::Object {
    required: &ITEM_PL_REQ,
    optional: &[],
};

const ITEM_DAILY_REQ: [FieldSpec; 2] = [
    FieldSpec {
        name: "amount",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "rate",
        schema: &S_DECIMAL,
    },
];
const ITEM_DAILY_SCHEMA: Schema = Schema::Object {
    required: &ITEM_DAILY_REQ,
    optional: &[],
};

const ITEM_COST_REQ: [FieldSpec; 1] = [FieldSpec {
    name: "commission",
    schema: &S_DECIMAL,
}];
const ITEM_COST_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "tax",
    schema: &S_NULLABLE_DECIMAL,
}];
const ITEM_COST_SCHEMA: Schema = Schema::Object {
    required: &ITEM_COST_REQ,
    optional: &ITEM_COST_OPT,
};

const HOLDING_ITEM_REQ: [FieldSpec; 11] = [
    FieldSpec {
        name: "symbol",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "name",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "marketCountry",
        schema: &S_MARKET_COUNTRY,
    },
    FieldSpec {
        name: "currency",
        schema: &S_CURRENCY,
    },
    FieldSpec {
        name: "quantity",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "lastPrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "averagePurchasePrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "marketValue",
        schema: &ITEM_MV_SCHEMA,
    },
    FieldSpec {
        name: "profitLoss",
        schema: &ITEM_PL_SCHEMA,
    },
    FieldSpec {
        name: "dailyProfitLoss",
        schema: &ITEM_DAILY_SCHEMA,
    },
    FieldSpec {
        name: "cost",
        schema: &ITEM_COST_SCHEMA,
    },
];
const HOLDING_ITEM_SCHEMA: Schema = Schema::Object {
    required: &HOLDING_ITEM_REQ,
    optional: &[],
};
const HOLDING_ITEMS_SCHEMA: Schema = Schema::Array {
    item: &HOLDING_ITEM_SCHEMA,
};

const HOLDINGS_REQ: [FieldSpec; 5] = [
    FieldSpec {
        name: "totalPurchaseAmount",
        schema: &PRICE_SCHEMA,
    },
    FieldSpec {
        name: "marketValue",
        schema: &OVERVIEW_MV_SCHEMA,
    },
    FieldSpec {
        name: "profitLoss",
        schema: &OVERVIEW_PL_SCHEMA,
    },
    FieldSpec {
        name: "dailyProfitLoss",
        schema: &OVERVIEW_DAILY_SCHEMA,
    },
    FieldSpec {
        name: "items",
        schema: &HOLDING_ITEMS_SCHEMA,
    },
];
const HOLDINGS_RESULT: Schema = Schema::Object {
    required: &HOLDINGS_REQ,
    optional: &[],
};

const BUYING_POWER_REQ: [FieldSpec; 2] = [
    FieldSpec {
        name: "currency",
        schema: &S_CURRENCY,
    },
    FieldSpec {
        name: "cashBuyingPower",
        schema: &S_DECIMAL,
    },
];
const BUYING_POWER_RESULT: Schema = Schema::Object {
    required: &BUYING_POWER_REQ,
    optional: &[],
};

const TRANSACTION_REQ: [FieldSpec; 17] = [
    FieldSpec {
        name: "transactionType",
        schema: &S_TRANSACTION_TYPE,
    },
    FieldSpec {
        name: "marketCountry",
        schema: &S_MARKET_COUNTRY,
    },
    FieldSpec {
        name: "currency",
        schema: &S_CURRENCY,
    },
    FieldSpec {
        name: "symbol",
        schema: &S_NULLABLE_STRING,
    },
    FieldSpec {
        name: "assetClass",
        schema: &S_NULLABLE_ASSET_CLASS,
    },
    FieldSpec {
        name: "isinCode",
        schema: &S_NULLABLE_STRING,
    },
    FieldSpec {
        name: "quantity",
        schema: &S_NULLABLE_DECIMAL,
    },
    FieldSpec {
        name: "filledPrice",
        schema: &S_NULLABLE_DECIMAL,
    },
    FieldSpec {
        name: "amount",
        schema: &S_NULLABLE_DECIMAL,
    },
    FieldSpec {
        name: "commission",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "tax",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "netAmount",
        schema: &S_NULLABLE_DECIMAL,
    },
    FieldSpec {
        name: "exchangeRate",
        schema: &S_NULLABLE_DECIMAL,
    },
    FieldSpec {
        name: "settlementDate",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "settled",
        schema: &S_BOOL,
    },
    FieldSpec {
        name: "transactionTime",
        schema: &S_NULLABLE_STRING,
    },
    FieldSpec {
        name: "isCancellation",
        schema: &S_BOOL,
    },
];
const TRANSACTION_SCHEMA: Schema = Schema::Object {
    required: &TRANSACTION_REQ,
    optional: &[],
};
const TRANSACTION_ARRAY: Schema = Schema::Array {
    item: &TRANSACTION_SCHEMA,
};
const TRANSACTIONS_REQ: [FieldSpec; 3] = [
    FieldSpec {
        name: "transactions",
        schema: &TRANSACTION_ARRAY,
    },
    FieldSpec {
        name: "nextCursor",
        schema: &S_NULLABLE_STRING,
    },
    FieldSpec {
        name: "hasNext",
        schema: &S_BOOL,
    },
];
const TRANSACTIONS_RESULT: Schema = Schema::Object {
    required: &TRANSACTIONS_REQ,
    optional: &[],
};

const EXCHANGE_RATE_REQ: [FieldSpec; 8] = [
    FieldSpec {
        name: "baseCurrency",
        schema: &S_CURRENCY,
    },
    FieldSpec {
        name: "quoteCurrency",
        schema: &S_CURRENCY,
    },
    FieldSpec {
        name: "rate",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "midRate",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "basisPoint",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "rateChangeType",
        schema: &S_RATE_CHANGE,
    },
    FieldSpec {
        name: "validFrom",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "validUntil",
        schema: &S_STRING,
    },
];
const EXCHANGE_RATE_RESULT: Schema = Schema::Object {
    required: &EXCHANGE_RATE_REQ,
    optional: &[],
};

const SESSION_REQ: [FieldSpec; 2] = [
    FieldSpec {
        name: "startTime",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "endTime",
        schema: &S_STRING,
    },
];
const KR_PRE_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "singlePriceAuctionStartTime",
    schema: &S_NULLABLE_STRING,
}];
const KR_AFTER_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "singlePriceAuctionEndTime",
    schema: &S_NULLABLE_STRING,
}];
const KR_PRE_SCHEMA: Schema = Schema::Object {
    required: &SESSION_REQ,
    optional: &KR_PRE_OPT,
};
const KR_AFTER_SCHEMA: Schema = Schema::Object {
    required: &SESSION_REQ,
    optional: &KR_AFTER_OPT,
};
const NULLABLE_KR_PRE: Schema = Schema::Nullable(&KR_PRE_SCHEMA);
const NULLABLE_KR_AFTER: Schema = Schema::Nullable(&KR_AFTER_SCHEMA);
const INTEGRATED_OPT: [FieldSpec; 3] = [
    FieldSpec {
        name: "preMarket",
        schema: &NULLABLE_KR_PRE,
    },
    FieldSpec {
        name: "regularMarket",
        schema: &NULLABLE_KR_PRE,
    },
    FieldSpec {
        name: "afterMarket",
        schema: &NULLABLE_KR_AFTER,
    },
];
const INTEGRATED_SCHEMA: Schema = Schema::Object {
    required: &[],
    optional: &INTEGRATED_OPT,
};
const NULLABLE_INTEGRATED: Schema = Schema::Nullable(&INTEGRATED_SCHEMA);
const KR_DAY_REQ: [FieldSpec; 1] = [FieldSpec {
    name: "date",
    schema: &S_STRING,
}];
const KR_DAY_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "integrated",
    schema: &NULLABLE_INTEGRATED,
}];
const KR_DAY_SCHEMA: Schema = Schema::Object {
    required: &KR_DAY_REQ,
    optional: &KR_DAY_OPT,
};
const CALENDAR_KR_REQ: [FieldSpec; 3] = [
    FieldSpec {
        name: "today",
        schema: &KR_DAY_SCHEMA,
    },
    FieldSpec {
        name: "previousBusinessDay",
        schema: &KR_DAY_SCHEMA,
    },
    FieldSpec {
        name: "nextBusinessDay",
        schema: &KR_DAY_SCHEMA,
    },
];
const CALENDAR_KR_RESULT: Schema = Schema::Object {
    required: &CALENDAR_KR_REQ,
    optional: &[],
};

const US_SESSION_SCHEMA: Schema = Schema::Object {
    required: &SESSION_REQ,
    optional: &[],
};
const NULLABLE_US_SESSION: Schema = Schema::Nullable(&US_SESSION_SCHEMA);
const US_DAY_OPT: [FieldSpec; 4] = [
    FieldSpec {
        name: "dayMarket",
        schema: &NULLABLE_US_SESSION,
    },
    FieldSpec {
        name: "preMarket",
        schema: &NULLABLE_US_SESSION,
    },
    FieldSpec {
        name: "regularMarket",
        schema: &NULLABLE_US_SESSION,
    },
    FieldSpec {
        name: "afterMarket",
        schema: &NULLABLE_US_SESSION,
    },
];
const US_DAY_SCHEMA: Schema = Schema::Object {
    required: &KR_DAY_REQ,
    optional: &US_DAY_OPT,
};
const CALENDAR_US_REQ: [FieldSpec; 3] = [
    FieldSpec {
        name: "today",
        schema: &US_DAY_SCHEMA,
    },
    FieldSpec {
        name: "previousBusinessDay",
        schema: &US_DAY_SCHEMA,
    },
    FieldSpec {
        name: "nextBusinessDay",
        schema: &US_DAY_SCHEMA,
    },
];
const CALENDAR_US_RESULT: Schema = Schema::Object {
    required: &CALENDAR_US_REQ,
    optional: &[],
};

const INDICATOR_REQ: [FieldSpec; 2] = [
    FieldSpec {
        name: "symbol",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "lastPrice",
        schema: &S_DECIMAL,
    },
];
const INDICATOR_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "timestamp",
    schema: &S_NULLABLE_STRING,
}];
const INDICATOR_SCHEMA: Schema = Schema::Object {
    required: &INDICATOR_REQ,
    optional: &INDICATOR_OPT,
};
const INDICATOR_QUOTES_RESULT: Schema = Schema::Array {
    item: &INDICATOR_SCHEMA,
};

const SECTOR_REQ: [FieldSpec; 4] = [
    FieldSpec {
        name: "sectorId",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "name",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "depth",
        schema: &S_INTEGER,
    },
    FieldSpec {
        name: "parentSectorId",
        schema: &S_NULLABLE_STRING,
    },
];
const SECTOR_SCHEMA: Schema = Schema::Object {
    required: &SECTOR_REQ,
    optional: &[],
};
const SECTORS_ARRAY: Schema = Schema::Array {
    item: &SECTOR_SCHEMA,
};
const SECTORS_REQ: [FieldSpec; 1] = [FieldSpec {
    name: "sectors",
    schema: &SECTORS_ARRAY,
}];
const SECTORS_RESULT: Schema = Schema::Object {
    required: &SECTORS_REQ,
    optional: &[],
};

const QUOTE_REQ: [FieldSpec; 3] = [
    FieldSpec {
        name: "symbol",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "lastPrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "currency",
        schema: &S_CURRENCY,
    },
];
const QUOTE_SCHEMA: Schema = Schema::Object {
    required: &QUOTE_REQ,
    optional: &INDICATOR_OPT,
};
const QUOTES_RESULT: Schema = Schema::Array {
    item: &QUOTE_SCHEMA,
};

const CANDLE_REQ: [FieldSpec; 7] = [
    FieldSpec {
        name: "timestamp",
        schema: &S_STRING,
    },
    FieldSpec {
        name: "openPrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "highPrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "lowPrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "closePrice",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "volume",
        schema: &S_DECIMAL,
    },
    FieldSpec {
        name: "currency",
        schema: &S_CURRENCY,
    },
];
const CANDLE_SCHEMA: Schema = Schema::Object {
    required: &CANDLE_REQ,
    optional: &[],
};
const CANDLES_ARRAY: Schema = Schema::Array {
    item: &CANDLE_SCHEMA,
};
const CANDLES_REQ: [FieldSpec; 1] = [FieldSpec {
    name: "candles",
    schema: &CANDLES_ARRAY,
}];
const CANDLES_OPT: [FieldSpec; 1] = [FieldSpec {
    name: "nextBefore",
    schema: &S_NULLABLE_STRING,
}];
const CANDLES_RESULT: Schema = Schema::Object {
    required: &CANDLES_REQ,
    optional: &CANDLES_OPT,
};

#[must_use]
pub const fn result_schema(kind: EndpointKind) -> &'static Schema {
    match kind {
        EndpointKind::Token => &TOKEN_SCHEMA,
        EndpointKind::Accounts => &ACCOUNTS_RESULT,
        EndpointKind::Holdings => &HOLDINGS_RESULT,
        EndpointKind::BuyingPower => &BUYING_POWER_RESULT,
        EndpointKind::Transactions => &TRANSACTIONS_RESULT,
        EndpointKind::ExchangeRate => &EXCHANGE_RATE_RESULT,
        EndpointKind::CalendarKr => &CALENDAR_KR_RESULT,
        EndpointKind::CalendarUs => &CALENDAR_US_RESULT,
        EndpointKind::IndicatorQuotes => &INDICATOR_QUOTES_RESULT,
        EndpointKind::Sectors => &SECTORS_RESULT,
        EndpointKind::Quotes => &QUOTES_RESULT,
        EndpointKind::Candles => &CANDLES_RESULT,
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn is_valid_decimal(text: &str) -> bool {
    if text.is_empty() || text.len() > 30 {
        return false;
    }
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    if unsigned.is_empty() {
        return false;
    }
    match unsigned.split_once('.') {
        Some((whole, frac)) => {
            !whole.is_empty()
                && !frac.is_empty()
                && whole.bytes().all(|byte| byte.is_ascii_digit())
                && frac.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => unsigned.bytes().all(|byte| byte.is_ascii_digit()),
    }
}

#[derive(Clone, Copy)]
struct EnumSpec {
    label: &'static str,
    values: &'static [&'static str],
    open: EnumOpenness,
}

struct Validator {
    mismatches: Vec<String>,
    known_enums: BTreeMap<&'static str, BTreeSet<&'static str>>,
    unknown_enum_count: usize,
    extra_field_count: usize,
}

impl Validator {
    fn new() -> Self {
        Self {
            mismatches: Vec::new(),
            known_enums: BTreeMap::new(),
            unknown_enum_count: 0,
            extra_field_count: 0,
        }
    }

    fn record_mismatch(&mut self, path: &str, reason: &str) {
        if self.mismatches.len() < 16 {
            self.mismatches.push(format!("{path}: {reason}"));
        }
    }

    fn check_object(
        &mut self,
        path: &str,
        object: &Map<String, Value>,
        required: &'static [FieldSpec],
        optional: &'static [FieldSpec],
    ) {
        for field in required {
            let child_path = format!("{path}.{}", field.name);
            match object.get(field.name) {
                Some(entry) => self.check(&child_path, entry, field.schema),
                None => self.record_mismatch(&child_path, "필수 필드 누락"),
            }
        }
        for field in optional {
            if let Some(entry) = object.get(field.name) {
                let child_path = format!("{path}.{}", field.name);
                self.check(&child_path, entry, field.schema);
            }
        }
        let extra = object
            .keys()
            .filter(|key| {
                !required.iter().any(|spec| spec.name == key.as_str())
                    && !optional.iter().any(|spec| spec.name == key.as_str())
            })
            .count();
        self.extra_field_count = self.extra_field_count.saturating_add(extra);
    }

    fn check_enum(&mut self, path: &str, value: &Value, spec: EnumSpec) {
        let Some(text) = value.as_str() else {
            self.record_mismatch(
                path,
                &format!("타입 불일치(기대=string, 실제={})", json_type_name(value)),
            );
            return;
        };
        if let Some(matched) = spec
            .values
            .iter()
            .copied()
            .find(|candidate| *candidate == text)
        {
            self.known_enums
                .entry(spec.label)
                .or_default()
                .insert(matched);
            return;
        }
        match spec.open {
            EnumOpenness::Open => {
                self.unknown_enum_count = self.unknown_enum_count.saturating_add(1);
            }
            EnumOpenness::Closed => {
                self.record_mismatch(path, "허용되지 않은 열거 값");
            }
        }
    }

    fn check_primitive(&mut self, path: &str, value: &Value, schema: &Schema) {
        match schema {
            Schema::String => {
                if !value.is_string() {
                    self.record_mismatch(
                        path,
                        &format!("타입 불일치(기대=string, 실제={})", json_type_name(value)),
                    );
                }
            }
            Schema::NonEmptyString => match value.as_str() {
                Some(text) if !text.is_empty() => {}
                Some(_) => self.record_mismatch(path, "빈 문자열 불가"),
                None => self.record_mismatch(
                    path,
                    &format!("타입 불일치(기대=string, 실제={})", json_type_name(value)),
                ),
            },
            Schema::DecimalString => match value.as_str() {
                Some(text) if is_valid_decimal(text) => {}
                Some(_) => self.record_mismatch(path, "십진수 문자열 형식 불일치"),
                None => self.record_mismatch(
                    path,
                    &format!(
                        "타입 불일치(기대=decimal-string, 실제={})",
                        json_type_name(value)
                    ),
                ),
            },
            Schema::Integer => {
                if value.as_i64().is_none() && value.as_u64().is_none() {
                    self.record_mismatch(
                        path,
                        &format!("타입 불일치(기대=integer, 실제={})", json_type_name(value)),
                    );
                }
            }
            Schema::MinInteger(minimum) => match value.as_i64() {
                Some(number) if number >= *minimum => {}
                Some(_) => self.record_mismatch(path, "정수 최솟값 미만"),
                None => self.record_mismatch(
                    path,
                    &format!("타입 불일치(기대=integer, 실제={})", json_type_name(value)),
                ),
            },
            Schema::Boolean => {
                if !value.is_boolean() {
                    self.record_mismatch(
                        path,
                        &format!("타입 불일치(기대=boolean, 실제={})", json_type_name(value)),
                    );
                }
            }
            Schema::Object { .. }
            | Schema::Array { .. }
            | Schema::Enum { .. }
            | Schema::Nullable(_) => {}
        }
    }

    fn check(&mut self, path: &str, value: &Value, schema: &Schema) {
        match schema {
            Schema::Nullable(inner) => {
                if !value.is_null() {
                    self.check(path, value, inner);
                }
            }
            Schema::Object { required, optional } => match value.as_object() {
                Some(object) => self.check_object(path, object, required, optional),
                None => self.record_mismatch(
                    path,
                    &format!("타입 불일치(기대=object, 실제={})", json_type_name(value)),
                ),
            },
            Schema::Array { item } => match value.as_array() {
                Some(items) => {
                    for (index, element) in items.iter().enumerate() {
                        let child_path = format!("{path}[{index}]");
                        self.check(&child_path, element, item);
                    }
                }
                None => self.record_mismatch(
                    path,
                    &format!("타입 불일치(기대=array, 실제={})", json_type_name(value)),
                ),
            },
            Schema::Enum {
                label,
                values,
                open,
            } => self.check_enum(
                path,
                value,
                EnumSpec {
                    label,
                    values,
                    open: *open,
                },
            ),
            Schema::String
            | Schema::NonEmptyString
            | Schema::DecimalString
            | Schema::Integer
            | Schema::MinInteger(_)
            | Schema::Boolean => self.check_primitive(path, value, schema),
        }
    }
}

fn summarize_object_fields(
    object: &Map<String, Value>,
    required: &'static [FieldSpec],
    optional: &'static [FieldSpec],
) -> Vec<FieldShape> {
    let mut fields = Vec::new();
    for spec in required.iter().chain(optional.iter()) {
        if let Some(entry) = object.get(spec.name) {
            fields.push(FieldShape {
                name: spec.name,
                kind: json_type_name(entry),
                count: entry.as_array().map(Vec::len),
            });
        }
    }
    fields
}

fn extract_item_count(kind: EndpointKind, target: &Value) -> Option<usize> {
    match kind {
        EndpointKind::Accounts | EndpointKind::IndicatorQuotes | EndpointKind::Quotes => {
            target.as_array().map(Vec::len)
        }
        EndpointKind::Holdings => target.get("items").and_then(Value::as_array).map(Vec::len),
        EndpointKind::Transactions => target
            .get("transactions")
            .and_then(Value::as_array)
            .map(Vec::len),
        EndpointKind::Sectors => target
            .get("sectors")
            .and_then(Value::as_array)
            .map(Vec::len),
        EndpointKind::Candles => target
            .get("candles")
            .and_then(Value::as_array)
            .map(Vec::len),
        EndpointKind::Token
        | EndpointKind::BuyingPower
        | EndpointKind::ExchangeRate
        | EndpointKind::CalendarKr
        | EndpointKind::CalendarUs => None,
    }
}

fn summarize_target_fields(schema: &'static Schema, target: &Value) -> Vec<FieldShape> {
    match schema {
        Schema::Object { required, optional } => target.as_object().map_or_else(Vec::new, |obj| {
            summarize_object_fields(obj, required, optional)
        }),
        Schema::Array { item } => match (item, target.as_array().and_then(|list| list.first())) {
            (Schema::Object { required, optional }, Some(Value::Object(first))) => {
                summarize_object_fields(first, required, optional)
            }
            _ => Vec::new(),
        },
        Schema::String
        | Schema::NonEmptyString
        | Schema::DecimalString
        | Schema::Integer
        | Schema::MinInteger(_)
        | Schema::Boolean
        | Schema::Enum { .. }
        | Schema::Nullable(_) => Vec::new(),
    }
}

fn unwrap_api_result<'a>(
    validator: &mut Validator,
    body: &'a Value,
    schema: &'static Schema,
) -> Option<&'a Value> {
    let Some(root) = body.as_object() else {
        validator.record_mismatch(
            "$",
            &format!("타입 불일치(기대=object, 실제={})", json_type_name(body)),
        );
        return None;
    };
    if root.get("error").is_some_and(|error| !error.is_null()) {
        validator.record_mismatch("$.error", "성공 응답에 error 필드가 존재");
    }
    match root.get("result") {
        Some(result_value) if !result_value.is_null() => {
            validator.check("$.result", result_value, schema);
            Some(result_value)
        }
        Some(_) => {
            validator.record_mismatch("$.result", "null 불가");
            None
        }
        None => {
            validator.record_mismatch("$.result", "필수 필드 누락");
            None
        }
    }
}

#[must_use]
pub fn validate_response(kind: EndpointKind, body: &Value) -> ShapeSummary {
    let mut validator = Validator::new();
    let schema = result_schema(kind);
    let target = match kind {
        EndpointKind::Token => {
            validator.check("$", body, schema);
            Some(body)
        }
        EndpointKind::Accounts
        | EndpointKind::Holdings
        | EndpointKind::BuyingPower
        | EndpointKind::Transactions
        | EndpointKind::ExchangeRate
        | EndpointKind::CalendarKr
        | EndpointKind::CalendarUs
        | EndpointKind::IndicatorQuotes
        | EndpointKind::Sectors
        | EndpointKind::Quotes
        | EndpointKind::Candles => unwrap_api_result(&mut validator, body, schema),
    };
    let fields = target.map_or_else(Vec::new, |val| summarize_target_fields(schema, val));
    let item_count = target.and_then(|val| extract_item_count(kind, val));
    let spec_match = if validator.mismatches.is_empty() {
        SpecMatch::Matched
    } else {
        SpecMatch::Mismatched(validator.mismatches)
    };
    ShapeSummary {
        fields,
        item_count,
        known_enums: validator.known_enums,
        unknown_enum_count: validator.unknown_enum_count,
        extra_field_count: validator.extra_field_count,
        spec_match,
    }
}
