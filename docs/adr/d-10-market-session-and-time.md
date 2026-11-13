# ADR 초안: D-10 장 캘린더와 도메인 시간 타입

- 상태: 초안, 적대적 리뷰 수정 반영
- 최초 작성: 2026-11-13
- 수정: 2026-11-14
- 관련 작업: D-10, 후속 T-01·T-08·J-05·O-01·D-15·D-16
- 최초 기준: 827fb7325503e577d7fd1c436358e2cff7b949a5
- 수정 기준: 9026dd943ed104ab8f35dc786695127606164333

## 근거와 범위

ADR 0025, architecture 3.2·3.7, design 4.3·6.3·6.4·6.5를 따른다. 시각은 Clock 포트에서 받아 인자로 전달한다. 서머타임과 휴장일을 자체 계산하지 않는다. 실제 토스 응답 녹화는 T-08의 범위로 남긴다.

확인한 사본은 `docs/toss-openapi-1.2.28.json`이며 R-01 엔드포인트 목록과 대조했다. 국내·미국 응답은 모두 `previousBusinessDay`, `today`, `nextBusinessDay` 세 행을 제공한다. 국내 date는 KST 영업일, 미국 date는 미국 현지 영업일이다. 양쪽 엔드포인트는 시각을 KST(+09:00)로 준다고 명시한다. 미국 정규장 예제는 2026-03-25T22:30:00+09:00부터 2026-03-26T05:00:00+09:00까지다.

국내 IntegratedHour는 프리·정규·애프터 세션 각각 nullable이고 전체 휴장은 integrated=null이다. 미국 UsMarketDay는 데이·프리·정규·애프터 각각 nullable이고 전체 휴장은 모두 null이다. 각 세션 startTime·endTime은 필수 date-time이다. 국내 프리·정규에는 singlePriceAuctionStartTime, 애프터에는 singlePriceAuctionEndTime이 선택적으로 있다. 정규·애프터는 KRX·NXT 합집합이며 국내 장전/장후 시간외종가는 제외된다.

## 세 행으로 판정하는 캘린더

- MarketSession은 날짜와 SessionInformation으로 생성한다. 시장은 Kr/Us 변형에서 유도한다. 별도 Market 인자를 받지 않아 모순된 조합을 만들 수 없다. 단일 행의 공개 상태 판정 함수는 제거한다.
- MarketCalendar는 토스 응답의 세 행을 모두 받는다. 날짜는 previous < today < next여야 하고, 시장이 같아야 한다. 각 행과 전체 캘린더는 실제 시작 시각으로 정렬하고 구간 겹침을 거부한다. 필드 나열 순서를 시간 순서로 가정하지 않는다.
- state_at은 모든 세션 중 판정 시각을 포함하는 구간을 직접 고른다. 세션은 [시작, 끝)이다. 미국 현지 오늘이나 KST 오늘에 해당하는 행을 먼저 고르지 않는다. 미국 영업일 D의 데이마켓이 현지 D-1 저녁에 열리는 경우도 절대 시각으로 찾는다.
- 확인 범위는 세 행 전체에서 얻은 첫 실제 세션 시작 이상, 마지막 실제 세션 종료 미만이다. 그 바깥은 Unknown이다. 처음이나 마지막 행이 휴장이면 그 행의 날짜를 임의의 경계 시각으로 만들지 않고 실제 세션들까지 범위를 줄인다. 세 행이 모두 비어 있으면 확인 가능한 시작·끝이 없어 모든 시각에서 Unknown이다.
- 확인 범위 안의 모든 세션 밖은 Closed다. 여기에는 세션 사이 공백, 밤, 주말, 명시된 휴장 행이 포함된다. 미국 현지 날짜를 시간대 DB 없이 추정해 Holiday나 Ended를 구분하지 않는다. 닫힌 상태 집합은 Unknown·Closed·DayMarket·PreMarket·RegularMarket·AfterMarket이다. 주문 게이트는 Unknown과 Closed에서 장 운영 근거를 얻을 수 없다.
- 예: 3/24·3/25·3/26 미국 행에서 3/25 10:00 KST는 3/25 데이마켓, 3/25 04:00 KST는 3/24 정규장이다. 3/18·3/19·3/20 응답으로 3/25 23:00을 판정하면 Unknown이다. 휴장 행 하나로 임의의 먼 시각을 Holiday로 판정하는 경로는 없다.
- 응답 자체가 없으면 캘린더를 만들지 않는다. T-08은 누락·파싱 실패를 명시적 휴장 null로 바꾸지 않는다. 이 타입은 응답 획득 시각이나 데이터 출처를 인증하지 않으며, 확인 범위 안의 응답 최신성 정책은 application에서 추가한다.

