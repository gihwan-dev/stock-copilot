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
| G-04 | 성적표는 명령 `get_scorecard`가 `GetScorecard`(MCP `get_scorecard`와 같은 저장소·계산)를 부른다. 판정 기록이 없으면 빈 목록(오류 아님), 계산 실패는 재시도 오류. 신호 수익률·벤치마크·신뢰구간은 domain 계산은 있으나 성적표 관측(`ScorecardObservation`)에 가상 청산·벤치마크 입력이 없어 비워 둔다(화면은 "아직 계산되지 않았어요"). 자료 없는 구간은 표에서 뺀다. 기간·구간 이름은 코드로 받아 `messages.ts` | 가상 청산 입력을 새로 모음(J-12 판정 자료가 먼저 필요) |
| G-05 | 배포 앱(Finder 실행)은 프로세스 PATH가 `/usr/bin:/bin:/usr/sbin:/sbin`뿐이라, 로그인 셸 PATH로 `claude`를 찾아도 자식 프로세스에 최소 PATH가 넘어가 npm 설치본(`#!/usr/bin/env node`)이 `node`를 못 찾는다. 조립 루트가 자식 환경 PATH를 "실행 파일 폴더 → 로그인 셸 PATH → 프로세스 PATH"(중복·빈 항목 제거)로 만든다. 다른 허용 키(HOME·USER·LOGNAME·TMPDIR)와 API 키 거부는 그대로. 브리지는 실행 파일 옆 `mcp-bridge`(릴리스 스크립트가 `Contents/MacOS`에 복사)에서 찾는 것을 시험으로 고정했다 | 로그인 셸 PATH만 사용(셸 설정이 PATH를 지우면 시스템 경로가 빠짐) |
| G-06 | 외부 링크 열기: 카탈로그는 `SqliteRepositories`가 수집 항목 표(`information_items.url`)에서 요청 주소의 정규 https 형태와 정확히 같은 것만 찾는다(`KNOWN_URL`). 고정 주소(학습 설정 등)는 기존 `authorize_fixed`. 열기는 조립 루트의 `/usr/bin/open <정규 https 주소>`(인자 하나, `spawn_blocking`). 웹 수집 결과(브리핑 `web` 출처)는 URL이 표에 없어 열리지 않는다 | 웹뷰 `shell` 플러그인(권한 확대), 호스트 허용 목록만으로 열기 |
| 수정(메인 지시) | master `check-lite` 실패 정리: (1) `ipc/self_check.rs`의 지역 변수·필드 이름 `sources`가 어댑터 crate 이름과 같아 `adapter-construction`에 잡혀 `statuses`로 바꾸고 시험의 `broker_toss` 시각 도우미를 domain 생성으로 바꿈, (2) 조립 루트의 `std::iter::once`가 웹뷰 API 이름 `once`로 잡혀 배열 `chain`으로 바꿈, (3) 위험 편집·상태·성적표 변환 파일을 원시 타입 허용 목록에서 빼고 공용 `settleWithSchema`/`WireOutcome`(브리핑 선례)로 바꿈. FD-5가 넣은 세 파일의 고정 시험 항목·위반 예제도 되돌림 | 허용 목록 유지 |
| G-07 (J-12) | 판정 작업 두 개(국내 `EstimateResolution`, 미국 `UsEstimateResolution`, native 묶음이 정한 일정)를 등록하고 `ResolveEstimates`를 제품 이벤트 버스로 부른다(`ResolveEstimatesRunner`는 버스를 값으로 받아 쓰지 않음). 장 일정은 토스 `market-calendar?date=`(새 `TossAccountRead::calendar_on`)를 기준일부터 영업일마다 이어 받아 메모리에 캐시한다(처음 8일, 부족하면 20일씩, 최대 140일) | 오늘 일정만으로 판정(fin-e가 버림) |
| G-07 (J-12) | 기업 행위 자료 원천이 없어, 수정주가 일봉과 무수정 일봉을 둘 다 받아 한 봉이라도 다르면(분할 등) 판정하지 않고 `Incomplete(Adjustment)`로 미룬다(판정 불가 기록은 별도 로그 표라 다음 실행에 다시 시도). 같으면 기업 행위 없음으로 `OutcomeAdjustment::from_actions(.., &[])`. 기준일 분봉은 늘 받고, 판정이 분봉 부족이면 창 안의 다음 장 최대 10일 분봉을 더 받는다. 경계가·비율 계산은 domain에만 있다 | 기업 행위 없음으로 가정(분할 뒤 즉시 손절 오판), 전 기간 분봉 |
| G-07 (J-12) | 이 묶음 시점에는 제품에서 추정 과제를 만드는 경로(종목 추정 순환 E-05)가 아직 없어, 판정 작업은 채팅·장중 감시에서 제출된 추정이 있을 때만 일한다 | - |

## 남은 것 (묶음 끝)

| 기능 | 지금 상태 | 막힌 이유·다음에 할 것 |
|---|---|---|
| 주문 켜기·위험 완화 확인 창, 주문 요약 창 | 구현은 로컬 브랜치 `fin-gaps-native-dialog`(1c61b255)에만 있음 | `harness/arch-check/rules.toml`에 rfd 전이 예외 한 항목이 필요(보호 하네스, 자동 모드가 거부) — 사용자 승인 |
| 본인 확인(Touch ID) | `UnavailableDeviceOwnerAuth` | `objc2-local-authentication`과 `platform-macos`의 `unsafe_code` 예외 ADR(O-07)이 사용자 몫 |
| macOS 알림 | `UnavailableNotificationDelivery` | `tauri-plugin-notification` 추가와 구조 검사 예외가 보호 하네스 변경 |
| 주문 승인 런타임 | `OrderApprovalRuntime` 미조립(요청은 항상 `unavailable`) | `ProposalContextPort`·`ExpiredProposalRedrafter`·`PreSubmitTransactionPort` 구현과 `broker-toss-trade` 조립이 필요(등급 3), 위 확인 창이 먼저 |
| 추정 → 신호 → 제안 사슬 | 추정 과제 생성(`CreateEstimateTask`)·종목 추정 순환(E-05)·신호 평가(`EvaluateSignals`)·제안 초안이 제품 경로에서 불리지 않음 | `SignalEvaluationData` 등 자료 포트 구현이 큼. 다음 묶음 |
| 신호 카드·넘기기·종목 목록·종목 상세·논거 노트·관심 추가/삭제·거래내역 | 화면 원본 `pending` | 신호는 위 사슬 뒤, 종목 목록은 종목별 당일 등락 자료 경로가 없음, 논거 이력은 시각이 저장되지 않음 |
| 설정 고급(AI·뉴스·원칙·계좌 선택·백업·내보내기·전체 삭제) | 설치 앱에 숨김 | IPC 19개, 다음 묶음 |
| 미리 띄운 채팅·거시·재무 | `Unavailable` | 선택 기능, 다음 묶음 |
