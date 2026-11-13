# D-14 초안: 봉인된 실행 토큰과 가격 사실

상태: 초안. 적대적 리뷰 수정 기준 `a83ea97c701ce7b84152c0df056e781bb270a44a`. 수정 중 로컬 master `a291516a7808c7fe2468cf0e0014c7399ce44583`를 병합했다. D-11·D-21·O-01은 아직 이 기준에 없다.

## 상태와 실행 경계

상태는 architecture 4.3의 Draft와 Validated 두 개다. ExecutionToken은 검증된 제안을 소유한 소비 결과물이며 승인 상태를 추가하는 것이 아니다.

| 전이 | 조건·결과 | 근거 |
|---|---|---|
| 없음 → Draft | 고정 ID·OrderIntent·추정 범위 사실·만료 설정. 키·종목·통화가 일치해야 한다 | 4.3 “Proposal<Draft>는 코드(DraftProposal)만 만든다” |
| Draft → Validated | validate가 현재 관측 사실과 시각으로 계산한 보고서 전 항목이 Passed. Failed·Unknown 하나라도 있으면 거부 | 4.3 domain::validate, 4.4 순서 4 “최신 시세와 잔고로 validate를 다시 돌려” 만든다 |
| Draft → 실행 | 메서드 없음, E0599 | 4.3의 Validated 다음 단계만 실행 경로에 들어간다 |
| Validated → ExecutionToken | self 소비. 시각 역행·만료·관측 종목·신선도·가격 범위를 재확인 | 4.4 순서 7 “다시 계산해서 보내지 않는다. 사용자가 본 요약과 보내는 주문이 같아야” 한다 |
| 소비된 Validated 재사용 | E0382 | 4.3 “토큰은 self를 소비하는 메서드로만 넘어간다” |

**OrderIntent 값은 실행 경로의 입력이 될 수 없다.** O-03의 승인 타입과 실행 crate는 ExecutionToken만 받아야 한다. O-17/O-18은 본인 확인(순서 6), 잔고·시장 등 전체 재확인(7), 소비·전송 전 기록(8)을 거친 뒤 ApprovedOrder를 만들고 전송(9)한다. domain의 into_execution은 순서 7의 시간·가격 부분만 담당하며 승인 권한 자체가 아니다. 전송까지 지연되면 보관된 관측과 시각으로 다시 확인해야 한다.

ExecutionToken의 공개 API는 `intent() -> &OrderIntent`, `id() -> &ProposalId`, `validated_at() -> Timestamp`, `checked_at() -> Timestamp`, `validation_observation() -> &PriceObservation`, `execution_observation() -> &PriceObservation`뿐이다. 생성자·값 추출·가변 접근자가 없고 Clone·Copy·Default·직렬화·역직렬화도 없다. 토큰은 Proposal<Validated> 전체를 비공개로 소유한다. 읽은 OrderIntent를 clone해도 ExecutionToken이 아니므로 실행 입력으로 넘길 수 없다. 검증 시점과 소비 시점의 관측을 함께 보존한다.

Validated도 비공개 필드이며 복제·Default·직렬화·복원 경로가 없다. 공개 validate는 외부 보고서를 받지 않는다. 비공개 finish_validation만 타입을 만들고, 단위 테스트는 동일 모듈에서 결과를 구성해 허용 전이를 검증한다. 테스트용 공개 생성자는 없다.

## 사실 생성 책임

PriceObservation은 조회로 확정한 종목(시장 포함), 가격, 관측 시각과 도출한 신선도 끝 시각을 비공개로 보유한다. 최소 경계 생성자는 `from_quote_parts`다. T-07의 검증된 시세 어댑터가 채울 책임이며, D-11 병합 뒤 Quote에서 만드는 생성자로 연결한다. 이 타입은 응답의 진위를 인증하는 서명이 아니다. 값의 출처는 코어·어댑터 신뢰 경계에 속하며, domain은 종목·시장 일치, 미래 관측 거부, 신선도를 직접 검사한다.

주문용 시세 지연 한도를 design·architecture·open-issues에서 찾았다. design 4.3 표의 보유 종목 시세 1분 갱신·캐시와 ACC-4의 5분 경고는 화면 요구이며 주문 허용 한도가 아니다. **주문 신선도 5초 미만은 문서 근거 없음, 보수적 초안**이다. 관측 시각 이상부터 정확히 5초 전까지만 허용한다. 미래 관측, 다른 종목·시장, 시한 계산 범위 초과는 통과시키지 않는다.

