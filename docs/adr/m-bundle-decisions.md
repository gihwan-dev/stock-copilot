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
