# fin-gaps 묶음 결정 기록

경량 절차(ADR 0173)로 `fin-gaps` 묶음(설치 앱에서 design.md 기능이 실제로 동작하게 빠진 조립을 채움)을 구현하며 문서가 정하지 않아 정한 것과 조사 결과다.

## 전수 조사 (시작 시점 master `ae18a4fc`)

| 기능 (design) | 전 상태 | 근거 | 담당 |
|---|---|---|---|
| 현황·자산배분·거래내역 (2.4) | 프런트 원본 `pending`(자료 없음), 보유 변환에 종목 마스터 없음 | `App.tsx` `coreScreenSources`, `main.rs` `portfolios: Unavailable`, 빈 `MasterInstrumentFacts` | `fin-hold` |
| 종목 목록·상세 (2.4) | 원본 `pending` | `screen-sources.ts` | 관심 종목은 이 묶음, 보유·마스터는 `fin-hold` |
| 논거 노트 | 원본 `pending` | 같음 | 이 묶음 |
| 신호 카드·넘기기 (6.4) | 원본 `pending`, 목록 조회 IPC 없음 | `registry.rs` | 이 묶음 |
| 주문 승인 요청 | IPC는 있으나 원본 `pending`, `OrderApprovalRuntime` 미조립 → 항상 `unavailable` | `ipc/order_approval.rs`, `main.rs` | 이 묶음 |
| 주문 켜기 (SEC-12) | 위험 변경 확인 창이 `MacOsRiskDialogPresence::unavailable()` → 켤 수 없음 | `main.rs` `presence` | 이 묶음 |
| 요약 창·본인 확인 (4.5) | `UnavailableOrderDialogBackend`·`UnavailableDeviceOwnerAuth`, `rfd`·`objc2-local-authentication` 잠금 파일에 없음 | `platform-macos` | 이 묶음 |
| 주문 쓰기 어댑터 | `broker-toss-trade` 미조립, 승인 포트(`ProposalContextPort`·`ExpiredProposalRedrafter`·`PreSubmitTransactionPort`) 구현 없음 | `approval-command` | 이 묶음 |
| 성적표 (6.6) | 원본 `pending` | `screen-sources.ts` | 이 묶음 |
| 판정 J-12 | `EstimateResolutionJob` 미등록, `ResolutionMarketData` 구현 없음 | `platform/scheduling.rs` | 이 묶음 |
| 장 운영 일정 | 스케줄러·동기화가 `NoMarketCalendar` | `platform/scheduling.rs` | 이 묶음 |
| 종목 추정 순환·유망 종목 발굴·주간 점검 (4.3) | 예약 작업 미등록 | `register_stored_jobs` | 조사 뒤 판단 |
| 자체 점검·상태 띠·고급 > 상태 (2.6) | 원본 `pending` | `screen-sources.ts` | 이 묶음 |
| 위험 편집(투자 성향·주문 설정) | 원본 `pending` | 같음 | 이 묶음 |
| 설정 고급 AI·뉴스·원칙·시스템 (8장) | 실제 조립에 설정 클라이언트 없음(`system="hidden"`), 예약 작업 외 IPC 없음 | `App.tsx`, `settings-client.ts` | 이 묶음 |
| 계좌 선택·백업·내보내기·전체 삭제 | IPC 없음 | `settings-client.ts` | 이 묶음 |
| macOS 알림 (U-14) | `UnavailableNotificationDelivery`, `tauri-plugin-notification` 잠금 파일에 없음 | `main.rs` | 이 묶음 |
| 외부 링크 열기 | `ExternalLinkRuntime`에 `Unavailable` 둘 | `main.rs` `product_builder` | 이 묶음 |
| 미리 띄운 채팅 (C-09) | `warm_chat: Unavailable` | `main.rs` | 조사 뒤 판단 |
| 거시·재무 (FRED·DART) | `macro_series`·`fundamentals: Unavailable` | `main.rs` | 선택 키 기능, 조사 뒤 판단 |
| 채팅·브리핑·뉴스·공시 수집 | 실제 연결됨 | `main.rs` | - |

