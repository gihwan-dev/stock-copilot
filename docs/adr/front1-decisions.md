# front1 묶음에서 정한 것

ADR 0173 경량 절차로 front1 묶음을 구현하며 문서가 정하지 않은 것을 가장 단순하고 안전한 쪽으로 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| U-25 | 금지어·예외 목록을 `apps/desktop/tests/ux6-terms.mjs`(에이전트 쓰기 허용 목록 밖, 보호 경로)에 두고 `node --test`로 화면 코드(`apps/desktop/src`)와 코어 화면 문구 층(`apps/desktop/src-tauri/src`, `crates/application/src`)의 한국어 문자열·JSX 글을 훑는다. 시험 파일, 이야기 파일, `testing/`, 생성된 `bindings.ts`, Rust `#[cfg(test)]` 뒤는 뺀다. 예외는 `features/status/` 폴더뿐. 목록의 사용자 서명 확정은 사용자가 한다 | Rust·TS 양쪽에 목록을 따로 두기, `domain`까지 훑기(검색어 규칙 문자열이 화면 문구가 아니라서 뺌) |
| U-17 | 네 고정 문구(AI 글, 신호 카드 머리, 채팅 첫머리 데이터 전송, 앱 정보)를 R-13 Q7 문구로 `shared/lib/messages.ts`의 `notice.*`에 두어 다른 묶음(채팅, 신호 카드)이 같은 문구를 쓰게 한다. front1에서는 설정 화면 맨 아래 "앱 정보" 구역과 채팅 자리의 첫머리에 넣는다. AI 글·신호 카드 화면은 담당 묶음이 `notice.aiContent`·`notice.signalCard`를 붙인다 | 앱 정보를 별도 경로로 두기(설정 화면 조작이 하나 늘어 버림), 고정 문구 전용 DS 컴포넌트(`shared/ds/index.ts` 보호 파일이라 버림) |
| DS-12 | 건너뜀. `xtask ds-check`는 보호 경로 `crates/xtask/`의 새 명령이고 `shared/ds/index.ts` 잠금은 사용자 승인 행이라 사용자 서명 커밋으로 들어가야 한다 | - |
