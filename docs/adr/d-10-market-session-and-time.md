# ADR 초안: D-10 장 상태와 도메인 시간 타입

- 상태: 초안
- 날짜: 2026-11-13
- 관련 작업: D-10, 후속 T-01·T-08·J-05·O-01·D-15·D-16
- 기준 커밋: 827fb7325503e577d7fd1c436358e2cff7b949a5

## 근거

ADR 0025의 시간 타입 결정, architecture 3.2·3.7, design 4.3·5.2·6.4를 따른다. 현재 시각은 Clock 포트의 책임이고 장 개폐·휴장·서머타임 규칙을 domain에서 재계산하지 않는다.

확인한 사본은 `docs/toss-openapi-1.2.28.json`이다. R-01의 국내·미국 장 운영 정보 엔드포인트 목록과 대조했다.

| 스키마                                                                            | 확인한 계약                                                                             | 도메인 매핑                                                     |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| KrMarketDay                                                                       | date는 KST 영업일. integrated가 null이면 휴장                                           | CalendarDate와 SessionInformation::Kr. 전체 휴장은 세 구간 None |
| IntegratedHour                                                                    | preMarket·regularMarket·afterMarket 각각 nullable. 모두 null이면 integrated 자체가 null | 각 Option<TimeInterval>. 일부 None은 부분 휴장                  |
| PreMarketSession                                                                  | startTime·endTime은 필수 date-time. singlePriceAuctionStartTime은 nullable              | 프리마켓 구간. 단일가 시작은 이 대분류에 영향을 주지 않음       |
| RegularMarketSession                                                              | KRX·NXT 정규장 합집합, 종가단일가 포함. singlePriceAuctionStartTime nullable            | 정규장 구간. 종목별 거래소 차이는 주문 게이트 책임              |
| AfterMarketSession                                                                | KRX·NXT 애프터 합집합. singlePriceAuctionEndTime nullable                               | 애프터 구간. 단일가 종료는 이 대분류에 영향을 주지 않음         |
| UsMarketDay                                                                       | date는 미국 현지 영업일. 네 세션 각각 nullable, 휴장 시 모두 null                       | KST 날짜로 고치지 않은 CalendarDate와 SessionInformation::Us    |
| UsDayMarketSession·UsPreMarketSession·UsRegularMarketSession·UsAfterMarketSession | 각각 필수 startTime·endTime, date-time                                                  | 데이·프리·정규·애프터 구간                                      |
| KrMarketCalendarResponse·UsMarketCalendarResponse                                 | today·previousBusinessDay·nextBusinessDay 필수                                          | T-08이 판정할 영업일 행을 선택하고 변환                         |

두 엔드포인트 설명은 모든 시각을 KST(+09:00)로 준다고 명시한다. 미국 정규장 예제는 2026-03-25T22:30:00+09:00부터 2026-03-26T05:00:00+09:00까지다. 오프셋 붙은 절대 시점을 비교하면 DST를 별도 계산하지 않아도 된다. 시간대 DB를 추가하지 않는다. America/New_York의 임의 역사 시점 변환 요구가 생기면 구현을 확장하기 전에 별도 선택을 검토한다.

## 결정

