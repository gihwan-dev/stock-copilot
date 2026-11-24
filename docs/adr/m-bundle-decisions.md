# ADR M-bundle 초안: MCP 브리지·도구 틀·정규화·계약·포트폴리오·시장·주문 0건 결정

- 상태: 초안 (ADR 0173 속도 우선 묶음 진행 중 기록)
- 날짜: 2026-11-24
- 관련 작업: M-07, M-02, M-08, M-09, M-03, M-04, M-05

## 닫힌 실패(거부) 선택 및 세부 결정 표

| 작업 | 항목 | 결정(닫힌 실패 방향) | 대안 및 인계 |
|---|---|---|---|
| M-07 | `mcp-bridge` 인자·토큰 형식 | 정확히 `--socket <절대 경로>`(2인자)와 환경 변수 `STOCK_MCP_TOKEN`(64소문자 hex)만 수용. `--token`·위치 인자·상대 경로·심볼릭 링크·일반 파일·104바이트 초과·토큰 형식 오류·응답 0바이트 종료 모두 비정상 종료 코드로 거부. 작업 공간 crate 의존 0개(`zeroize` 단독) | Tauri `tauri.conf.json`은 현재 `bundle.active = false`이므로 `externalBin` sidecar 번들링과 코드 서명은 macOS 번들 작업(U-18)에 인계 |
| M-02 | 도구 호출 원장 실패·미구성 처리 | 도구가 등록된 서버에서 `ToolCallLedger`가 `Unconfigured`이거나 `record_call`이 실패하면 도구 실행 결과를 돌려주지 않고 `isError: true` + `code: "ledger_unavailable"`로 닫힌 실패 거부. S-09 SQLite 원장 구현은 `application::ToolCallLedger` 포트로 분리 | SQLite 영속 원장 연결은 S-09·U-01에 인계 |
| M-02 | 불투명 커서 수명·범위 및 3만 자 상한 | 커서는 OS CSPRNG 128비트(32소문자 hex)로 발급하고 `(run_id, tool_name, query_fingerprint)`에 묶으며 기본 TTL 10분(`600`초), 실행당 상한 128개 순환 유지. 페이지 응답이 3만 자를 넘으면 자동 축소하고 단일 항목이 3만 자를 넘으면 `response_too_large` 구조화 오류로 거부 | 커서 TTL·상한 조정 필요 시 설정으로 승격 가능 |
| M-08 | `untrusted_text` 정규화 범위 | 줄바꿈만 남기고 제어 문자 전부, 폭 0·양방향·태그·변형 선택 문자(고정 표) 제거. HTML 태그 제거, 마크다운 링크·이미지는 표시 글만, 맨 URL·위험 스킴 토큰은 통째로 삭제. 안정될 때까지 반복(최대 16회)해 중첩으로 링크가 되살아나지 않게 함. 길이는 문자 수 기본 4000, 상한 20000. 도구 설명·오류 메시지는 `&'static str`만 받게 해 외부 텍스트가 들어갈 수 없게 함 | NFKC 정규화와 스포트라이팅(데이터마킹)은 새 패키지·효과 미확인이라 하지 않음. URL을 수집 목록 대조로 남기는 일은 M-06·N-13에서 앱이 만든 별도 필드로 줌 |
| M-09 | 계약 테스트 위치·스냅샷 | 인증 뒤 서빙 함수가 `pub(crate)`라 duplex 계약 테스트는 `src/contract_tests.rs`(`#[cfg(test)]`)에 두고, 두 세대 JSON 줄 스냅샷은 `fixtures/mcp/contract_<세대>.jsonl`(`include_str!` 금지라 실행 파일 조상에서 읽음). `fake_ai_cli` 종단 간은 실제 Unix 소켓 서버로 두 세대 성공·다른 실행 토큰·빈 토큰·만료 토큰 거부를 고정 | 서빙 함수 공개(피어 검증 우회 경로가 생겨 버림) |
| M-09 | 2026-07-28 `ping` | `server/discover` 뒤 `_meta`를 실은 `ping`이 rmcp에서 `-32601`로 끝나 스냅샷에서 뺐다. 2025-06-18 세대 `ping`만 스냅샷에 고정 | rmcp 동작을 바꾸는 우회 |
| M-09 | 가짜 MCP 클라이언트의 `_meta` 키 | rmcp 3.9.0은 SEP-2575 `io.modelcontextprotocol/clientCapabilities`를 요구하는데 `fake_mcp_client`는 `clientCapabilities`만 실어 2026 세대 도구 목록이 비었다. 기존 테스트를 고치지 않고 두 키를 함께 싣도록 추가 | 기존 testkit 단언 변경(기존 테스트 변경 금지) |
| M-09 | `tools/list` 골든 파일 | M-09는 고정 도구 레지스트리의 목록을 스냅샷에 포함하고, 제품 도구 목록 골든(`fixtures/mcp/tools_list.json`)은 첫 제품 도구가 들어오는 M-03에서 카탈로그 조립 함수와 함께 시작 | 빈 카탈로그 골든 |
| M-03 | 포트폴리오 도구의 자료원 | `application::PortfolioToolReads` 포트가 도구별 보기 레코드(`PortfolioToolView` 등)를 돌려주고, `mcp-server`는 인자 검증·정렬·JSON 렌더링만 한다. 계좌는 `MaskedAccountNumber`(뒤 4자리)만 레코드에 들어갈 수 있어 전체 번호가 도구에 닿지 않는다. 가짜는 `testkit-ports::tool_read_fakes::FixedPortfolioToolReads`. 포트 구현(저장소+`GetPortfolioView`·`GetAllocationView` 조립)은 앱 조립 작업(U-01)에 인계 | 도구가 `PortfolioValuation`을 직접 렌더링(도메인 내부 구조에 MCP 출력이 묶임) |
| M-03 | 표본 부족 처리 | `SampleSufficiency::Insufficient`이면 `sample: "insufficient"`와 함께 위험 지표(변동성·최대 낙폭)를 `null`로 고정해 내보내고 기간 수익률·벤치마크는 그대로 준다 | 수익률까지 숨김 |
| M-03 | 외부 이름 | 종목명·섹터 이름은 브로커 자료지만 외부 텍스트로 보고 `untrusted_text`로 정규화해 준다. 시각은 오프셋을 붙인 RFC 3339 문자열, 금액은 `{amount, currency}` 십진 문자열 | 이름을 평문 문자열로 노출 |
| M-03 | `get_transactions` 범위 | 인자는 `kind`(all·trades·dividends·cash_flows), `limit`, `cursor`뿐. 최신순(같으면 종류·종목순) 정렬, 커서는 `kind`별 질의 지문에 묶음 | 기간 인자(문서 미정) |
| M-04 | 시장 도구의 자료원 | M-03과 같은 방식으로 `application::MarketToolReads` 포트와 보기 레코드(`QuoteToolView` 등, 모두 `collected_at` 필수)를 두고 `mcp-server`는 검증·정렬·렌더링만 한다. 포트 구현(`GetMarketOverview`·`read_queries` 조립)은 U-01에 인계 | 도구가 `MarketOverview`·`MarketIndicatorQuote`를 직접 렌더링 |
| M-04 | 근사·공식 표시 | 항목마다 `provenance`(official·approximate). 토스 미국 ETF로만 얻는 S&P 500·나스닥 100·원유·금·달러 지수는 포트 값과 무관하게 `approximate`로 고정(공식 값으로 잘못 표시되는 쪽을 닫음). 미 국채 금리는 재무부·FRED 값이라 포트 표시를 따른다 | 포트 표시를 그대로 믿음 |
| M-04 | 인자 범위 | `market`은 KR·US(생략 시 KR), `get_investor_flow`는 KR만, `get_candles`는 `days` 1~250(기본 60)과 `limit`·`cursor` 페이지, `get_rankings`의 `kind`는 거래대금·상승·하락·외국인 순매수·기관 순매수. 지수·금리 값은 단위(`unit`)를 붙인 십진 문자열 | 미국 수급 허용, 일봉 한 번에 전부 |
| M-03 | 거래 커서 | 위치 기반 커서를 유지한다. 쪽 사이에 새 거래가 맨 앞에 들어오면 다음 쪽에 중복이 생길 수 있으나 커서 TTL 10분·질의 지문 범위 안의 표시 문제라 받아들인다 | 시점 고정 스냅샷 커서(저장소 버전 필요) |
| 묶음 | 리뷰 위임 경로 | 세션 훅이 기본 `Agent` 리뷰를 막아 T3 `delegate_task`(별도 작업 사본, `role: review`, 기준 커밋·브랜치·대상 커밋 줄)로만 위임 | 기본 Agent |
