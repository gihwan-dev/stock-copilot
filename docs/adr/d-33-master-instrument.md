# D-33: 종목 마스터 사실과 알려진 시장

- 상태: 초안
- 기준: e31ab9d0ce070a9911c9e6402c9651184081e2c3

## 사실의 출처와 범위

StockInfo 상세 행을 MasterInstrumentRow로 변환한 뒤 MasterInstrument::try_new가 일관성을 검증한다. 공개 입력 행은 어댑터 경계이고, 결과 마스터와 QuoteInstrument의 필드는 비공개다. 행의 공개 필드를 바꾸어도 만들어진 마스터는 바뀌지 않는다. 외부 응답의 진위나 내부 ID의 실제 대응은 domain 생성자가 인증하지 않는다. T-09만 이 경계를 호출하도록 제한하는 것은 하네스 인계다.

| 스키마 필드                                                                    | 도메인 보존                                                                                                                      |
| ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| market                                                                         | listing_market 원문과 QuoteInstrument.market의 국가 시장. KOSPI/KOSDAQ/KR_ETC → Kr, NYSE/NASDAQ/AMEX/US_ETC → Us                 |
| symbol                                                                         | Symbol, 알려진 국내 시장은 ASCII 영숫자 6자리, 미국은 영문자로 시작하는 기존 Symbol 문법                                         |
| name                                                                           | D-32 DisplayInstrumentName. 문자 제한과 256바이트 상한을 재사용                                                                  |
| securityType                                                                   | MasterSecurityType: 아홉 ExcludedSecurityType 또는 원문 코드와 SecurityTypeKnowledge::Unknown. 모르는 값을 Stock으로 바꾸지 않음 |
| currency                                                                       | Currency, 국내 KRW·미국 USD만 허용                                                                                               |
| status                                                                         | ListingStatus: Scheduled/Active/Delisted                                                                                         |
| isCommonShare                                                                  | CommonShare: Common/Other                                                                                                        |
| koreanMarketDetail                                                             | Option<KoreanMasterDetail>. 정리매매, KRX 정지, NXT 지원과 nullable 정지 상태를 열거형으로 보존                                  |
| 내부 ID, 조회 시각                                                             | 스키마 필드 없음. T-09가 보존한 InstrumentId와 Clock의 Timestamp                                                                 |
| englishName, isinCode, listDate, delistDate, sharesOutstanding, leverageFactor | 이번 도메인 소비 계약에 필요한 사실로 확정되지 않아 보관하지 않음                                                                |
| 섹터                                                                           | 스키마에 없음. tasks D-33의 섹터 포함 문구 정정 요청. D-06 SectorAssignments가 사용자 확인·별도 섹터 조회로 채움                 |

/stocks/all의 ListedStock은 심볼·이름·유형·보통주 여부·ISIN만 준다. 조회 요청의 시장만으로 통화·상장 상태·거래 정지를 발명하지 않는다. /stocks로 상세 정보를 채운 후에만 마스터를 만든다. R-01의 STOCK_ALL 초당 1회는 T-09 책임이다.

InstrumentId는 심볼이나 ISIN에서 유도하지 않는다(ADR 0022). T-09는 저장된 안정적 ID 대응을 확보하고 코드 변경에도 유지해야 한다. 이 대응이 없으면 마스터를 만들지 않는다. 별개의 두 마스터 사이 중복 내부 ID 또는 (국가 시장, 심볼)은 MasterInstrumentFacts가 거부한다. 같은 심볼의 다른 시장은 내부 ID가 다르면 허용한다. 충돌을 마지막 행으로 덮지 않는다.

## 파생과 이행

| 파생 경로                                    | 결합                                                                               | 기존 생성자                                                                                                                 |
| -------------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| order_instrument                             | 행의 심볼·국가 시장, 미지 시장 거부                                                | ResolvedOrderInstrument::from_master_lookup_result는 기존 master 테스트 때문에 남김                                         |
| name_fact                                    | 같은 행에서 꺼낸 주문 식별과 표시 이름                                             | InstrumentNameFact::from_master_lookup_result는 기존 파일 항목 변경 금지 범위 때문에 남김. 확인된 호출은 domain 내부 테스트 |
| price_observation                            | 같은 마스터의 내부 ID와 주문 식별, 관측 가격·현지 시각. 통화와 기존 시각 규칙 검증 | 기존 from_identified_quote_parts는 소비자 이전 후 닫음                                                                      |
| quote_instrument                             | 같은 행의 시장·심볼·내부 ID. 주문 생성 없이 사용                                   | 새 QuoteInstrument는 외부 필드 조립·생성자·가변 접근자 없음                                                                 |
| MasterInstrumentFacts::policy_security_facts | 각 마스터의 식별과 유형. 미지 유형·미지 시장이면 전체 오류                         | PolicySecurityFacts의 기존 생성자는 crate 안에 계속 닫힘                                                                    |

