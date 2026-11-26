# ADR 0184: 앱 명령 하나에 AI 정책 하나, IPC로 할 수 있는 일을 MCP로도 한다

- 상태: 확정 (2026-11-26). 사용자가 "MCP 베이스로 서버를 구성해서 서버 API로 할 수 있는 모든 거는 AI도 할 수 있게"를 요청했다. 이 ADR은 `mcp-server` `ToolSpec::try_new`의 이름 조각 금지(`is_forbidden_tool_name`)와 architecture.md 2.4의 "승인, 설정 변경은 MCP에 없다"를 대체한다. 주문 안전 겹(architecture.md 4장)은 그대로다.

## 배경

화면은 Tauri IPC 명령 51개를 쓰는데 MCP는 분석용 읽기 도구 20개뿐이었다. 도구 이름에 `order`, `setting`, `token` 같은 조각이 있으면 등록을 막는 규칙이 노출 여부를 이름으로 추측했고, 새 IPC 명령이 생겨도 AI 쪽은 따로 손으로 만들어야 했다.

## 결정

1. **명령 표가 단일 원천이다.** `apps/desktop/src-tauri/src/ai_commands/policy.rs`의 `COMMAND_POLICIES`에 IPC 명령마다 `AiPolicy`(`Read`, `WriteDirect`, `Confirm`, `Never`)를 하나씩 적는다. 시험 `every_ipc_command_declares_exactly_one_ai_policy`가 생성된 `ipc/commands.txt`와 표를 맞대어, 정책을 적지 않은 IPC 명령이 생기면 실패한다.
2. **도구는 IPC 명령 함수를 그대로 부른다.** `ai_commands/routes.rs`의 경로마다 IPC 함수(`system_data::set_principles::<R>` 등)를 `ipc_handler`에 넘긴다. 도구 인자를 IPC 요청 타입으로 역직렬화(`deny_unknown_fields`)해 같은 `AppHandle`·관리 상태·유스케이스로 실행하므로 화면과 AI의 동작이 갈라지지 않는다. `scope: "current"` 같은 고정 필드는 경로의 `fixed`가 채운다. 도구 이름은 `app_<IPC 이름>`이다(분석 도구 `get_portfolio` 등과 겹치지 않게).
3. **등록이 표를 강제한다.** `register_app_commands`는 `Never`가 아닌 명령에 경로가 없거나, `Never` 명령에 경로가 있거나, 표에 없는 경로가 있으면 실패한다. 시험이 모든 노출 명령의 도구·스키마 존재와 `Never` 명령의 부재(`tools/list`에 없음, 호출하면 unknown tool)를 확인한다. 스키마의 모든 열거 값과 필수 필드 조합이 IPC 요청 타입으로 읽히는지도 시험한다.
4. **정책 표(51개).**

| 정책 | 명령 |
|---|---|
| `Read` (23) | `app_version`, `get_settings`, `get_chat_history`, `get_briefings`, `get_portfolio`, `get_allocation`, `get_job_schedules`, `get_public_ip`, `check_toss_connection`, `get_update_status`, `get_risk_settings`, `get_self_check`, `get_status`, `get_scorecard`, `get_transactions`, `get_instruments`, `get_instrument_detail`, `get_thesis`, `get_signals`, `get_account_selection`, `get_principles`, `get_news_settings`, `get_ai_settings` |
| `WriteDirect` (16) | `set_job_schedule`, `save_investor_profile`, `save_order_settings`, `add_watchlist`, `remove_watchlist`, `skip_signal`, `select_account`, `set_principles`, `backup_now`, `export_data`, `set_news_source`, `set_sec_contact`, `set_wake_criteria`, `set_aliases`, `set_task_model`, `set_budget_percent` |
| `Confirm` (1) | `request_order_approval` |
| `Never` (11) | `set_order_permission`, `send_chat_message`, `resubscribe_chat_turn`, `cancel_chat_turn`, `open_external`, `copy_text`, `submit_toss_keys`, `restart_to_update`, `delete_all_data`, `set_optional_key`, `set_claude_config_dir` |

