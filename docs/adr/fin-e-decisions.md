# ADR: fin-e 묶음 결정 표

- 상태: 확정 (ADR 0173 경량 절차, 2026-11-25)

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| 뉴스 수집 워커 연결 | application `CollectFeed`가 피드 한 번의 수집(`new_items` → 뉴스는 `NewsRepository`, 공시는 `DisclosureRepository`에 저장 → 공시마다 `DisclosureArrived`, 끝나면 `CollectionFinished` → `RecordSourceStatus`로 소스 상태 기록)을 맡고 다음 커서를 돌려줌. 소스 실패·키 없음·자료 없음은 커서를 그대로 두고 상태만 기록, 저장 실패는 커서를 넘기지 않고 오류로 끝냄. src-tauri `FeedCollectorWorker`가 30초 틱마다 `PlanCollectionSchedule`(N-11 표, 장 상태는 `MarketInfo`, 실패하면 장 밖 표)로 각 피드의 주기(`collection_due`)를 판정해 돌림. 커서·마지막 시도 시각은 메모리에만 두어 앱 시작 때 처음부터(`FeedCursor::Start`) 수집. KIND는 첫 회만 하루치 따라잡기 창(`StartupCatchUp`), 이후 정규 창. 조립 루트는 키가 필요 없는 언론사·거시 RSS 6종과 KIND 공시만 등록(DART·SEC·Google 뉴스·Yahoo는 키·연락처·종목 목록이 필요해 등록하지 않음). DART 키는 없음으로 고정. DB가 열리지 않았으면 요청을 보내지 않음. 피드 타입이 제각각이라 `SharedFeed`(형 지움)로 묶고 `Ports.news_feeds`는 그대로 둠 | 커서를 DB에 저장(새 표 필요), 모든 소스 등록, 스케줄러 작업으로 등록(작업 ID 추가 필요) |