- 기존 Cargo.lock의 time =0.3.55를 domain 일반 의존으로 추가한다. default-features=false이며 직접 켠 기능은 없다. parsing·formatting·macros·local-offset·serde·std를 직접 켜지 않는다. 작업 공간의 Tauri 전이 경로는 기존대로 기능을 통합할 수 있다.
- Timestamp는 비공개 OffsetDateTime newtype이다. 오프셋과 나노초를 보존하고 비교·동등성은 절대 시점 기준이다. CalendarDate는 비공개 Date newtype이며 시간대나 시장을 추론하지 않는다. TimeInterval은 비공개 IntervalBounds를 감싼 newtype이고 시작보다 끝이 늦을 때만 생성된다. 빈 구간도 오류다.
- raw-types의 external-types는 비워 둔다. Date와 OffsetDateTime만 newtype-fields에 추가한다. time 타입은 new·as_ 경계에만 노출된다. CalendarDate::as_date는 원시 타입 규칙과 Copy 참조 린트를 함께 지키는 공유 참조다. 일반 공개 시그니처와 세션 필드는 전용 타입만 쓴다.
- MarketSession::try_new는 Market·CalendarDate·SessionInformation을 받는다. Kr/Us와 정보 종류가 다르면 MarketMismatch, Market::Unknown은 UnsupportedMarket다. 세션의 실제 시점 순서가 역전되거나 겹치면 OverlappingOrUnordered다. 인접 구간의 끝과 시작이 같은 것은 허용한다.
- state_at에 판정 시각을 반드시 전달한다. 닫힌 상태 집합은 Unknown·Holiday·BeforeOpen·DayMarket·PreMarket·RegularMarket·AfterMarket·BetweenSessions·Ended다. 구간은 [시작, 끝)이다. 세션 사이 공백은 BetweenSessions, 마지막 세션 종료 정각부터 Ended다. 반일장은 공급된 짧은 종료 시각을 그대로 따른다.
- 응답 자체가 없거나 필수 정보를 확정할 수 없으면 Unavailable → Unknown이다. 성공 응답에서 스펙이 휴장으로 명시한 null만 전체 None → Holiday로 변환한다. 누락·파싱 실패를 휴장으로 바꾸지 않는 책임은 T-08 경계에 있다. optional 필드 생략과 명시 null의 차이를 경계에서 확인한다.
- date는 캘린더 행의 식별 정보다. state_at은 그 행에 대한 상대 상태만 계산하며 해당 행의 유효기간이나 조회 최신성을 추론하지 않는다. 판정 시점과 맞는 영업일 행 선택, 정보 갱신, 여러 행 중 휴장과 이전 영업일의 자정 이후 세션을 대조하는 책임은 T-08·application에 있다. 미국 현지 날짜를 KST 자정으로 생성하지 않는다.
- 주문 가능 여부나 승인·전송 API를 만들지 않는다. 국내 통합 시간에는 장전/장후 시간외종가가 제외된다. O-01은 이 상태에 종목별 운영·주문 조건을 더해 검증해야 한다.
- D-15·D-16 파일은 변경하지 않는다. 후속 시각 필드에 Timestamp, 날짜 필드에 CalendarDate를 사용할 수 있다. chrono 생성물은 T-01 어댑터에서만 변환한다(ADR 0025).

## 달력 사실과 테스트 입력

실제 날짜의 기대값은 구현으로 계산하지 않고 아래 자료에서 확인해 고정했다. API 녹화 자료나 실측 계좌 응답을 확보했다는 뜻은 아니다.

- [NIST DST 규칙](https://www.nist.gov/pml/time-and-frequency-division/popular-links/daylight-saving-time-dst): 2026년 시작은 3월 8일, 종료는 11월 1일이다. 전환은 각각 일요일이며 테스트에서 그날의 휴장 정보를 별도로 넣는다.
- [NYSE 운영 시간과 달력](https://www.nyse.com/markets/hours-calendars): 정규장 09:30~16:00 ET, 2026년 11월 27일은 13:00 ET 조기 종료다.
- KST 고정 입력: 3월 6일 23:30~7일 06:00, 3월 9일 22:30~10일 05:00, 10월 30일 22:30~31일 05:00, 11월 2일 23:30~3일 06:00. 반일장은 11월 27일 23:30~28일 03:00이다. 미국의 표준시·서머타임 정규장을 한국 시각으로 직접 옮긴 상수다.
- 휴장 입력: 스펙 예제의 국내 2026-05-05, 미국 2026-07-03. 국내 부분 휴장은 프리만 None인 스펙 예제를 따른다.
- 시작·끝 정각, 자정, 세션 사이 공백, 역전·빈 구간, 겹침·세션 역순, 시장 불일치, 나노초 구간과 동일 시점의 다른 오프셋을 검증한다. 순수성과 구간 밖 비정규장 속성은 4096회 생성한다.

## 의존성과 검사 경계

새 패키지를 잠금에 추가하지 않는다. time의 최소 일반 전이는 deranged 0.5.9, num-conv 0.2.2, powerfmt 0.2.1, time-core 0.1.9다. 모두 기존 잠금과 deny 허용 목록에 있으므로 deny.toml은 변경하지 않는다. domain 직접 의존 구조 허용 목록에는 time 하나만 추가한다. 작업 공간 기능 통합의 time → serde_core → serde_derive → syn·proc-macro2 경로만 위치 예외로 추가한다. domain은 serde를 직접 켜지 않는다. 원시 타입 허용 목록도 두 newtype 필드만 추가한다.

기존 xtask source-check는 now 접두 호출을 파일 허용 목록 밖에서 거부한다. time now_utc 위반 예제(clock-time-crate.rs), chrono·jiff·SystemTime·Instant 예제가 있고 domain 예외는 없다. 이번 작업은 검사 끄기나 예외를 더하지 않는다.

프로젝트의 .codex/config.toml에는 hooks=true가 있지만 이 T3 세션은 런타임 프로필이 disabled/full-access다. 프로젝트·훅 신뢰가 실제 적용됐다는 증거로 취급하지 않는다. 사용자 홈 설정은 읽거나 변경하지 않고 전체 검사를 직접 실행한다.
