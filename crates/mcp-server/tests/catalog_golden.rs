#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use mcp_server::{
        AnalysisCatalogPorts, ToolRegistry, build_analysis_catalog, make_cursor_store,
    };
    use serde_json::Value;
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::{FixedMarketToolReads, FixedPortfolioToolReads};

    fn golden_text() -> String {
        let exe = testkit::test_executable().expect("test executable");
        let root = exe
            .ancestors()
            .find(|dir| dir.join("fixtures").join("mcp").is_dir())
            .expect("workspace root with fixtures/mcp");
        fs::read_to_string(root.join("fixtures").join("mcp").join("tools_list.json"))
            .expect("골든 파일 읽기")
    }

    fn production_catalog() -> ToolRegistry {
        let clock = Arc::new(SteppedClock::new(
            Timestamp::from_parts(
                UnixSeconds::new(1_764_000_000_i64),
                Nanosecond::new(0_u32),
                OffsetSeconds::new(0_i32),
            )
            .expect("시각"),
        ));
        let cursors = make_cursor_store(clock);
        let ports = AnalysisCatalogPorts {
            portfolio: Arc::new(FixedPortfolioToolReads::default()),
            market: Arc::new(FixedMarketToolReads::default()),
        };
        build_analysis_catalog(&ports, &cursors).expect("제품 카탈로그 조립")
    }

    #[test]
    fn tools_list_matches_golden_file() {
        let tools = production_catalog().list_rmcp_tools();
        let actual = serde_json::to_value(&tools).expect("도구 목록 직렬화");
        let rendered = serde_json::to_string_pretty(&actual).expect("출력");
        let expected: Value = serde_json::from_str(&golden_text()).expect("골든 JSON");
        assert_eq!(
            actual, expected,
            "tools/list 골든 파일과 다르다. 실제 목록:\n{rendered}\n"
        );
    }

    #[test]
    fn every_catalog_tool_is_read_only_or_explicitly_state_changing() {
        let tools = production_catalog().list_rmcp_tools();
        assert!(!tools.is_empty(), "카탈로그가 비어 있으면 안 된다");
        for tool in tools {
            let annotations = tool.annotations.as_ref().expect("주석");
            assert_eq!(
                annotations.destructive_hint,
                Some(false),
                "{}: 파괴적 도구는 없다",
                tool.name
            );
        }
    }
}
