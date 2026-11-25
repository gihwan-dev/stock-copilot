# fin-d 묶음 결정

경량 절차(ADR 0173)로 `fin-d` 묶음(AI 러너·MCP 서버를 데스크톱 앱 조립 지점에 연결)을 구현하며 문서가 정하지 않아 정한 것이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| fin-d 1 | AI-17이 미룬 "실행마다 새 토큰"을 `ClaudeCodeProvider::run_attached`·`resume_attached`로 연다. 실행별 첨부(`RunMcpAttachment`)는 브리지 설정과 토큰을 담고, 분석 실행에서만 받는다. 수집 실행에 첨부를 주거나 서버 이름이 기대와 다르면 자식을 띄우지 않고 검증 오류로 끝낸다. 설정에 고정된 브리지·토큰 경로(`run`)는 그대로 둔다 | 실행마다 `ClaudeCodeProvider`를 새로 만들기(중단·한도 추적이 실행마다 흩어짐) |
| fin-d 1 | 첨부에 `SpawnedPidNotice`를 달면 자식을 띄운 직후, 첫 stdin 줄을 쓰기 전에 CLI의 PID를 알린다. 실행별 MCP 서버가 상대의 부모 PID를 묶는 데 쓴다(architecture 2.4 보조 확인) | 스트림 이벤트로 PID 전달(앱 계약 `AiEvent` 변경 필요) |