A-20·T-09와 기존 소비자가 새 경로로 옮긴 뒤 두 공개 legacy 생성자와 PriceObservation::from_identified_quote_parts의 임의 쌍 경계를 닫아야 한다. 기존 생성자가 남아 있으므로 저장소 전체에서 유일 출처가 강제됐다고 주장하지 않는다. 새 봉인 경로에서 식별자 필드를 다른 행의 것으로 교체할 수 없다는 범위만 검증한다. 원시 입력 행을 위조하는 것은 하네스 호출 위치 제한 전까지 가능하다.

A-20은 quotes(&[QuoteInstrument]), candles(&QuoteInstrument, ...)로 임시 주문용 입력을 교체한다. QuoteInstrument는 주문 의도·계좌·승인을 받지 않아 보유 종목 조회에 쓸 수 있다. T-07은 이 타입의 심볼로 요청하고 내부 ID를 Candle에 넣는다. 가격 관측과 과제에도 동일 ID를 전달한다. application은 이번에 바꾸지 않는다.

KnownMarket은 Kr/Us만 있고 try_from_market 하나가 Market::Unknown을 거부한다. A-20의 calendar 입력, D-10 MarketCalendar·SessionInformation 생성 입력과 장 세션 조회, O-01의 시장 요구 입력을 이 타입으로 교체할 목록으로 남긴다. 기존 함수 시그니처는 이번에 바꾸지 않는다. 현재 캘린더 포트는 정의 대기이므로 새 타입을 받는 외부 실험으로 경계를 확인한다.

## 보수적 초안과 미해결

- 미국 첫 글자 영문자 제한은 보수적 정책이다. 소문자·점·하이픈·옵션 형식은 기존 Symbol 허용 범위 안에서 보존하며 주식 여부를 심볼로 추정하지 않는다. 실제 상장 여부는 T-09 응답 출처 책임이다.
- 국내 상세 정보는 스키마에서 nullable이므로 없어도 허용한다. 없음은 거래 가능 사실이 아니다. 상장 상태·정지·정리매매의 주문 허용 판정은 O-01 책임이다. 미지 시장은 원문을 보관하지만 주문·이름·정책 사실은 만들지 않는다.
- 상장 상태의 새 값과 모르는 통화는 어댑터에서 검증 실패로 격리한다. 새 값을 기존 값으로 추측하지 않는다. 종목 유형의 Unknown은 원문을 유지하며 정책 사실 변환은 실패해 D-12가 모르는 유형을 통과시키지 않는다.
- 코드 문자열의 64바이트 상한은 자원 제한 초안이다. 시장 코드는 기존 MarketCode의 32바이트 상한도 따른다.
- 프로젝트 설정의 hooks=true와 lefthook 설치는 확인했으나 세션 프로젝트·훅 신뢰의 실제 적용 증거는 확보하지 못했다. 직접 just check와 커밋 훅 결과를 증거로 삼는다.
- 새 의존성, 검사 끄기, 기존 테스트 변경은 없다. ADR 0054/H-43은 시작 시 로컬 master에 없어 새 코드의 손 변형 14개 이상을 실행한다.

## 외부 crate 실험

`cargo build --locked -p domain --message-format=json`이 낸 rlib와 해당 deps 디렉터리를 rustc의 `--extern domain=... -L dependency=... --edition=2024`에 전달했다. 계좌·키·네트워크 호출은 없다. 허용 대조 1건은 KnownMarket::try_from_market(&Market::Kr)를 KnownMarket 입력 함수에 넘겨 성공했다.

