# fin-d 묶음 결정

경량 절차(ADR 0173)로 `fin-d` 묶음(AI 러너·MCP 서버를 데스크톱 앱 조립 지점에 연결)을 구현하며 문서가 정하지 않아 정한 것이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| fin-d 1 | AI-17이 미룬 "실행마다 새 토큰"을 `ClaudeCodeProvider::run_attached`·`resume_attached`로 연다. 실행별 첨부(`RunMcpAttachment`)는 브리지 설정과 토큰을 담고, 분석 실행에서만 받는다. 수집 실행에 첨부를 주거나 서버 이름이 기대와 다르면 자식을 띄우지 않고 검증 오류로 끝낸다. 설정에 고정된 브리지·토큰 경로(`run`)는 그대로 둔다 | 실행마다 `ClaudeCodeProvider`를 새로 만들기(중단·한도 추적이 실행마다 흩어짐) |
| fin-d 1 | 첨부에 `SpawnedPidNotice`를 달면 자식을 띄운 직후, 첫 stdin 줄을 쓰기 전에 CLI의 PID를 알린다. 실행별 MCP 서버가 상대의 부모 PID를 묶는 데 쓴다(architecture 2.4 보조 확인) | 스트림 이벤트로 PID 전달(앱 계약 `AiEvent` 변경 필요) |
| fin-d 2 | 앱 쪽 연결 계층 `ai_runtime`을 `src-tauri`에 둔다. `McpAttachedAi`가 `application::AiProvider`를 구현하고, 분석 실행(실행·재개·사전 점검)마다 `RunMcpHost`로 실행별 MCP 서버를 열어 소켓·토큰·PID 묶기를 첨부해 넘긴다. 수집 실행은 서버를 열지 않고 그대로 넘긴다(앱 MCP 없음, architecture 2.2). 어댑터 타입 이름은 `main.rs` 밖에 두지 않으므로(arch-check `adapter-construction`) 어댑터는 `AttachableAi` 트레이트 뒤에 둔다 | 어댑터가 MCP 서버를 직접 띄움(ai-claude-code → mcp-server 의존, 구조 규칙 위반) |
| fin-d 2 | 서버를 못 띄우면 CLI를 띄우지 않고 `Unavailable`로 끝낸다. 스트림이 끝나거나(`None`·오류) 버려지면 세션을 놓아 토큰 폐기·소켓 삭제가 일어난다(`McpServerHandle`의 `Drop`) | 실행 종료 기록 시점에 정리 |
| fin-d 2 | 서버는 부모 PID를 `Pending`으로 시작하고 첨부의 PID 알림으로 CLI PID를 묶는다. 묶기 전 연결, 다른 부모, 상대 확인 수단 없음은 모두 거부(mcp-server 기존 동작)를 앱 조립 경로에서 다시 확인한다 | 상대 확인 끄기 |
| fin-d 2 | 소켓 기반 폴더는 앱 데이터 폴더 아래 짧은 `m`(0700)으로 하고, 실행마다 `RunSocketSpec::generate_under`로 무작위 하위 폴더를 만든다. 경로가 104바이트를 넘거나 조상에 심볼릭 링크가 있으면 분석 실행을 띄우지 않는다 | 시스템 임시 폴더 |
| fin-d 3 | 제품 분석 카탈로그는 `mcp_server::build_analysis_catalog`로 문서 도구 20개(design 3.1, `fixtures/mcp/tools_list.json`)를 그대로 등록한다. 자료원 포트 중 제품 구현이 없는 것(포트폴리오·시장·뉴스·웹 조사·기사·논거 대상·호출 이력·추정 실행 문맥·투자 성향)은 `UnconnectedToolData`가 저장소 사용 불가 오류로 닫고, `read_article`의 본문 읽기는 꺼짐으로 본다. 논거 저장소·추정 과제·제출·성적표 관측은 `SqliteRepositories`에 위임을 더해 연결한다 | 자료원이 생길 때까지 해당 도구를 목록에서 빼기(문서 목록과 어긋남) |
| fin-d 3 | 앱 MCP 서버 이름은 `stock`, CLI 허용 도구는 카탈로그 이름에서 `mcp__stock__<도구>`로 만든다(사전 점검의 기대 목록과 같은 원본) | 이름 목록을 따로 고정 |
| fin-d 3 | MCP 호출 원장은 `SqliteRepositories`의 `ToolCallLedger` 위임(채팅 저장소의 추가 전용 표)으로 남긴다 | 원장 미설정(모든 호출이 `ledger_unavailable`) |
