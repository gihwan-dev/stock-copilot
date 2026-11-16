pub(crate) mod validation;
use serde_json::Value;
use std::{fs, path::Path};
use validation::validate_recording;

pub(crate) fn spec() -> Value {
    serde_json::from_str(
        &fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
    )
    .expect("JSON")
}

pub(crate) fn recordings() -> Vec<Value> {
    let mut paths = fs::read_dir(Path::new("tests/recordings"))
        .expect("recordings")
        .map(|entry| entry.expect("entry").path())
        .collect::<Vec<_>>();
    paths.sort();
    assert_ne!(paths.len(), 0);
    paths
        .into_iter()
        .map(|path| {
            serde_json::from_str(&fs::read_to_string(path).expect("file")).expect("recording JSON")
        })
        .collect()
}

pub(crate) fn serve_recordings(recording: &Value) -> testkit::LocalHttpServer {
    validate_recording(&spec(), recording).expect("validate before serving");
    let body = serde_json::to_vec(&recording["body"]).expect("body");
    testkit::LocalHttpServer::start(format!("HTTP/1.1 {} Recorded\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", recording["status"].as_str().expect("status"), body.len(), String::from_utf8(body).expect("utf8")).into_bytes()).expect("server")
}

pub(crate) fn route(recording: &Value) -> broker_toss::ReadRoute {
    use broker_toss::ReadRoute;
    match recording["path"].as_str().expect("path") {
        "/api/v1/accounts" => ReadRoute::Accounts,
        "/api/v1/holdings" => ReadRoute::Holdings,
        "/api/v1/transactions" => ReadRoute::Transactions,
        "/api/v1/prices" => ReadRoute::Quotes,
        "/api/v1/candles" => ReadRoute::Candles,
        "/api/v1/buying-power" => ReadRoute::BuyingPower,
        "/api/v1/sellable-quantity" => ReadRoute::SellableQuantity,
        "/api/v1/orders" => ReadRoute::Orders,
        "/api/v1/exchange-rate" => ReadRoute::FxRate,
        "/api/v1/orderbook" => ReadRoute::Orderbook,
        "/api/v1/price-limits" => ReadRoute::PriceLimits,
        "/api/v1/commissions" => ReadRoute::CommissionRate,
        "/api/v1/stocks/all" => ReadRoute::Instruments,
        "/api/v1/stocks" => ReadRoute::Stocks,
        "/api/v1/rankings" => ReadRoute::Rankings,
        "/api/v1/rankings/investor-trading" => ReadRoute::InvestorRankings,
        "/api/v1/market-indicators/prices" => ReadRoute::IndicatorQuotes,
        "/api/v1/sectors" => ReadRoute::Sectors,
        "/api/v1/sectors/rankings" => ReadRoute::SectorRankings,
        "/api/v1/market-calendar/KR" => ReadRoute::CalendarKr,
        "/api/v1/market-calendar/US" => ReadRoute::CalendarUs,
        other => dynamic_route(other),
    }
}

fn dynamic_route(path: &str) -> broker_toss::ReadRoute {
    use broker_toss::ReadRoute;
    match path {
        "/api/v1/orders/{orderId}" => {
            ReadRoute::Order(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/stocks/{symbol}/warnings" => {
            ReadRoute::StockWarnings(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/stocks/{symbol}/investor-trading" => {
            ReadRoute::StockInvestorFlow(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/stocks/{symbol}/program-trades" => {
            ReadRoute::ProgramTrades(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/stocks/{symbol}/short-selling" => {
            ReadRoute::ShortSelling(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/stocks/{symbol}/credit-trades" => {
            ReadRoute::CreditTrades(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/stocks/{symbol}/securities-lending" => {
            ReadRoute::SecuritiesLending(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/market-indicators/{symbol}/candles" => {
            ReadRoute::IndicatorCandles(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/market-indicators/{symbol}/investor-trading" => ReadRoute::IndicatorInvestorFlow(
            broker_toss::RouteId::new("synthetic-731").expect("id"),
        ),
        "/api/v1/sectors/{sectorId}" => {
            ReadRoute::Sector(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/sectors/{sectorId}/stocks" => {
            ReadRoute::SectorStocks(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        "/api/v1/sectors/{sectorId}/etfs" => {
            ReadRoute::SectorEtfs(broker_toss::RouteId::new("synthetic-731").expect("id"))
        }
        other => panic!("unknown read route {other}"),
    }
}
