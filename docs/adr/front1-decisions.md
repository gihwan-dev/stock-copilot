# front1 묶음에서 정한 것

ADR 0173 경량 절차로 front1 묶음을 구현하며 문서가 정하지 않은 것을 가장 단순하고 안전한 쪽으로 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| U-25 | 금지어·예외 목록을 `apps/desktop/tests/ux6-terms.mjs`(에이전트 쓰기 허용 목록 밖, 보호 경로)에 두고 `node --test`로 화면 코드(`apps/desktop/src`)와 코어 화면 문구 층(`apps/desktop/src-tauri/src`, `crates/application/src`)의 한국어 문자열·JSX 글을 훑는다. 시험 파일, 이야기 파일, `testing/`, 생성된 `bindings.ts`, Rust `#[cfg(test)]` 뒤는 뺀다. 예외는 `features/status/` 폴더뿐. 목록의 사용자 서명 확정은 사용자가 한다 | Rust·TS 양쪽에 목록을 따로 두기, `domain`까지 훑기(검색어 규칙 문자열이 화면 문구가 아니라서 뺌) |
| U-17 | 네 고정 문구(AI 글, 신호 카드 머리, 채팅 첫머리 데이터 전송, 앱 정보)를 R-13 Q7 문구로 `shared/lib/messages.ts`의 `notice.*`에 두어 다른 묶음(채팅, 신호 카드)이 같은 문구를 쓰게 한다. front1에서는 설정 화면 맨 아래 "앱 정보" 구역과 채팅 자리의 첫머리에 넣는다. AI 글·신호 카드 화면은 담당 묶음이 `notice.aiContent`·`notice.signalCard`를 붙인다 | 앱 정보를 별도 경로로 두기(설정 화면 조작이 하나 늘어 버림), 고정 문구 전용 DS 컴포넌트(`shared/ds/index.ts` 보호 파일이라 버림) |
| U-17 | 채팅 첫머리 문구의 "보유 종목"을 "보유 주식"으로 쓴다. 채팅 자리는 주 화면과 함께 그려지고, 주 화면의 빈 상태에 "보유 종목" 글이 없어야 한다는 U-09 시험과 부딪히기 때문이다 | R-13 Q7 초안 문구 그대로 쓰고 U-09 시험 고치기(기존 시험 변경이라 버림) |
| check-lite | 채팅 묶음의 로그인 실패 문구에서 "터미널에서"를 빼 "Claude에 다시 로그인한 뒤 물어봐 주세요"로 쓴다. U-28(P-3) 시험이 화면 문구의 터미널 안내를 막는다 | 채팅 묶음에 돌려보내기(묶음 끝 검사가 깨진 채로 남아 버림) |
| check-lite | master에 있던 Clippy 위반을 동작을 바꾸지 않고 고친다: 표시용 백분율 곱셈의 `unwrap_or_else`를 `match`로(`domain/money_display.rs`), 설정 IPC 명령을 `State<'_>` 대신 다른 명령처럼 `AppHandle<R>`로 받아 매크로 확장의 `unreachable!`을 없애고, 저장된 투자 성향이 없을 때의 기본값은 저장소 어댑터와 같은 `as_ref().unwrap_or(&fallback)`로, 이벤트 변환 함수는 제안 ID 문자열 도우미로 60줄 아래로 줄인다. 기존 시험 파일 `src-tauri/src/ipc/tests.rs`, `testkit-ports/tests/ai_job_finished.rs`의 위반은 기존 시험이라 고치지 않는다 | Clippy 설정에 예외 넣기(보호 파일·규칙 약화라 버림) |
| DS-12 | 건너뜀. `xtask ds-check`는 보호 경로 `crates/xtask/`의 새 명령이고 `shared/ds/index.ts` 잠금은 사용자 승인 행이라 사용자 서명 커밋으로 들어가야 한다 | - |