| 금지 사례                                               | 컴파일 결과 |
| ------------------------------------------------------- | ----------- |
| A의 시장·심볼과 B의 내부 ID로 QuoteInstrument 필드 조립 | E0451       |
| 만들어진 QuoteInstrument의 내부 ID 교체                 | E0616       |
| A의 이름과 B의 유형으로 MasterInstrument 필드 조립      | E0451       |
| order_instrument에 다른 내부 ID 전달                    | E0061       |
| name_fact에 다른 이름 전달                              | E0061       |
| PolicySecurityFacts의 기존 내부 생성자 호출             | E0624       |
| Market::Unknown을 KnownMarket 입력에 전달               | E0308       |
| KnownMarket::Unknown 만들기                             | E0599       |
| 표시 이름에 String 직접 대입                            | E0308       |
| QuoteInstrument::new 호출                               | E0599       |

이 실험은 봉인 타입의 외부 조립 금지를 확인한다. 공개 어댑터 입력 MasterInstrumentRow가 실제 같은 응답 행인지 인증하는 실험이 아니며, 기존 legacy 생성자 차단도 아니다. 주문 식별 생성자는 crates/application/tests/ports.rs의 기존 외부 테스트가 사용한다. 이름 사실 생성자는 외부 호출을 찾지 못했으나 기존 항목의 가시성 수정은 이번 파일 범위와 충돌해 보류한다. 메인이 후속 범위에서 이름 생성자를 pub(crate)로 닫아야 한다. 컴파일 실패 회귀를 하네스에 영속화하는 것은 별도 인계다.

## 검증 결과와 재현

- 기준 e31ab9d0ce070a9911c9e6402c9651184081e2c3에서 frozen install·lefthook 설치·전체 just check가 종료 코드 0이었다. 워크스페이스 837건, 커버리지용 553건이 통과했다.
- 테스트를 먼저 추가했을 때 MasterInstrument 등 미정의 타입으로 컴파일이 실패했다. 구현 뒤 단위·속성 테스트가 통과했다. derivations_round_trip_one_row와 unknown_codes_survive_but_never_become_known_facts는 각각 Config::with_cases(4096)이다. 전자는 서로 다른 심볼·내부 ID·이름·유형을 생성해 파생 식별과 정책 사실까지 대조한다.
- 최종 배치의 도메인 clippy, source-check, raw-types가 통과했다. coverage-check의 테스트 564건이 모두 통과했고 위반은 없다. 처음 발견한 가격 관측의 미지 시장 오류 분기는 테스트로 채웠다. 테스트 전용 파일도 측정 대상으로 요구되어 새 마스터 파일 내부 테스트 모듈로 옮겼다. 기존 테스트 본문은 수정하지 않았다.
- 손 변형 직전에도 로컬 master에는 ADR 0054가 없었다. 새 코드에 아래 16개 변형을 하나씩 넣고 `cargo nextest run --locked -p domain -E 'test(master_instrument::tests)'`로 검출한 뒤 매번 원본을 복원했다. 모두 컴파일 후 테스트 실패로 검출되었고 생존·시간 초과·컴파일 불가는 0개다. 이 결과를 자동 변이 전수 실행 결과로 확대하지 않는다.

| 손 변형                              | 결과 |
| ------------------------------------ | ---- |
| 국내 통화 거부 제거                  | 검출 |
| 미국 통화 거부 제거                  | 검출 |
| 국내 심볼 길이 거부 제거             | 검출 |
| 국내 심볼 문자 거부 제거             | 검출 |
| 미국 첫 문자 거부 제거               | 검출 |
| 미국에 국내 상세 정보 거부 제거      | 검출 |
| 미지 시장에 국내 상세 정보 거부 제거 | 검출 |
| ETF를 Stock으로 매핑                 | 검출 |
| 미지 유형을 Stock으로 매핑           | 검출 |
| 중복 내부 ID 거부 제거               | 검출 |
| 중복 시장·심볼 거부 제거             | 검출 |
| 가격 관측 통화 거부 제거             | 검출 |
| 상장 시장에 국가 코드 KR/US 허용     | 검출 |
| KnownMarket의 Kr를 Us로 매핑         | 검출 |
| KnownMarket의 Us를 Kr로 매핑         | 검출 |
| 미지 시장을 KnownMarket::Kr로 허용   | 검출 |

새 ADR 추적으로 필요한 .codex/config.toml·harness/claude-hooks/hashes.json은 지정된 쓰기 모드 생성기로 재생성했다. 직접 수정하지 않았다. 최종 전체 검사는 커밋 훅에서 실행한다. 푸시·PR·실계좌 조회는 하지 않는다.