## 세션 검증과 조회

- TimeInterval은 끝이 시작보다 늦아야 한다. 세션 하나는 최대 24시간이다. 이는 서버의 모든 장 운영 시간을 하드코딩하는 대신 5일짜리 세션 같은 오류를 막는 보수적 입력 제한이다. 정확히 24시간은 허용하고 1나노초 초과부터 거부한다. 향후 공식 단일 세션이 24시간을 넘으면 재검토한다.
- 국내는 시작을 KST로 투영한 날짜가 행 date와 같아야 한다. 미국은 시작과 끝을 KST로 투영한 날짜가 date의 전날부터 이틀 뒤까지여야 한다. 이 범위는 현지 날짜 정합의 증명이 아니라 심하게 잘못된 날짜를 거부하는 보수적 제한이다. 미국 지역 날짜를 정확히 재계산하지 않는다. 끝 시각도 독립적으로 검사한다.
- SessionData는 구간과 선택적 AuctionBoundary를 보존한다. StartsAt/EndsAt 시각은 시작 이상·끝 이하여야 한다. 국내 프리·정규는 StartsAt, 애프터는 EndsAt만 허용한다. 미국 세션에 국내 단일가 표식을 넣을 수 없다. 값은 세션 접근자로 주문 게이트에 전달한다.
- 단일가 시각 때문에 대분류 상태를 나누지는 않는다. design 6.3·6.5와 O-01은 종목별 가격·주문 조건 검증을 요구하지만 이 캘린더가 단일가별 주문 허용표를 정할 근거는 없다. O-01·T-08이 보존한 표식을 세부 주문 조건과 대조한다.
- 행·캘린더의 sessions 접근자는 정렬된 상태·구간·단일가 표식을 제공한다. J-05는 RegularMarket 구간 시작에서 40분을 checked_sub하여 예약 시각을 계산할 수 있다.
- next_transition(after)는 after보다 엄격히 뒤에 있는 첫 실제 상태 변화의 시각과 새 상태를 돌려준다. 인접 세션은 동일한 경계에서 다음 상태로 바뀐다. 같은 상태가 인접해 이어지는 경계는 건너뛴다. 마지막 종료 시각의 새 상태는 Unknown이다. 단일가 표식은 상태 전이에 포함하지 않는다. 후보가 없으면 None이다.

## 공개 시간 API와 범위

- 기존 잠금 time =0.3.55, default-features=false, 직접 켠 기능 없음은 유지한다. 시간대 DB는 추가하지 않는다. 오프셋 붙은 서버 시각만 비교하므로 DST 계산이 필요하지 않다. 임의 역사 시점의 America/New_York 변환이 필요해지면 구현 전에 별도 선택을 검토한다.
- 공개 생성자·접근자는 time 타입을 받거나 반환하지 않는다. Timestamp::from_parts는 UnixSeconds·Nanosecond·OffsetSeconds, CalendarDate::from_parts는 CalendarYear·MonthNumber·DayNumber를 받는다. 이 구성 요소들은 미검증 스칼라 newtype이며 최종 타입 생성 때 범위를 검증한다. raw-types의 Date·OffsetDateTime·SignedDuration 허용은 내부 비공개 필드용이다. external-types는 여전히 비어 있다.
- 날짜와 시각의 UTC·오프셋 기준 연도를 모두 0001~9999로 제한한다. API의 date/date-time을 양의 4자리 그레고리력으로 다루고, 거래 자료에 필요하지 않은 기원전·0년을 받지 않는 제품 경계다. time의 넓은 내부 범위를 공개 계약으로 삼지 않는다. 실제 존재하지 않는 월·일은 거부한다.
- 오프셋은 분 단위이며 −23:59~+23:59(−86340~+86340초)다. RFC 3339의 time-numoffset은 시·분이고 시 범위는 00~23이다. 초 단위 오프셋과 +25:59:59는 거부한다. 나노초는 0~999999999다. 오프셋 투영 또는 checked 산술 후 UTC·오프셋 날짜가 범위를 벗어나도 오류다.
- utc_seconds·nanosecond는 절대 시점의 구성 요소다. offset_seconds·offset_calendar_date는 보존한 오프셋 기준의 표현이다. date_at_offset은 명시한 다른 오프셋 기준 날짜를 준다. 같은 절대 시점은 오프셋이 달라도 ==지만 offset_calendar_date는 다를 수 있다. 접근자 이름으로 이 차이를 드러낸다.
- TimeSpan은 음수 없는 기간이다. SpanSeconds와 Nanosecond로 만들며 40분 전 예약은 음수 기간 대신 Timestamp::checked_sub를 쓴다. Timestamp의 checked_add·checked_sub와 elapsed_until은 넘침·음수 차이를 오류로 돌려준다. time의 panic 연산자를 쓰지 않는다. 기존 forbid arithmetic_side_effects가 OffsetDateTime + SignedDuration 변형을 거부하는지도 검증한다.
- UnixSeconds는 POSIX 초다. 윤초 23:59:60과 RFC 3339의 의미상 알 수 없는 오프셋 -00:00 처리는 T-01 파싱 경계의 책임이다. 윤초는 domain으로 넘기기 전에 오류로 거부하고 다음 초로 묵시적으로 정규화하지 않는다. 나노초 1e9 이상으로 윤초를 우회 표현할 수도 없다. chrono 생성물은 어댑터에서 검증한 구성 요소로만 변환한다.
- D-15·D-16의 후속 시각 필드에 이 타입을 쓸 수 있지만 이번에는 해당 코드를 수정하지 않는다.

