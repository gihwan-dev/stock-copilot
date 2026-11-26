# ADR 0182: 제품 종목 정보는 이상한 행만 건너뛰고, 종목 정보에 없는 보유도 보여 준다

- 상태: 확정 (2026-11-26). v0.4.1 실사용 진단 기록에서 현황이 매번 실패해서 정한다.

## 배경

진단 기록(ADR 0181)에서 세 가지가 보였다.

1. 첫 종목 정보 갱신이 `MasterNotReady(Storage(Repository Unavailable))`로 실패했다. `start_toss_check`가 `run_app` 전에 토스 확인을 띄우는데, Tauri 2는 `setup`(DB 열기)을 `RunEvent::Ready`에서 돌린다. 토스 연결이 DB 설치보다 먼저 끝나 `SqliteRepositories::begin`이 "열리지 않음"을 돌려줬고, 저장된 종목 정보 복원도 빈 DB를 읽었다.
2. 실제 `/stocks/all`·`/stocks` 응답으로 만든 종목 정보가 `Validation`으로 통째로 실패했다. 한 행이라도 기준(빈 ISIN, 시장 규칙에 맞지 않는 심볼, 목록과 상세의 이름 차이, NXT 미지원인데 정지 여부가 `false`, 요청하지 않은 상세 행 등)에 어긋나면 전체를 버렸다.
3. 현황 동기화는 모든 보유가 종목 정보에 있어야 해서, 종목 정보가 없으면 보유 전체가 실패했다.

## 결정

1. 토스 확인 작업은 `database::wait_for_setup`으로 `setup`이 `DatabaseStatus`를 등록할 때까지(최대 30초) 기다린 뒤 저장된 현황 복원과 연결 확인을 한다.
2. `instrument_master`에 행 정책 `RowPolicy`를 둔다. 포트 `BrokerRead::instruments`(`TossInstrumentRead`)는 기존 계약 시험대로 `Strict`이고, 제품 경로 `TossAccountRead::instrument_master`는 `SkipInvalid`다. `SkipInvalid`는 행마다 검증하고, 틀린 행은 건너뛰며 이유별 개수를 `skipped N instrument rows: <이유>`로 기록한다. 상세 응답이 이름·종류·보통주 여부를 다르게 주면 상세 값을 쓰고, 같은 국가의 다른 거래소면 상세 시장을 쓴다. NXT 미지원인데 정지 여부가 오면 미지원으로, NXT 지원인데 정지 여부가 없으면 정지로(보수적으로) 본다. 해외 행의 국내 상세는 버린다. 남은 행이 없으면 빈 종목 정보를 저장하지 않고 실패한다. `Strict`도 실패한 검증 단계를 진단 기록에 남긴다.
3. `TossAccountRead::showing_unmatched_holdings`를 켜면(제품 조립만 켠다) 종목 정보에 없는 보유를 `ResolvedOrderInstrument::reported_by_broker`(시장 심볼 규칙 검증)로 만들고, 보유 응답의 종목명을 `Holding::with_reported_name`으로 붙인다. 이름이 없거나 쓸 수 없으면 비워 두고 화면은 심볼을 보여 준다. 진단 기록에는 개수만 남긴다. 기본값은 기존 시험이 지키는 거절이다.

## 다른 선택지

- 포트 기본 동작을 바꾸기: 보호된 기존 계약 시험(`t21_instruments`, `accounts_holdings`)을 바꿔야 한다.
- 건너뛴 행마다 한 줄 기록: 수천 행 시장에서 기록이 넘친다.
- 보유 이름을 저장 스냅샷에 넣기: 마이그레이션이 필요하고, 다음 동기화가 다시 채운다.
