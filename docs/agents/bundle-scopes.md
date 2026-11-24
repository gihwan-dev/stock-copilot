# 묶음 범위

경량 절차(ADR 0173)의 묶음 스레드가 맡는 범위다. 각 묶음은 자기 절의 행을 위에서부터 차례로 구현한다. 이미 커밋된 행(커밋 제목의 작업 ID)은 건너뛴다. 규칙은 [bundle-rules.md](bundle-rules.md).

## ai — AI 러너 (`adapters/ai-claude-code`)
AI-17, AI-18(프로필 생성과 명령 조립까지), AI-08, AI-14, AI-16, AI-07(H-20 없이 가능한 범위), AI-10 코어(유스케이스·IPC), AI-09(Codex: AI-01 디코더와 가짜 CLI로, AI-12에 걸린 부분은 포트·가짜).
- 꼼꼼하게: 2행마다 커밋 범위를 하위 에이전트(Claude Opus medium, 리뷰 전용)에게 맡기고 기다리지 않는다. 리뷰 초점: 격리 점검 전 출력 누출, 허용하지 않은 환경 변수·API 키 전달, 토큰·프롬프트의 로그·오류 노출, 남는 자식 프로세스, 디코드 오류·격리 퇴보 뒤 진행. 차단·중요만 고치고 `docs/reviews/AI-bundle.md` 표에 남긴다.
- 경계: `env_clear` 뒤 이름 정한 키만, API 키 환경 변수가 있으면 거부, 격리 통과 전 출력 없음, 퇴보·디코드 오류는 즉시 중단, AI 글은 `UntrustedText`, 테스트는 `fake_ai_cli`만.

## n — 수집 (`adapters/sources`)
N-16, N-09, N-10, N-11(스케줄러는 포트), N-12, N-20 코어, N-21·N-14 코어(유스케이스·IPC).
- MCP 도구 등록(N-13, N-18, M-06)은 m 묶음 담당: 유스케이스 함수까지만. 테스트는 녹화 픽스처와 가짜 HTTP 서버만. 화면 없음.

## front1 — 프런트 1 (`apps/desktop/src`)
U-09, U-10, U-11, U-21, U-27, U-16, U-17, U-25(UX-6 목록 그대로), DS-12.
- 단순하고 직관적인 UI, DS 컴포넌트만, 인라인 스타일·토큰 밖 색 금지, PNG 기준 화면 없음, 코어가 없으면 모의 바인딩. 채팅·첫 실행·고급 설정·종목 목록/상세·신호 카드/성적표 폴더는 다른 묶음 담당(라우터엔 자리만 한 줄).

## m — MCP (`crates/mcp-server`, `crates/mcp-bridge`)
M-02(원장은 포트), M-08, M-09(`fake_ai_cli`로 두 세대 종단 간), M-03, M-04, M-05, 그리고 도구 등록: N-13, N-18, M-06, J-02, E-01~E-03의 도구. 유스케이스가 없으면 `application` 포트와 `testkit-ports` 가짜로.
- 꼼꼼하게: ai 묶음과 같은 방식의 리뷰 병행. 초점: 토큰 없이·다른 실행의 토큰으로·만료 뒤 통과, 스키마 검증 우회, 정규화 없는 AI 글, MCP로 주문·설정 변경, 토큰 노출. `docs/reviews/M-bundle.md`.
- 경계: `unsafe`·`cfg` 분기·프로세스 실행·TCP/HTTP 리스너 없음, 상대 프로세스 확인 실제 구현 전에는 모든 연결 거부, MCP로 주문·설정 변경 불가.

## s — 저장소 (`adapters/store-sqlite`)
S-22, S-08, S-09, S-10, J-13 저장소 쪽, S-16의 HMAC 없는 부분, S-11, 그리고 다른 묶음이 포트로만 둔 저장소의 SQLite 구현(master를 받을 때마다 `application`의 새 저장소 포트를 확인).
- 마이그레이션은 이 묶음만 만든다(마지막 번호 다음부터). S-20, S-12는 하지 않는다. 포트는 `application`, 가짜는 `testkit-ports`, 공유 계약 테스트는 양쪽, 쓰기는 `BEGIN IMMEDIATE`.

## o — 주문 (`crates/order-execution`, `crates/approval-command`, `adapters/broker-toss-trade`)
T-16, O-06(기본 꺼짐), O-05, O-25, O-11 코어, O-26 코어, O-07·O-09 코어(macOS 호출은 포트·가짜), O-08 승인 IPC, O-13, O-20, O-21(가짜만).
- 실제 주문 절대 없음. `broker-toss-trade`는 가짜 HTTP 서버만. O-15는 하지 않는다. 타입 봉인과 주문 타입 격리 `arch-check` 규칙은 풀지 않는다. 묶음 끝에 리뷰 한 번(승인 없이·같은 승인 두 번·스위치 꺼짐·상한 초과 주문, 결과 모르는 주문 재전송, 금액·수량 넘침).

## native — 상주·스케줄러 (`crates/scheduler`, `apps/desktop/src-tauri`, `adapters/platform-macos`)
J-14, J-11, J-12, J-15, J-13 연결, J-10 코어, J-03, J-04, J-05, J-06, U-08 코어, 그리고 앱 조립(master를 받을 때마다 `application`에 새로 생겼는데 `build_app`·명령·capability에 연결되지 않은 것을 연결).
- 화면 없음. 벽시계 비교, 시계는 포트. 실제 시스템 변경 테스트 없음.

## t — 토스·심층 연구
T-17(남았으면), D-59, A-26, T-14(도구와 합성 픽스처까지), G-05, G-10, G-11, G-06, G-07, G-08, G-12(유스케이스, AI·웹 조사는 포트).
- `toss-smoke` 실행 금지. 토스 테스트는 가짜 서버·합성 픽스처만. `crates/mcp-server`, `adapters/ai-claude-code`, `adapters/sources`, `adapters/store-sqlite`, `crates/scheduler`, `crates/order-execution`, `apps/desktop`은 고치지 않는다.

## chat — 채팅
C-10 코어, G-01 코어, E-09 코어, C-03(`AppShell` 채팅 자리), C-04, C-05, C-06, C-07, O-12(승인 IPC가 없으면 버튼 비활성).
- AI 글은 `UntrustedText`, HTML로 넣지 않는다. 채팅에서 주문·설정 변경 불가를 타입과 테스트로. DS 컴포넌트만.

## est — 차트·추정
E-02, E-03, E-04, E-05, E-10, G-13(코어), E-06, E-07, E-11, E-08, E-09 화면, O-08 버튼, O-14, U-29, J-09 화면, U-13, J-07, J-08.
- MCP 도구 등록은 m 묶음 담당. DS 컴포넌트만, 새 차트 라이브러리 금지(Canvas).

## front2 — 첫 실행·고급 설정 화면
U-06(키는 화면 상태·로그에 남기지 않는다), U-07, U-22, U-08 화면, U-15, AI-10 화면, N-14 화면, G-01 화면, J-10 화면, U-28(모의 바인딩).
- 실제 키로 시험하지 않는다. `src-tauri`와 `crates/*`는 고치지 않는다.
