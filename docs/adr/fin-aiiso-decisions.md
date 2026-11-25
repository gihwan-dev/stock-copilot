# fin-aiiso 묶음에서 정한 것

ADR 0173 경량 절차로 fin-aiiso 묶음(AI-12 격리 실측 결과 반영, AI-15 웹 읽기 결론)을 하며 문서가 정하지 않은 것을 가장 단순하고 안전한 쪽으로 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| AI-12 | `--restricted`에서 init `plugins`에 나온 `cc-plugin-agents-md`·`cc-plugin-privacy`·`cc-plugin-plugin-authoring`는 Claude Code 내장 플러그인이다(R-02 실측과 같음). 사용자 플러그인으로 보지 않고 실행 옵션을 바꾸지 않는다. 사전 점검이 내장 허용 목록(`ALLOWED_BUILTIN_PLUGINS`) 밖의 플러그인과 `plugin_errors`를 이미 거부한다 | `--settings`로 `enabledPlugins`를 비우거나 끄기: 에이전트 세션은 `claude --help`조차 하네스가 막아 내장 플러그인의 키 형식과 `--restricted`에서의 효과를 확인할 수 없다. 확인 못 한 설정을 AI 경로에 넣으면 실행이 깨지거나 아무 효과가 없을 수 있어 버림 |
| AI-12 | 로그인 계정 식별자(이메일) 문맥 주입은 끄는 옵션을 확인하지 못해 남은 위험으로 받아들인다. 수집 실행에 임의 주소 도구가 없어 바깥으로 보낼 길이 검색어뿐이다. 버전을 올릴 때 다시 본다 | 앱 전용 `CLAUDE_CONFIG_DIR`(R-02 권장): 구독 로그인을 다시 해야 할 수 있어 사용자 절차가 늘고(P-1), 계정 문맥이 빠진다는 실측도 없다 |
| AI-15 | 수집 실행은 WebFetch를 허용하지 않고 `--restricted --tools WebSearch`를 유지한다(ADR 0179, ADR 0178 유지). 코드·시험 변경 없음 | `restricted-webfetch` 채택: 격리는 통과하지만 SEC-2의 도구 범위를 넘고, 계정 문맥이 있는 상태에서 임의 주소 요청이 유출 경로가 된다 |
