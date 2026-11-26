#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{
        AiRunId, MarketToolError, RetryDelay, SystemFailure, SystemPort, SystemPortError, ToolName,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use mcp_server::{
        ToolErrorPayload, ToolInvocation, ToolRegistry, make_cursor_store, register_market_tools,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::FixedMarketToolReads;

    const TOOLS: [&str; 6] = [
        "get_quote",
        "get_candles",
        "get_market_overview",
        "get_rankings",
        "get_sectors",
        "get_investor_flow",
    ];

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn registry_with(reads: FixedMarketToolReads) -> ToolRegistry {
        let clock = Timestamp::from_parts(
            UnixSeconds::new(1_764_000_000_i64),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(32_400_i32),
        )
        .expect("time");
        let cursors = make_cursor_store(Arc::new(SteppedClock::new(clock)));
        let mut registry = ToolRegistry::new();
        register_market_tools(&mut registry, &Arc::new(reads), &cursors).expect("register");
        registry
    }

    fn args(name: &str) -> Map<String, Value> {
        let value = match name {
            "get_quote" | "get_candles" | "get_investor_flow" => json!({ "symbol": "000660" }),
            "get_rankings" => json!({ "kind": "trading_amount" }),
            _ => json!({}),
        };
        value.as_object().cloned().expect("object")
    }

    async fn error_of(registry: &ToolRegistry, name: &str) -> Value {
        let tool_name = ToolName::try_new(name).expect("name");
        let (_spec, tool) = registry.get(&tool_name).expect("tool");
        let run_id = AiRunId::try_new("run-errors").expect("run");
        let arguments = args(name);
        let reply: Result<Value, ToolErrorPayload> = tool
            .invoke(ToolInvocation {
                run_id: &run_id,
                arguments: &arguments,
            })
            .await;
        reply.expect_err("tool error").to_json_value()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Retry {
        Yes,
        No,
    }

    fn expect_all(error: MarketToolError, code: &str, retry: Retry, phrase: &str) {
        runtime().block_on(async {
            let registry = registry_with(FixedMarketToolReads::default().failing_with(error));
            for name in TOOLS {
                let value = error_of(&registry, name).await;
                assert_eq!(value["code"], json!(code), "{name}: {error:?}");
                assert_eq!(
                    value["retryable"],
                    json!(retry == Retry::Yes),
                    "{name}: {error:?}"
                );
                let message = value["message"].as_str().expect("message");
                assert!(message.contains(phrase), "{name}: {message}");
            }
        });
    }

    #[test]
    fn unconnected_market_data_is_not_retryable_and_says_so() {
        expect_all(
            MarketToolError::NotConnected,
            "data_unavailable",
            Retry::No,
            "not connected",
        );
    }

    #[test]
    fn capability_and_symbol_gaps_are_not_retryable() {
        expect_all(
            MarketToolError::NotOffered,
            "data_unavailable",
            Retry::No,
            "does not offer",
        );
        expect_all(
            MarketToolError::NotFound,
            "data_unavailable",
            Retry::No,
            "no data for this symbol",
        );
        expect_all(
            MarketToolError::Rejected,
            "data_unavailable",
            Retry::No,
            "rejected",
        );
        expect_all(
            MarketToolError::InvalidResponse,
            "internal_error",
            Retry::No,
            "could not read",
        );
    }

    #[test]
    fn only_transient_brokerage_failures_are_retryable() {
        expect_all(
            MarketToolError::Unavailable,
            "upstream_unavailable",
            Retry::Yes,
            "temporarily unavailable",
        );
        expect_all(
            MarketToolError::RateLimited,
            "rate_limited",
            Retry::Yes,
            "limiting requests",
        );
        expect_all(
            MarketToolError::Storage(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            )),
            "upstream_unavailable",
            Retry::Yes,
            "retry later",
        );
    }
}
