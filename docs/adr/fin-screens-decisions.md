# fin-screens 묶음 결정 기록

경량 절차(ADR 0173)로 `fin-screens` 묶음(자료 없이 남은 화면을 실제 저장소·토스 읽기에 연결)을 구현하며 문서가 정하지 않아 정한 것이다. 작업 ID는 지시의 번호를 따른다(SC-1 종목·관심 종목, SC-2 논거 노트, SC-3 거래내역, SC-4 신호 카드, SC-5 설정 고급).

## 정한 것

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| 위임 | 하네스가 `Agent` 위임을 막아 모든 행을 메인 세션이 차례로 구현한다 | 하위 구현자 병렬 |
| SC-2 | 논거 이력에 시각이 없어 마이그레이션 0024(`thesis_history_times`, 이력 행마다 시각 하나, `thesis_history`를 외래 키로)를 더했다. domain `Thesis`·`ThesisRepository`는 그대로 두고, application `ThesisHistoryTimes` 포트와 장식자 `TimedThesisRepository`(저장이 성공하면 모든 이력 순번에 지금 시각을 `INSERT OR IGNORE`로 남김 → 처음 저장된 시각만 남는다)를 조립 루트가 MCP 논거 도구의 저장소로 끼운다. 스키마 스냅숏·마이그레이션 개수 고정값(23→24, 스키마 항목 217→219)을 갱신했다 | `HistoryEntry`에 시각 필드(도메인·코덱·모든 가짜 변경) |
| SC-2 | 논거 노트 명령 `get_thesis(symbol)`: 주장은 현재 판정된 주장과 검토 대기 중인 추가 주장, 검토 대기 대상이면 `needs-review`. 이력은 최신순으로 변경 종류 코드·대상 주장 문장·판정 코드만 보내고 요약 문장은 `messages.ts`에서 만든다. 마이그레이션 전 이력은 시각이 없어 "시각 기록 전"으로 보인다(화면 타입 `at` 선택값). 노트가 없으면 빈 노트(오류 아님) | Rust에서 요약 문장 생성 |
| SC-3 | 거래내역은 명령 `get_transactions`가 기존 예약 작업(`TransactionsSyncJob`)이 저장한 첫 계좌의 최신 동기화 기록(`TransactionRepository::latest`)을 읽는다. 정산된 이력 행만 보이고(미정산 행은 정산 뒤 나타남) 취소 행은 뺀다. 사라진 행은 `vanished`로 남긴다. 최신순 500행까지. 금액은 순금액 → 거래금액 → 수수료 순으로 있는 값을 쓴다. 시각은 체결 시각, 없으면 정산일 0시(KST). 종목 이름은 마스터 디렉터리, 없으면 심볼. 현금 행 이름은 통화로 `messages.ts`(원화·달러) | 미정산 행도 표시, 금액 없는 행 제외 |
| SC-1 | 종목 목록은 명령 `get_instruments`: 첫 계좌의 저장된 보유(`GetPortfolioView`)를 먼저, 관심 종목(`ManageWatchlist::list`)을 뒤에 둔다(겹치면 보유). 이름·시장은 종목 마스터 디렉터리에서 찾고, 마스터에 없는 관심 종목은 보이지 않는다. 디렉터리 항목에 `MasterInstrument`를 함께 두어 심볼↔종목 ID를 잇는다 | 관심 종목 표에 심볼·이름 열 추가(마이그레이션) |
| SC-1 | 당일 등락은 domain `previous_session_close`(시세 시각의 날짜를 일봉의 시장 시간대로 구해 그보다 앞선 마지막 무수정 일봉 종가)와 `daily_change`(정확 나눗셈)로 계산한다. 토스 시세(`quotes`)는 목록 전체를 한 번에, 전일 종가는 종목마다 일봉 14일을 받아 같은 날짜 동안 메모리에 캐시한다. 시세·일봉이 없으면 등락을 비우고(화면 "등락 정보 없음") 0으로 지어내지 않는다. 그래서 화면 타입 `todayRate`를 선택값으로 바꿨다 | 등락 없는 행을 목록에서 뺌, 0% 표시 |
| SC-1 | 종목 상세는 `get_instrument_detail(symbol)`: 현재가·시세 시각(없으면 재시도 오류), 등락, 일봉 120일 중 최근 60개(무수정), 저장된 보유의 수량·평가·손익. 관심 종목 추가·삭제는 `add_watchlist`/`remove_watchlist(symbol)`이 `ManageWatchlist`를 거쳐(이벤트 `WatchlistChanged`) 거부를 코드(`unknown-instrument`·`inactive-instrument`·`capacity-exceeded`·`not-in-watchlist`)로 보내고 문장은 `messages.ts`. 이미 있으면 적용으로 본다 | 이미 있음을 거부로 표시 |
| SC-1 | 조립 루트 `main`이 60줄 상한을 넘어 성적표·종목 상태 등록을 `ManageScreens::manage_screens` 한 줄로 묶었다. master의 `platform/scheduling.rs` 변수 이름 유사(`similar_names`)도 이름만 바꿔 고쳤다 | - |
| SC-3 | master에 있던 `ipc/portfolio/tests.rs`의 변수 가림(`clippy::shadow_unrelated`)을 이름만 바꿔 고쳤다(검사 결과 동일) | 그대로 둠(`check-lite` 실패) |