## 달력 사실과 검증

- [NIST DST 규칙](https://www.nist.gov/pml/time-and-frequency-division/popular-links/daylight-saving-time-dst): 2026년 시작 3월 8일, 종료 11월 1일. 전환 전 금요일과 다음 월요일의 실제 개장·종료 값을 테스트에 고정한다. 일요일은 비어 있는 행의 항진명제가 아니라 금요일부터 월요일까지의 확인된 공백으로 검증한다.
- [NYSE 운영 시간·달력](https://www.nyse.com/markets/hours-calendars): 정규장 09:30~16:00 ET, 2026년 11월 27일 13:00 ET 조기 종료. KST 입력은 3/6 23:30~3/7 06:00, 3/9 22:30~3/10 05:00, 10/30 22:30~10/31 05:00, 11/2 23:30~11/3 06:00, 반일장 11/27 23:30~11/28 03:00이다.
- [RFC 3339 5.6](https://www.rfc-editor.org/rfc/rfc3339#section-5.6)의 숫자 오프셋·날짜 구성 요소를 경계 범위 근거로 삼는다.
- 속성 테스트는 각각 4096회다. 오프셋·나노초·세 행의 12개 세션·0 또는 양수 공백을 생성하고, 생성 원시 UTC 나노초 구간 목록을 직접 선형 탐색하는 독립 기준과 대조한다. 시작 직전·시작·끝 직전·끝을 대조하고, 다음 전이를 따라가 모든 경계를 통과하며 실제 상태가 바뀌는지 검증한다.
- 수동 변형으로 행 선택 조건 반전, 범위 밖 Ended 반환, 다음 인접 전이 생략, 산술 넘침 무시를 주입해 회귀 테스트가 실패해야 한다. panic 연산자는 린트로, time·rust_decimal의 serde 직접 활성화는 external-feature로 거부해야 한다.

## 의존성과 하네스

새 의존성·기능·잠금 패키지는 없다. 최소 일반 전이는 deranged 0.5.9, num-conv 0.2.2, powerfmt 0.2.1, time-core 0.1.9다. 기존 time → serde_core → serde_derive → syn·proc-macro2 위치 예외는 작업 공간 기능 통합 경로에만 적용한다.

feature-allowlist로 domain의 time과 rust_decimal에 default-features=off·features=[]를 강제한다. 기본 기능과 serde를 켠 새 예제 두 개가 external-feature를 확인한다. 기존 정상 예제의 rust_decimal 선언 두 곳에도 default-features=false를 명시해 새 규칙에 맞춘다. 기존 테스트 함수나 기대 지적을 약하게 바꾸지 않는다. raw-types에는 SignedDuration 비공개 필드만 추가한다. 검사 끄기는 추가하지 않는다.

Codex 프로젝트·훅 신뢰의 실제 적용 증거는 확보하지 않았으므로 강제 적용으로 보고하지 않는다. 사용자 홈 설정은 읽거나 변경하지 않고 전체 검사와 Git 커밋 훅 결과를 확인한다.