## 정한 것

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| 위임 | 하네스가 구현 위임(`delegate_task` role=implementation, `Agent`)을 막아 모든 행을 메인 세션이 차례로 구현한다 | 하위 구현자 병렬 |
| G-01 | 위험 편집은 명령 3개(`get_risk_settings`, `save_investor_profile`, `save_order_settings`). 저장은 기존 유스케이스(`UpdateInvestorProfile`, `ChangeRiskSettings`)만 거쳐 SEC-12 확인 창 규칙이 그대로 선다. 화면 상태(`ScreenState`)가 만들던 빈 이벤트 버스를 제품 버스(`Ports.events`)로 바꿔 `SettingsChanged`가 실제 구독자에게 간다 | 빈 버스 유지(설정 변경이 감사·화면 갱신에 안 감) |
| G-01 | 종목 주의 비중은 문서값 10% 고정(편집 화면에 항목 없음), 섹터 제외는 저장값을 그대로 둔다(화면에 항목 없음). 화면의 제외 유형 중 우선주·스팩은 domain `ExcludedSecurityType`에 없어 고르면 한 문장으로 거부한다. ETF는 국내·해외 ETF 둘 다 | 우선주·스팩을 조용히 버림 |
| G-01 | 카드 만료 시간·주문 횟수 상한은 domain·저장소에 자리가 없어(만료 10분 고정, 시간당 상한 고정) 기본값(10분, 끔)만 받고 다른 값은 한 문장으로 거부한다 | 새 설정 표 추가(만료·상한을 쓰는 주문 경로가 아직 값을 읽지 않음) |
| G-02 | 네이티브 위험 확인·주문 요약 창(rfd =0.18.0, ADR 0002 24)은 구현했으나 `arch-check`가 rfd의 Linux·wasm 백엔드 전이(`socket2`·`syn`·`proc-macro2`)를 잡고, 그 예외를 `harness/arch-check/rules.toml`에 더하는 것을 자동 모드가 보호 하네스 변경으로 거부했다. 작업은 로컬 브랜치 `fin-gaps-native-dialog`(1c61b255)에 두고 `fin-gaps`에는 넣지 않았다. 그래서 설치 앱의 "주문 켜기"·위험 완화 저장은 여전히 "확인할 수 없어요"로 끝난다 | 예외 없이 병합(검사 실패), 우회 |
| G-02 | macOS 알림(`tauri-plugin-notification`)과 본인 확인(`objc2-local-authentication`, `unsafe` 예외 필요)도 같은 이유(보호 하네스·`unsafe_code` forbid 예외)로 사용자 승인 몫으로 남긴다 | - |
| G-03 | 자체 점검은 명령 `get_self_check`: 토스 연결 확인(`TossLinkState.check`, 실패 결과는 기존 재시도 창 동안 캐시) + Claude 설치(조립 루트가 `AiAvailability`로 정한 `ClaudeInstall`)를 `evaluate_self_check`에 넣는다. 로그인(5번)·AI 실행 환경(6번)·주문 사전 요건(8번)은 아직 관찰 경로가 없어 정상으로 둔다 | 앱 시작 때 `claude` 로그인 점검 실행(fin-d 4가 버림) |
| G-03 | 고급 > 상태는 명령 `get_status`: 소스별 마지막 성공 시각(`SourceStatusRepository::list_all`), AI 실행 가능 여부, 한도 재개 시각(`AiRunnerQueue::evaluate_chat_quota`의 `WaitRateLimit`). 점검 기록 표와 작업별 AI 사용량은 읽을 저장 자료가 없어 빈 목록(작업 실행 기록은 고급 > AI 예약 작업에 있음). 소스 이름은 코드로 받고 화면 문구는 `messages.ts` | 소스 이름을 Rust에서 문장으로 보냄(화면 문구 규칙 위반) |