EstimateRangeFacts는 종목·추정 기준가·일간 σ·거래일 H를 봉인한다. 외부 crate에는 생성자·필드 쓰기가 없다. 현재 공개 API로 임의 범위 사실을 만들 수 없고, 같은 domain crate의 D-21이 EstimateTask에서 생성해야 한다. 지금은 crate 내부 최소 필드만 두었으며 D-21 병합 때 과제에서 만드는 생성자로 연결한다. ProposalPriceRange::from_estimate_task는 이 사실과 카드 기준 관측을 받아 종목·통화를 대조하고, Proposal 생성자가 주문 종목과 다시 대조한다. 카드를 재생성해도 추정 기준가는 바꾸지 않는다.

σ의 단위는 일간이다. 현재 ProposalVolatility는 양수 Ratio의 최소 타입이며 **D-11 병합 뒤 일간 변동성 타입만 받는다. 연율 타입은 받지 않는다.** H는 거래일 수이고 양수 소수 거래일도 표현한다. 몇 시간 기간의 설정 허용 여부는 O-01 정책 판정이다.

## 만료 설정과 시계

ProposalLifetime은 1~600초의 검증된 설정 타입이고 기본은 600초다. design 6.5의 10분과 고급 설정 표·O-14의 “카드 만료 시간”을 함께 반영한다. 설정 허용 범위는 문서에 없어 **최소 1초·상한 10분은 초안**으로 정했다. 기본보다 오래 유효하게 하지 않는 보수적 상한이다. O-14는 이 타입을 통해 설정해야 한다.

시각은 인자로만 받는다. Timestamp·TimeSpan checked 산술로 시한을 계산하며 `created_at <= now < expires_at`만 유효하다. 정확히 10분은 만료, 1ns 전은 유효다. 검증 5초 후 실행 3초의 역행도 ClockRollback이다. 고정된 정상 TimeSpan의 불가능한 오류 변환 분기는 따로 만들지 않는다.

## 가격 비교

open-issues 1.1의 a·b는 목표·손절 계수다. 제안 만료 계수 후보는 없고 design 6.5가 D-14에 맡긴다. **k=0.5는 초안**이며 H 기간의 1σ보다 좁은 반 σ를 초기 허용 폭으로 쓰는 보수적 출발점이다. 실측·최적화한 금융 기준이 아니다.

`|P−R| <= 0.5 σ √H R`를 `4(P−R)² <= σ² H R²`로 비교한다. 카드·추정 기준가 양쪽을 만족해야 한다. 경계는 포함한다. Decimal 중간 곱이나 제곱근을 계산하지 않는다. 이전의 “정밀도 손실이면 거부” 방식은 현실적인 긴 소수 σ를 거부하므로 폐기했다.

R·P의 정수 계수를 Cr·Cp, 스케일을 sr·sp, σ·H의 계수를 Cs·Ch, 스케일을 ss·sh라 하자. S=max(sr,sp), D=|Cp×10^(S−sp)−Cr×10^(S−sr)|이면 다음 정수식을 비교한다.

`4 D² × 10^(2ss+sh) <= Cs² Ch Cr² × 10^(2S−2sr)`

ProposalMagnitude는 기존 Magnitude의 96비트 계수 추출과 Limb·Word·split_word를 재사용하는 32 limb, 1,024비트 정수다. 가격 정렬 계수는 190비트 미만이고 차이 제곱·4·최대 84자리 스케일을 포함한 좌변은 662비트 미만이다. 우변은 5개의 96비트 계수와 최대 56자리 스케일이므로 667비트 미만이다. 모든 정상 Decimal 조합과 모든 중간 곱이 1,024비트 안에 들어간다. 따라서 용량 초과·정밀도 손실이라는 판정 불가 상태를 만들지 않는다. 통화 불일치는 Failed다.

자리 곱의 합은 기존 ADR 0029와 같이 최대 u64::MAX이며 wrapping word 연산의 실제 넘침은 없다. 차이의 overflowing_sub는 자리 빌림을 전달하는 정수 알고리즘이다. Decimal 금액 산술에는 wrapping·saturating을 사용하지 않는다. 스케일의 saturating 정수 연산은 입력 스케일 0~28과 max 관계 때문에 포화하지 않는다. 기존 산술 함수·master 테스트 본문은 바꾸지 않았다.

현실 표는 KRW 71234.5·1000000, USD 187.23·0.01, σ 0.5%~10% 및 28자리·1/3 표현값, H 1·250·0.125·20.5를 포함한다. 이전에 거부하던 σ=1e-14와 현재가=기준가도 Passed다. 독립 기준은 십진 숫자 벡터의 유리수 교차곱이며 구현의 32비트 limb와 다르다. Config::with_cases(4096) 생성기는 긴 28자리 σ·소수 H·소수 가격을 포함한다.

## 검사 항목과 다른 단계의 책임

ValidationReport는 모든 필드를 패턴으로 구조 분해한다. `..`나 ALL 병행 목록은 없다. 새 필드를 추가하고 판정에서 빠뜨리면 E0027 또는 사용하지 않은 바인딩 오류가 난다. 항목 결과 접근자는 닫힌 ValidationCheck를 와일드카드 없이 match한다. O-01은 봉인된 사실 값으로 항목별 domain 순수 함수를 호출해야 하며 호출자가 Passed를 넣는 setter·builder는 만들지 않는다.