5. **쓰기는 채팅에서만, 기록과 알림을 남긴다.** `WriteDirect`·`Confirm` 도구는 실행 ID가 `ChatTurnBook`에 있는 사용자 채팅 실행에서만 돈다. 예약 추정·브리핑 실행은 `forbidden_operation`으로 거부한다(뉴스 본문을 통한 프롬프트 주입으로 설정이 바뀌는 길을 막는다). 성공하면 `AiCommandJournal`이 감사 로그(`AuditKind::AiCommandApplied { tool, summary }`, 해시 사슬 태그 `ai_command_applied`)와 진단 로그 한 줄(`ai-command tool=… summary="…"`)을 남긴다. 요약은 인자 이름과 짧은 식별자 값만 남기고, 자유 문장은 `<text>`, `secret`·`key`·`token`·`password`·`contact`·`credential`이 들어간 필드는 `<redacted>`로 바꾼다. 이어서 `UiEvent::AiCommandApplied(명령)`을 내고 화면은 전체 조회를 무효화한다. 채팅 답 아래에는 "AI가 투자 원칙을 바꿨어요." 같은 한 줄이 남는다(도구 이름 → 채팅 주제 `change-*`).
6. **주문은 요청만 만든다.** `app_request_order_approval`은 IPC `request_order_approval`을 부르지 않는다. 제안 ID 형식만 확인하고 `UiEvent::AiApprovalRequested(제안 ID)`를 낸 뒤 `awaiting-user-approval`을 돌려준다. 주문은 사용자가 신호 카드의 승인 버튼으로 기존 IPC를 부를 때만 네이티브 확인 창, Touch ID, 주문 스위치, 시간당 한도를 거쳐 나간다. 시험이 이 도구 호출 뒤 주문 승인 처리기 호출 수가 0인 것을 확인한다. AI·MCP 코드는 주문 전송 포트에 닿지 않는다(AGENTS.md 8항, architecture.md 4.1).
7. **`Never`는 사용자 전용이다.** 키 입력과 읽기, 주문 스위치, 전체 삭제, AI 설정 폴더, 재시작, 외부 링크·클립보드, 채팅 배관은 도구가 없다. 어떤 도구 응답도 키 값을 담지 않는다(설정 읽기는 "있음/없음"만).
8. **응답 규약.** 결과의 모든 문자열은 `NormalizedUntrustedText`로 정리한다(제어·폭 0 문자 제거, 링크 평문화, 4천 자 상한). 읽기 도구는 선택 인자 `limit`(1~200)·`cursor`를 받아 응답의 가장 큰 최상위 목록을 기존 `paginate_items`로 나눈다(`items`, `total`, `returned`, `next_cursor`, 원래 필드 이름은 `items_field`). 응답 3만 자 상한, 스키마 검증, 호출 기록(`ToolCallLedger`)은 기존 서버 규약을 그대로 탄다.
9. **채팅 허용 목록.** `application::check_analysis_tool`의 `APP_COMMAND_TOOLS`에 노출 도구 40개를 적는다. 시험 `chat_allowlist_matches_the_policy_table`이 표와 맞는지 확인한다. 앱 채팅(`send_chat_message`)과 같은 소켓·토큰으로 붙는 모든 실행이 같은 레지스트리를 받는다.
10. **이름 조각 금지 삭제.** `ToolSpec::try_new`는 이름을 보지 않는다. 그 규칙을 단언하던 보호 시험(`tool_framework.rs`)은 사용자 승인으로 별도 커밋에서 바꿨다. 분석 카탈로그 전용 시험(`no_order_path.rs`, `mcp_catalog` 골든 목록)은 분석 카탈로그가 그대로라 바꾸지 않았다.

## 버린 대안

- specta 타입에서 JSON 스키마 자동 생성: `specta-serde`를 직접 의존해야 하고(새 의존성) serde 이름 바꾸기 해석이 불확실하다. 대신 손으로 쓴 스키마를 IPC 타입 역직렬화 시험으로 묶었다.
- 쓰기 도구를 모든 실행에 열기: 예약 실행은 사람이 보지 않으므로 거부한다.
- AI의 주문 도구가 IPC 승인 흐름을 바로 띄우기: 확인 창과 Touch ID가 남아도 AI가 창을 띄우는 시점을 정하게 된다. 요청 알림만 만든다.
- `export_data`의 기본 위치 저장: 같은 IPC를 타서 저장 창이 뜨고 사용자가 위치를 고른다. AI가 조용히 파일을 쓰는 길을 만들지 않는다.

## 남은 일

- 감사 기록 실패는 도구 결과를 바꾸지 않는다(쓰기가 이미 끝난 뒤라). 감사 저장을 실행 앞에 두는 2단계 기록은 하지 않았다.
- `Touch ID 거절`처럼 IPC가 성공(`kind: declined`)으로 돌려준 쓰기도 "바꿨어요" 알림과 감사 기록이 남는다.
- 승인 요청 알림은 채팅 줄과 화면 갱신뿐이다. 신호 카드 강조는 하지 않았다.
