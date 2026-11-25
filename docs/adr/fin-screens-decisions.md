# fin-screens 묶음 결정 기록

경량 절차(ADR 0173)로 `fin-screens` 묶음(자료 없이 남은 화면을 실제 저장소·토스 읽기에 연결)을 구현하며 문서가 정하지 않아 정한 것이다. 작업 ID는 지시의 번호를 따른다(SC-1 종목·관심 종목, SC-2 논거 노트, SC-3 거래내역, SC-4 신호 카드, SC-5 설정 고급).

## 정한 것

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| 위임 | 하네스가 `Agent` 위임을 막아 모든 행을 메인 세션이 차례로 구현한다 | 하위 구현자 병렬 |
| SC-2 | 논거 이력에 시각이 없어 마이그레이션 0024(`thesis_history_times`, 이력 행마다 시각 하나, `thesis_history`를 외래 키로)를 더했다. domain `Thesis`·`ThesisRepository`는 그대로 두고, application `ThesisHistoryTimes` 포트와 장식자 `TimedThesisRepository`(저장이 성공하면 모든 이력 순번에 지금 시각을 `INSERT OR IGNORE`로 남김 → 처음 저장된 시각만 남는다)를 조립 루트가 MCP 논거 도구의 저장소로 끼운다. 스키마 스냅숏·마이그레이션 개수 고정값(23→24, 스키마 항목 217→219)을 갱신했다 | `HistoryEntry`에 시각 필드(도메인·코덱·모든 가짜 변경) |
| SC-2 | 논거 노트 명령 `get_thesis(symbol)`: 주장은 현재 판정된 주장과 검토 대기 중인 추가 주장, 검토 대기 대상이면 `needs-review`. 이력은 최신순으로 변경 종류 코드·대상 주장 문장·판정 코드만 보내고 요약 문장은 `messages.ts`에서 만든다. 마이그레이션 전 이력은 시각이 없어 "시각 기록 전"으로 보인다(화면 타입 `at` 선택값). 노트가 없으면 빈 노트(오류 아님) | Rust에서 요약 문장 생성 |
| SC-4 | 신호 카드는 명령 `get_signals`: 보유·관심 종목마다 신호 저장소 스냅숏(`SignalRepository::snapshot`)의 현재 신호를 읽어(저장소·이벤트 계약은 읽기만, 생성은 fin-flow) 그 신호의 제출 기록(`submission_records`)에서 AI 확률·근거·반대 논거를, `SignalDisplayFacts`에서 실제 적중률 반영 확률·임계값(매수는 매수, 매도는 매도 임계값)을, `sources(task)`에서 출처를, 성적표(`GetScorecard`)에서 같은 보정 키·같은 확률 구간(`CalibrationBin::for_probability`)의 적중률·건수를 붙인다. 백분율 표시는 domain(`display_probability_percent`, `display_scorecard_percent`). 현재가는 토스 시세, 없으면 비운다 | 후보 종목 포함(후보 목록 저장 자리 없음) |
| SC-4 | 제품 경로에 주문 제안(`DraftProposal`)을 만드는 곳이 아직 없어 카드에 제안이 없다. 수량·금액을 지어내지 않고 새 상태 `pending`("주문할 때 최신 호가로 계산")으로 보내며 주문 버튼은 나오지 않는다(`no-order`). 주문 실행 연결은 fin-gaps 몫 | 화면에서 수량 추정, 0주로 표시 |
| SC-4 | 장 상태는 토스 장 일정(`calendar_on`, KST 날짜, 날짜별 메모리 캐시)의 `state_at(now)`가 정규장 열림일 때만 `open`, 일정을 못 읽으면 `closed`(주문 버튼 없음). 주문 스위치는 저장된 위험 설정의 주문 허용 | 일정 실패를 열림으로 봄 |
| SC-4 | 넘기기는 `skip_signal(symbol, reason)`이 `SkipSignal` 유스케이스로 기록(빈 이유는 이유 없음, 여러 줄·200자 초과는 거부)하고, 새 읽기 포트 `SignalSkipHistory::latest_skip`으로 그 신호 제출 시각 이후 넘긴 종목의 카드를 목록에서 뺀다(다음 제출이 오면 다시 보인다). `SqliteRepositories`에 넘기기 기록·읽기 위임을 더했다. `ipc/signals.rs`의 함수 이름 `sources`가 어댑터 crate 이름과 같아 `arch-check`에 걸려 `cited`·`source_facts`로 바꿨다 | 넘긴 뒤에도 카드 유지 |
| SC-5가 | 시스템 설정 명령 7개(`get_account_selection`·`select_account`·`get_principles`·`set_principles`·`backup_now`·`export_data`·`delete_all_data`). 설치 앱은 이제 설정 클라이언트(`coreSettingsClient`)를 넘겨 설정 > 고급(시스템·AI·뉴스·원칙)이 보인다. 아직 연결하지 않은 항목은 기존처럼 "자료 없음"으로 끝난다 | 시스템 섹션만 따로 보이기(App 구조 변경) |
| SC-5가 | 계좌 선택: 화면에는 계좌 키를 보내지 않고 종합매매 계좌 목록의 순번("1", "2"…)과 끝 4자리만 보낸다. 고르면 `AccountSelector::choose`가 종합매매 계좌인지 다시 확인해 선택을 바꾸고 `AccountSelectionChanged`를 낸다. 토스 재연결 때 `select()`는 지금 고른 계좌가 아직 종합매매 계좌면 그대로 둔다(시작 때는 fin-hold의 마지막 동기화 계좌 복원과 이어져 재시작 뒤에도 유지) | 선택 저장용 새 설정 표 |
| SC-5가 | 투자 원칙은 기존 `PrinciplesStore`(SQLite)를 `SqliteRepositories`에 위임으로 붙여 `UpdateInvestmentPrinciples`로 저장. 빈 문장은 원칙 없음, 4000자 초과는 거부 | - |
| SC-5가 | 지금 백업은 매일 백업과 같은 `back_up`(앱 데이터의 `backups`, DB 키로 암호화). 내보내기는 저장 창(rfd)이 없어 같은 암호화 백업을 다운로드 폴더의 `Stock Copilot`에 만든다(이 기기 키로만 열림, 결과 `exported`) | 평문 내보내기(SEC-1 위반), 저장 위치 묻기(새 패키지) |
| SC-5가 | 전체 삭제(U-15, ADR 0176 4절): DB를 닫고 `stock-copilot.db`·`-wal`·`-shm`과 `backups` 폴더를 지운 뒤 키체인 항목 7개(토스 키·토큰, DB 키, 감사 HMAC 키, DART·FRED 키)를 지우고 `StoredDataChanged`를 낸 다음 앱을 다시 시작한다(`request_restart`, 새 키로 첫 실행). 감사 머리값 키체인 항목은 아직 구현되지 않아(S-16) 대상이 없다 | 실행 중 새 DB로 교체 |
| SC-3 | 거래내역은 명령 `get_transactions`가 기존 예약 작업(`TransactionsSyncJob`)이 저장한 첫 계좌의 최신 동기화 기록(`TransactionRepository::latest`)을 읽는다. 정산된 이력 행만 보이고(미정산 행은 정산 뒤 나타남) 취소 행은 뺀다. 사라진 행은 `vanished`로 남긴다. 최신순 500행까지. 금액은 순금액 → 거래금액 → 수수료 순으로 있는 값을 쓴다. 시각은 체결 시각, 없으면 정산일 0시(KST). 종목 이름은 마스터 디렉터리, 없으면 심볼. 현금 행 이름은 통화로 `messages.ts`(원화·달러) | 미정산 행도 표시, 금액 없는 행 제외 |
| SC-1 | 종목 목록은 명령 `get_instruments`: 첫 계좌의 저장된 보유(`GetPortfolioView`)를 먼저, 관심 종목(`ManageWatchlist::list`)을 뒤에 둔다(겹치면 보유). 이름·시장은 종목 마스터 디렉터리에서 찾고, 마스터에 없는 관심 종목은 보이지 않는다. 디렉터리 항목에 `MasterInstrument`를 함께 두어 심볼↔종목 ID를 잇는다 | 관심 종목 표에 심볼·이름 열 추가(마이그레이션) |
| SC-1 | 당일 등락은 domain `previous_session_close`(시세 시각의 날짜를 일봉의 시장 시간대로 구해 그보다 앞선 마지막 무수정 일봉 종가)와 `daily_change`(정확 나눗셈)로 계산한다. 토스 시세(`quotes`)는 목록 전체를 한 번에, 전일 종가는 종목마다 일봉 14일을 받아 같은 날짜 동안 메모리에 캐시한다. 시세·일봉이 없으면 등락을 비우고(화면 "등락 정보 없음") 0으로 지어내지 않는다. 그래서 화면 타입 `todayRate`를 선택값으로 바꿨다 | 등락 없는 행을 목록에서 뺌, 0% 표시 |
| SC-1 | 종목 상세는 `get_instrument_detail(symbol)`: 현재가·시세 시각(없으면 재시도 오류), 등락, 일봉 120일 중 최근 60개(무수정), 저장된 보유의 수량·평가·손익. 관심 종목 추가·삭제는 `add_watchlist`/`remove_watchlist(symbol)`이 `ManageWatchlist`를 거쳐(이벤트 `WatchlistChanged`) 거부를 코드(`unknown-instrument`·`inactive-instrument`·`capacity-exceeded`·`not-in-watchlist`)로 보내고 문장은 `messages.ts`. 이미 있으면 적용으로 본다 | 이미 있음을 거부로 표시 |
| SC-1 | 조립 루트 `main`이 60줄 상한을 넘어 성적표·종목 상태 등록을 `ManageScreens::manage_screens` 한 줄로 묶었다. master의 `platform/scheduling.rs` 변수 이름 유사(`similar_names`)도 이름만 바꿔 고쳤다 | - |
| SC-3 | master에 있던 `ipc/portfolio/tests.rs`의 변수 가림(`clippy::shadow_unrelated`)을 이름만 바꿔 고쳤다(검사 결과 동일) | 그대로 둠(`check-lite` 실패) |