| 항목·요구 | 현재 | 담당·근거 |
|---|---|---|
| 종목 존재·시장 | Unknown | O-01, 종목 마스터. architecture 4.4 순서 4, design 6.4 |
| 장 시간·금액/소수 주문 시간 제한 | Unknown | O-01, design 6.3·O-01 행 |
| 통화별 매수가능금액·매도가능수량 | Unknown | O-01, design 6.4·6.5 |
| 국내 반대 방향 미체결 | Unknown | O-01, design 6.4 |
| 매수 유의사항 | Unknown | O-01, design 6.4 |
| 원칙·성향·비중·기간 허용 | Unknown | D-12·O-01, design 6.3·6.4 |
| 주문 가격: 지정가·최신 호가 정합·호가 단위 | Unknown | D-26·O-01, design 6.3·6.5 |
| 시장가 허용 설정 | Unknown | D-26·O-01, design 6.3·6.5 |
| 계좌 일치 | Unknown | O-01, 계좌·잔고·주문 결합, architecture 4.4 순서 4~5 |
| 만료 | domain 계산 | design 6.5, 순서 3·7 |
| 카드·추정 기준가 가격 범위 | domain 계산 | design 6.5, 순서 3·7 |
| 관측 종목·미래/신선도 | domain 계산 | 최신 시세 요구, 순서 4·7 |
| 스위치·시간당 횟수 상한 | 별도 단계 | O-06·O-22, 순서 2 |
| 요청 빈도·동시 창·거절 후 제한 | 별도 단계 | O-04, 순서 1 |
| 요약·고액 확인·본인 확인 | 별도 단계 | D-32·O-07·O-09·O-17, 순서 5~6 |
| 소비·전송 전 감사 기록·중복·재시도 | 별도 단계 | O-18·O-19, 순서 8~10 |

O-01의 아홉 항목은 현재 Unknown이므로 공개 validate로 Validated를 만들 수 없다. 보고서는 감사·화면 힌트이며 실행 권한이 아니다. D-32는 읽기 접근자로 고정 주문과 ID를 받아 요약·비용·계좌 뒤 네 자리·키 앞 여덟 자리를 계산한다.

## ID와 Debug

ProposalId에서 같은 값의 ClientOrderId를 결정적으로 얻고, 제안 생성자가 OrderIntent의 키와 일치하는지 검사한다. 검증·소비 후에도 바뀌지 않는다. O-18의 제안 ID 고유 제약은 같은 ID의 중복만 막는다. ADR 0032가 지적한 “같은 의도에 다른 키”의 의미 중복 방지는 tasks의 O-18/O-19 행에 명시된 별도 기준이 없다. **담당 없음, 메인이 정할 것.** D-25의 같은 종목·기간 내 두 번째 매수 신호 제한은 일부만 막으며 일반적인 주문 의도 중복 방지로 대체하지 않는다.

Proposal과 ExecutionToken의 Debug에는 계좌 키가 없다. OrderIntent 자체의 Debug는 계좌 식별자 전체를 출력하는 기존 동작이다. D-13 파일은 이번에 수정하지 않으며 메인이 별도로 다룬다. 오류에는 입력값이 없다.

## 외부 컴파일·손 변형 증거

rustc 외부 crate 실험은 허용 호출 3종과 금지 호출 16종이다. validate·토큰 소비·읽기는 컴파일된다. 토큰/Validated/사실 리터럴 위조 E0451, 토큰 Clone·Copy·Default·Serialize·Deserialize와 Validated 복제·직렬화 E0277, 값 추출 메서드 E0599, 맨 OrderIntent·읽어 복제한 주문을 토큰 소비자에 전달 E0308, 소비 후 재사용 E0382를 확인했다.

토큰에 into_intent를 추가하는 손 변형에서는 원래 E0599인 금지 사용처가 컴파일되어 외부 음성 대조군이 API 후퇴를 검출함을 확인하고 원본을 복구했다. 종목 대조 제거·신선도 제거·Unknown 허용·M14 역행 검사 약화는 각각 테스트가 실패했다. 새 보고서 필드와 Unknown 초기화를 추가하고 판정에서 누락시키는 M21 변형은 E0027로 컴파일이 거부됐다. 자료는 target/d14-review에 있다.

봉인은 지금 외부 컴파일 실험으로만 확인됐고 영속 테스트는 하네스 작업 대기다. domain에 trybuild·serde·syn 개발 의존성을 추가하지 않았다. 새 의존성·검사 끄기·실제 계좌·주문 API 호출은 없다. 프로젝트·훅 신뢰 적용은 확정하지 못하므로 직접 검사·Git 훅 결과만 증거로 사용한다.
