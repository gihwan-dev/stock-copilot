use broker_toss_schema::types::{Account, ApiResponse, ErrorResponse, ExchangeRateResponse};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) fn validate_payload(path: &str, status: &str, body: &Value) -> Result<(), String> {
    if status == "200" {
        decode::<ApiResponse>(body)?;
        match path {
            "/api/v1/accounts" => decode::<Vec<Account>>(&body["result"]),
            "/api/v1/exchange-rate" => decode::<ExchangeRateResponse>(&body["result"]),
            _ => Ok(()),
        }
    } else {
        decode::<ErrorResponse>(body)
    }
}

fn decode<T: DeserializeOwned>(value: &Value) -> Result<(), String> {
    let _decoded: T = serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    Ok(())
}
