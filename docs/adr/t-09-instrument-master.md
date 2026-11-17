# T-09 종목 마스터와 섹터
- 상태: 초안

## 결정

섹터 카탈로그, 상세, 구성 종목, ETF는 기존 TokenManager와 SECTOR 그룹을 통해 조회하고 domain 생성자로 검증한다. 응답이 과도하게 크거나 필수 필드·계층·시장·식별자가 맞지 않으면 전체 실패한다. 서버 시각이 없는 응답에는 수집 시각만 남긴다. 구성 종목의 거래대금과 시가총액은 미국 종목이어도 명세상 KRW다.

종목 전체 조회는 미완료다. ListedStock과 StockInfo에는 안정적인 내부 InstrumentId가 없고 ADR 0022는 심볼·ISIN으로 내부 ID를 만들지 못하게 하며 발급·유일성 정책이 미정이다. 이미 주입된 마스터의 같은 심볼만으로는 재사용·코드 변경 뒤 같은 종목인지 증명할 수 없다. 또한 status 생략 시 서버의 기본값이 ACTIVE이므로 필터 없는 요청이 모든 상장 상태를 포함한다고 단정하지 않는다. 임의 ID나 완전한 목록을 만들지 않는다. BrokerRead::instruments는 기존 닫힌 미지원 상태를 유지한다.

## 실측 대기

섹터별 실제 데이터의 필드와 크기, 저장 후 갱신 시점은 합성 녹화로 확인할 수 없다. 종목 식별의 발급과 거래소별 status 범위는 실측만이 아니라 domain·저장소 정책 결정이 필요하다.

## 인계

- D-03/S-04/T-09: 심볼 재사용과 코드 변경에서도 동일 종목을 식별할 발급·영속 매핑과 완전 목록 계약을 정의한 뒤 해당 포트를 연결한다. 토스가 제공하지 않는 내부 ID를 추측하지 않는다.
- T-09: STOCK_ALL 경로를 실제 종목 조회에 연결하고 가짜 시계로 요청 간격을 검증한다. 기존 제한기 그룹의 존재만으로 해당 경로가 작동한다고 주장하지 않는다.

## 합성 계약과 검사

| 엔드포인트 | 포트 메서드 | 녹화 |
|---|---|---|
| GET /api/v1/sectors | MarketInfo::sectors | t09-sectors.json |
| GET /api/v1/sectors/{sectorId} | MarketInfo::sector | t09-sector.json |
| GET /api/v1/sectors/{sectorId}/stocks | MarketInfo::sector_composition | t09-sector-stocks.json |
| GET /api/v1/sectors/{sectorId}/etfs | MarketInfo::sector_etfs | t09-sector-etfs.json |
| GET /api/v1/stocks/all | 미연결 BrokerRead::instruments, 제한기 직접 시험 | t09-stocks-all.json |

섹터 ID는 HTTP 경계의 1~40자리 영숫자·밑줄·하이픈만 허용한다. 서버 목록과 상세의 ID 및 부모 ID도 같은 규칙으로 검사하고 유효하지 않으면 응답 오류로 닫는다. 섹터 행은 8192건까지 허용한다. ETF는 주입된 마스터에서 요청 시장과 심볼이 일치하고 ETF 유형이 확인돼야 한다. 종목 목록 전체를 생성할 근거가 없는 상태라 이 마스터의 실제 공급은 완료되지 않았다. 형식 오류 및 각 섹터 경로의 401 재발급·429 재시도는 합성 응답으로 확인한다.

제한기 시험은 두 번째 요청을 서버가 받은 순간의 가짜 시계를 읽는다. 대조군은 성공했고 별도 mktemp 복사본에서 STOCK_ALL 그룹 한도를 1에서 2로 바꾸거나 제한기 대기를 실제 송신 뒤로 옮긴 두 변형 모두 컴파일 후 stock_all_limiter_delays_second_request_at_least_one_second에서 실패했다. 실 API 속도 측정이나 BrokerRead::instruments의 연결 검증은 아니다.

## 보호 파일과 세션

harness/arch-check/rules.toml에는 합성 녹화를 재생하는 tests/sector_market.rs만 추가한다. ADR 등록에 따른 설정·해시는 저장소 생성기로 만든다. 프로젝트 설정 hooks=true와 Git 훅 설치는 확인했지만 현재 세션 프로젝트 신뢰·훅 신뢰 및 적용은 확인 불가다. 브리프를 읽는 명령과 병렬로 금지된 홈 MEMORY.md 검색이 최초 1회 실행됐다. 결과는 구현 근거로 쓰지 않았고 이후 홈 파일은 읽지 않았다.

최초 정상 커밋 훅 진행 중 응답 ID 검증 누락을 발견해 해당 커밋 프로세스만 중단했다. 섹터 목록의 경로 불가능 ID가 성공하는 red를 확인한 뒤 검사와 회귀를 더했다. 두 번째 훅 진행 중 제한기 시험이 응답 뒤 시각만 확인하는 한계를 발견해 해당 프로세스를 중단하고 서버 수신 시각 검사로 보강했다. 두 중단된 훅은 통과로 세지 않고 정상 훅을 다시 실행한다.
