# D-33: 종목 마스터 사실과 주문 게이트 경계

- 상태: 초안
- 최초 구현: efbf18c53d51048654beeb5bddd08c9b2f0a3b79
- 수정 회차 병합 기준: 로컬 master 792a35b5da51e9a8fff146793a172d1c2e210fc1 (D-24, DS-03, H-43)

## 입력과 봉인

T-09가 StockInfo 상세 행과 안정적 InstrumentId, 조회 시각을 MasterInstrumentRow에 넣고 MasterInstrument::try_new를 호출한다. 결과의 필드는 비공개이고 읽기 전용 접근자만 있다. 생성자는 실제 응답의 진위나 내부 ID의 실제 대응을 인증하지 않는다. 호출 위치를 T-09로 제한하는 것은 하네스 책임이다. InstrumentId는 ADR 0022대로 심볼·ISIN에서 유도하지 않는다.

| StockInfo 필드        | 도메인 계약                                                                                                                                                       |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| market                | 상장 시장 원문 보존. KOSPI/KOSDAQ/KR_ETC → Kr, NYSE/NASDAQ/AMEX/US_ETC → Us. 미지 시장 원문도 보존하지만 시세·주문 식별 생성은 거부                               |
| symbol                | 대문자 정규형만 수용. 소문자를 정규화하는 것은 ADR 0022의 어댑터 책임. 국내 ASCII 영숫자 6자리, 미국 영문자 시작의 기존 Symbol 문법                               |
| name                  | D-32 DisplayInstrumentName의 문자 제한·256바이트 상한 재사용                                                                                                      |
| securityType          | 스키마 아홉 유형과 원문 코드의 Unknown. 미지 값을 Stock으로 치환하지 않음                                                                                         |
| currency              | 국내 KRW·미국 USD 일치 검증. QuoteInstrument에도 보존                                                                                                             |
| status, isCommonShare | ListingStatus와 CommonShare 열거형                                                                                                                                |
| koreanMarketDetail    | 정리매매, KRX 정지, NXT 지원·정지 사실. NxtTrading은 Unsupported 또는 Supported(TradingSuspension)만 표현. try_from_parts는 미지원+정지 값, 지원+null을 모두 거부 |
| 내부 ID, 조회 시각    | 스키마 필드 없음. 안정적 InstrumentId와 Clock의 Timestamp                                                                                                         |
| 섹터                  | 종목 마스터 행 StockInfo에는 없음. /api/v1/sectors/{sectorId}/stocks가 별도로 있음. D-37·T-09의 별도 조회와 D-06 SectorAssignments가 담당                         |

/stocks/all의 ListedStock은 심볼·이름·유형·보통주 여부·ISIN만 준다. 통화·상장 상태·정지 여부를 발명하지 않고 /stocks 상세 행을 보완한 뒤 마스터를 만든다. R-01의 STOCK_ALL 초당 1회 한도는 T-09 책임이다. 같은 목록의 중복 내부 ID와 중복 (시장, 심볼)을 거부한다. AAPL과 aapl의 동시 등록은 소문자 행 생성 단계에서 거부한다.

englishName, isinCode, listDate, delistDate, sharesOutstanding는 이번 소비 계약에서 저장하지 않는다. leverageFactor도 현재 저장하지 않는다. R-11에는 1이 아니면 표시하고 ETF 노출에 곱한 값을 따로 보여 주자는 제안이 있다. 이를 구현하려면 보존 타입·표시 계약을 후속에서 정하고 T-09의 원본 값과 연결해야 한다. 현재 마스터가 레버리지 여부를 판정할 수 있다고 주장하지 않는다.

## 파생 사실과 생성자 폐쇄

| 공개 경로                            | 계약                                                                                                                                                                                                                                                           |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| quote_instrument                     | Result<QuoteInstrument, MasterInstrumentError>. KnownMarket·통화·심볼·내부 ID를 같은 행에서 봉인. 미지 시장 거부. 상장폐지·정지 종목도 조회 식별 자체는 만들 수 있음                                                                                           |
| order_gate_facts                     | 조회로 확정한 주문 식별과 원 행의 상장 상태·유형 알려짐·정리매매·정지·NXT 지원·시각을 함께 봉인. 입력 행과 식별을 외부에서 따로 조립할 수 없음                                                                                                                 |
| order_instrument                     | 아래 매수 방향의 보수적 판정을 적용하는 편의 경로                                                                                                                                                                                                              |
| MasterOrderGateFacts::instrument_for | 방향별 아래 판정 뒤 식별 반환. 이 식별은 승인·잔고·장 시간 검증의 증거가 아니며 O-01과 승인 경로를 면제하지 않음                                                                                                                                               |
| name_fact, price_observation         | 같은 행의 표시 이름 또는 내부 ID·주문 식별을 결합. 가격 관측 통화와 기존 현지 시각 규칙 검증. 공개 가격 관측에서 주문 식별을 다시 얻을 수 있으므로 동일 매수 식별 적격성을 요구. 거래 불가 종목의 순수 조회는 QuoteInstrument, 표시 이름은 원 행 접근자를 사용 |
| policy_security_facts                | 정상 행의 유형 사실을 유지. 미지 시장·미지 유형 행은 건너뛰고 그 원 행은 MasterInstrumentFacts에 보존. 누락 항목은 D-12의 security_type 조회에서 None이 되어 UnknownSecurityType으로 매수 차단. 전체 목록은 실패하지 않으므로 Result를 제거                    |

ResolvedOrderInstrument::from_master_lookup_result, InstrumentNameFact::from_master_lookup_result, PriceObservation::from_identified_quote_parts는 모두 pub(crate)로 닫는다. 기존 crate 내부 테스트는 그대로 둔다. 메인이 승인한 application/tests/ports.rs의 도우미와 종목별 시세 테스트만 MasterInstrument 경로로 이전하고 기존 단언은 유지한다. 기존 공개 생성자 유지라는 최초 구현의 이행 예외는 이 수정으로 폐기한다.

## 거래 가능 여부의 보수적 판정 초안

| 마스터 사실                                            | 매수 식별            | 매도 식별            | 근거와 범위                                                                                                                             |
| ------------------------------------------------------ | -------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Active, 알려진 유형, 국내 KRX 정지 아님, 정리매매 아님 | 생성                 | 생성                 | 식별 적격성일 뿐 최종 주문 허가 아님. O-01이 장·수량·유의사항 등을 검사                                                                 |
| Scheduled 또는 Delisted                                | NotListed            | NotListed            | ADR 0044는 상장폐지 뒤 매수·행사를 거부. 폐지 뒤 매도 결제 행 보존은 새 매도 주문 허용과 다름. 새 주문은 양쪽 모두 거부하는 보수적 초안 |
| 미지 유형                                              | UnknownSecurityType  | UnknownSecurityType  | ADR 0039의 미지 유형 주문 차단                                                                                                          |
| 정리매매, KRX 정지 아님                                | LiquidationBuy       | 생성                 | 매도 청산을 보존. 매수 거부는 보수적 초안. 유의사항·실제 주문 허용은 O-01                                                               |
| KRX 정지                                               | TradingSuspended     | TradingSuspended     | R-01의 거래 정지 사실. 현재 식별 편의 경로는 KRX 기준의 보수적 거부                                                                     |
| 국내 상세 미제공                                       | MissingTradingStatus | MissingTradingStatus | null을 거래 가능으로 간주하지 않음                                                                                                      |
| 미지 시장                                              | UnknownMarket        | UnknownMarket        | 닫힌 지원 시장이 아님                                                                                                                   |

NXT 지원·정지는 게이트 묶음의 원 행에서 읽을 수 있다. NXT 지원 자체를 거래 허가로 바꾸지 않는다. KRX가 정지이고 NXT만 가능한 경우를 허용하려면 O-01에서 거래소 선택·세션·경고와 함께 별도 계약을 정해야 한다. 현재 편의 경로에서 이 경우를 자동 허용하지 않는다. 정리매매·정지·미제공 정보 판정은 사용자 설정을 추가하지 않는 보수적 초안이다.

## A-20·T-09·후속 인계

- A-20: quotes(&[QuoteInstrument]), candles(&QuoteInstrument, ...)로 임시 주문 타입 입력을 교체. T-07은 이 식별의 심볼로 요청하고 내부 ID를 Candle에 넣는다.
- A-20: CalendarQuery의 시장을 KnownMarket으로 정의. J-05·A-14의 캘린더 소비자와 O-01의 장 시간 입력까지 연결한다. D-10의 MarketCalendar·SessionInformation·세션 생성 입력도 검토한다.
- KnownMarket은 Kr/Us만 있고 try_from_market이 미지 시장을 거부한다. KnownMarket → Market은 From 구현으로 제공한다.
- CommissionRateFact::from_commissions_result와 from_schedule_result의 Market 입력도 KnownMarket으로 바꿀 후속 목록에 포함한다. 이번에는 기존 시그니처를 바꾸지 않는다.
- O-01: MasterOrderGateFacts와 주문 방향·거래소를 함께 소비하고 승인 직전 다시 조회한다. 유형·상태·정지를 주문 식별만 보고 추정하지 않는다.
- T-09: 안정적 내부 ID 대응, 심볼 대문자 정규화, nullable NXT 조합 검증, 상세 행 결합과 초당 한도, 섹터 별도 조회를 연결한다.
- 메인이 고칠 tasks D-33 문장: 종목 마스터 사실 MasterInstrument(주문 식별, 시계열 ID, 이름, 종목 유형을 함께 담음), 시장의 닫힌 값 KnownMarket. 섹터 대응은 D-37·T-09의 별도 조회와 D-06이 담당.

미해결: 국내 6자리 제약은 스펙에 명시되어 유지한다. ETN·신주인수권 코드 형식은 T-09 실측 대상이다. 미국 첫 문자 영문자·코드 64바이트(시장 32바이트)·이름 256바이트 상한은 보수적 초안이다. 프로젝트 설정·훅 정의의 실제 세션 신뢰 적용은 별도 증거가 없으며 직접 실행한 검사 결과로 보고한다.

## 검증 기록

적대적 리뷰 수정 테스트를 먼저 추가해 미구현 경계에서 컴파일 실패를 확인했다. 기존 D-33 테스트는 이번 브랜치에서 추가한 것이므로 새 계약에 맞게 고쳤다. 파생 사실 왕복과 미지 값 보존 속성 테스트는 각각 Config::with_cases(4096)를 유지한다. 상장예정·상장폐지·정리매매의 방향 차이, 정지·정보 없음·미지 유형, 시세의 닫힌 시장·통화, 혼합 목록의 정상 정책 사실 보존, 소문자 거부와 NXT nullable 조합을 검사한다.

외부 crate 실험에서 허용 대조는 컴파일되고 세 legacy 생성자는 각각 E0624로 거부된다. 리뷰어의 005930 주문 식별·KODEX 200 이름·다른 내부 ID 가격 관측 혼합 재현도 세 호출 모두 E0624다. QuoteInstrument·MasterOrderGateFacts·MasterInstrument 필드 조립은 E0451, 미지 시장의 KnownMarket 입력과 Supported(None)은 E0308이다. 공개 어댑터 입력의 사실성 인증과 하네스 호출 위치 제한은 별개다.

전체 검사는 병합한 master SHA를 기준으로 실행한다. 승인된 기존 application 테스트 변경의 test-diff 항목만 예외로 보고하며 규칙·허용 목록은 바꾸지 않는다. 나머지 검사와 검사 뒤의 끄기 보고·훅 설치 확인까지 실행한다. 검사 결과·변이 결과·정확한 test-diff 항목과 최종 SHA는 구현 보고에 남긴다. 푸시·PR·실계좌 조회·새 의존성·새 검사 끄기는 없다.
