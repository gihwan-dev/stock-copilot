# D-21 초안: 추정 과제와 기간 제한 기준 확률

- 상태: 초안, 사용자 확인 대상
- 기준: af70b46fc6c62a0d1a7b036c9977b3d2f3d47366
- 근거: open-issues 1.1~1.3, design 6.1~6.2, DESIGN-v0.2 2절, ADR 0027~0029·0035·0037·0039

사용자 결정에 따라 세 번째 확인값 30.3%는 직접 계산 30.3258%의 소수 첫째 자리 표기이며, 소수 둘째 자리 완료 기준은 24.75%, 3.15%, 30.33%다. 비슷한 표기 차이는 문서의 식이 재현하는 값을 기준으로 한다. tasks.md 수정은 메인이 한다.

## 사건과 계산

가격이 마팅게일인 기하 브라운 운동, 로그 표류 −σ²/2, 연속 경계 감시를 가정한다. +T에 먼저 닿으면 성공, −S에 먼저 닿거나 H 만료면 실패다. DESIGN-v0.2의 식을 그대로 쓴다.

- x = −ln(1−S), L = ln((1+T)/(1−S)), wₙ = nπ/L
- λₙ = σ²wₙ²/2 + σ²/8
- p₀ = S/(T+S) + exp(x/2) Σ bₙ sin(wₙx) exp(−λₙH)
- bₙ = 4wₙ sinh(L/2)(−1)ⁿ / [L(exp(L)−1)(1/4+wₙ²)]

구현은 `sinh(L/2)/(exp(L)−1) = exp(−L/2)/2`로 정리해 같은 급수를 계산한다. 합의 공통 계수는 exp((x−L)/2), 각 계수는 2wₙ(−1)ⁿ/[L(1/4+wₙ²)]다. sinh의 뺄셈 소거와 불필요한 지수 계산을 피한다. 유한 기간을 무한 기간 값 S/(T+S)로 대체하지 않는다.

| T | S | 일간 σ | H | p₀ (%) | 소수 둘째 자리 |
|---|---|---|---|---|---|
| 10% | 5% | 2% | 20 | 24.74753464918288210760968794 | 24.75% |
| 10% | 5% | 1% | 20 | 3.15183054289253790417033055 | 3.15% |
| 20% | 10% | 2% | 120 | 30.32576591178544776222480224 | 30.33% |

## 기간 대응: 사용자 확인 대상

선택지의 넓은 한국어 이름에서 임의 날짜를 고르지 않고 아래 고정 길이 하나로 대응한다. 달력일이나 벽시계 TimeSpan을 H로 바꾸지 않는다.

| Horizon | 실제 계산 범위 초안 | H (거래일) |
|---|---|---|
| Hours | 정규장 거래 시간의 1/4. 휴장·장외 시간을 세지 않음 | 0.25 |
| FewDays | 이후 정규 거래일 5일분 | 5 |
| WeeksToMonths | 이후 정규 거래일 40일분 | 40 |
| HalfYearOrMore | 이후 정규 거래일 126일분 | 126 |

0.25·5·40은 기존 계산 예와 대조하기 쉽도록 초안으로 고른다. 126은 D-11의 연간 252거래일 가정의 절반이다. 이를 확정된 거래소 미래 일정으로 간주하지 않는다. 실제 종료 시각 계산은 거래소 캘린더와 분봉을 가진 D-27·application의 책임이다. 1/4거래일을 6시간·24시간의 1/4로 처리하지 않는다. Hours는 D-20의 봉인된 HoursPermission이 Enabled일 때만 허용한다.

## 위험 성향 계수: 사용자 확인 대상

T=aσ√H, S=bσ√H이며 계수는 코드의 닫힌 RiskAppetite 분기에서만 나온다. 보수에서 공격으로 갈수록 경계를 멀리 두되 T:S=2:1을 유지한다. 기존 예시 (1, 0.5)를 중립 초안으로 두고 양쪽에 20% 차이를 둔다. 실측 최적화 결과가 아니며 E-92에서 다시 정한다.

| 위험 성향 | a | b | H=0.25 p₀ | H=5 p₀ | H=40 p₀ | H=126 p₀ |
|---|---|---|---|---|---|---|
| 보수 | 0.8 | 0.4 | 31.569394% | 31.658840% | 31.844229% | 32.027080% |
| 중립 | 1 | 0.5 | 27.265835% | 27.525966% | 28.068636% | 28.608589% |
| 공격 | 1.2 | 0.6 | 21.508511% | 21.953260% | 22.886545% | 23.821920% |

표는 일간 σ=2%의 고정밀 계산이다. 모든 조합이 20~40%다. 다른 σ에서는 로그 경계 때문에 p₀가 완전히 일정하지 않다. 실행 시 20~40% 검사는 유지하되 범위 밖이면 CoefficientRegression으로 과제를 만들지 않는다. 이는 초안 계수의 회귀 검사이며, open-issues 1.2가 정한 실행 거부 규칙은 아니다. σ√H<10^-4, S>0.9, 산술 범위 초과, 급수 미수렴은 별도로 NumericalDomain이다.

D-11의 σ는 단순 수익률의 표본 표준편차이고 이 모형의 σ는 로그 수익률의 확산 계수다. 이번 작업은 D-11 값을 대입하는 모형 근사를 유지한다. 두 σ의 상대 차이는 작은 σ에서 약 ¾σ²이며 2%에서 약 3×10^-4다. 이 모형 차이는 수치 함수의 절대 오차 계약에 포함되지 않는다.

반년 초안 H=126은 연간 252거래일의 절반이며 open-issues의 예시 및 세 번째 확인값 H=120과 다르다. 둘을 같은 값으로 취급하지 않으며 H=126은 사용자 확인 대상이다.

## 사실과 소비 경계

EstimateContext는 PriceObservation과 생성 시각만 받는다. EstimateInstrument와 호출자가 넣던 latest_closed_day는 제거한다. 과제 생성은 별도의 Volatility와 이 두 사실만 받으며 AI 확률·설명·임계값·가격·계수·기간 수치를 입력하는 필드는 없다.

PriceObservation의 새 from_identified_quote_parts 생성 경로는 실제 가격 관측의 InstrumentId와 ResolvedOrderInstrument, 가격, 시장 현지 오프셋으로 정규화된 관측 시각을 함께 보존한다. 기존 D-14 생성 경로는 내부 ID가 없으며 D-21은 이를 InstrumentMismatch로 거부한다. D-11의 opaque InstrumentId를 Symbol 문자열로 추정하지 않는다. T-09 마스터 조회와 가격 어댑터가 실제 관측의 내부 ID를 넣고, domain은 두 봉인된 사실의 ID를 직접 비교한다. 과제 생성 시 기대 ID나 임의의 매핑을 따로 넘기지 않는다. 모든 사실 타입과 마찬가지로 생성자는 외부 응답의 진위를 인증하지 않는다.

관측 시각은 국내 +09:00, 미국 -05:00 또는 -04:00 현지 표현만 허용한다. 미국 원시 API 시각이 KST이면 어댑터가 동부 현지 오프셋으로 정규화해야 한다. 계절에 맞는 오프셋 선택은 어댑터의 사실 출처 책임이며 D-21은 DST를 자체 계산하지 않는다(ADR 0028). 새 생성자가 허용 범위를 검증하고 보존한 현지 오프셋에서 관측일을 유도하며 생성 시각도 같은 오프셋에 투영한다. 관측 이후 5초 미만만 생성할 수 있으므로 DST 전환을 가로질러도 달력 날짜의 투영이 달라지지 않는다. 임의의 UTC/KST 표현으로 미국 관측일을 바꾸는 경로는 없다. 이 어댑터 정규화의 실제 API 연결 검증은 후속 범위다.

Volatility는 검증한 수정주가 확정 일봉에서만 생긴다. 일수익률 표본 60개 미만(종가 61개 미만)은 InsufficientSamples, 두 사실의 내부 ID 불일치는 InstrumentMismatch다. 일간 σ만 소비하며 AnnualVolatility 경로가 없다. 표본 수·as_of·수정 기준은 원 Volatility를 그대로 보존한다.

시각 대조는 `Volatility.as_of ≤ 관측의 시장 현지일 ≤ 생성의 시장 현지일`이다. 미래 변동성은 FutureVolatility, 기준일에서 생성일까지 7달력일을 넘으면 StaleVolatility다. 7일은 긴 연휴를 덮기 위한 초안이며 사용자 확인 대상이다. 3,700일 경과·미래 정보·7일 허용·8일 거부·국내와 미국 두 오프셋의 현지 자정 경계·다른 내부 ID를 회귀로 고정한다.

D-10 MarketCalendar에는 previous_business_day·today·next_business_day 세 행과 장 구간이 있다. 그러나 변동성·가격 관측·생성 시각에는 이 캘린더가 포함되지 않고 domain은 캘린더를 조회하거나 휴장일을 외삽하지 않는다. 세 행의 라벨만으로 임의 생성 시각 직전의 확정 거래일을 증명할 수도 없다. 따라서 이번 세 사실 입력 계약에서는 직전 거래일 강제가 불가능해 7달력일 상한을 쓴다. 캘린더 사실을 명시적으로 추가하는 후속 작업에서 더 엄한 직전 확정 거래일 판정으로 바꿀 수 있다.

기준 가격과 관측 시각은 PriceObservation에서, 과제 생성 시각은 인자에서 고정한다. 기준 관측은 시장의 통화와 일치해야 한다. D-14의 5초 미만 신선도 및 미래 관측 거부를 재사용한다. 5초는 ADR 0037의 보수적 초안이며 D-21에서 새 시간 설정을 추가하지 않는다.

EstimateTask가 D-14 EstimateRangeFacts의 유일한 새 공개 생성 경로다. ProposalPriceRange와 TradingDayHorizon을 재사용한다. ProposalVolatility에는 DailyVolatility를 받는 crate 내부 생성만 추가한다. 기존 Ratio 생성자와 master 테스트는 보존한다. 기존 D-14 API 전체를 바꾸는 일은 별도 범위다.

T·S·p₀는 각각 봉인된 EstimateDistance·BaseProbability다. 공개 생성자·Decimal 접근·Ratio 변환·Money 변환·역직렬화·Default는 없다. Display와 Debug는 비율 소수 4자리로 중간값을 0에서 멀어지는 방향으로 반올림하고 ≈ 표지를 붙인다(사용자 확인 대상 표시 초안, 0.01%p 해상도). 호출자가 포맷 정밀도를 늘려도 전체 자릿수를 내지 않는다. 이 문자열은 정확 Ratio의 파서가 거부하며 내부 계산값은 바꾸지 않는다. D-22·D-23은 domain 내부에서 이를 소비할 수 있다. 정확한 H만 TradingDayHorizon의 Ratio 접근자로 읽는다.

목표·손절을 금액 가격으로 만드는 API는 이번 범위에 넣지 않는다. D-27의 판정 비교는 기준 가격과 근사 경계를 소비해야 하며, 주문 가격·호가 단위는 D-26의 별도 정책이다. 근사 값을 정확 Ratio로 포장해 Money 곱셈에 넣지 않는다(ADR 0029). D-22의 비용 비율 5%p 초과 거부가 추가되기 전에는 이 과제만으로 전체 거래 적격성을 뜻하지 않는다.

## 십진 수치 계약과 검증 범위

새 의존성이나 maths 기능은 없다. base_probability.rs만 문서 근거를 붙여 산술 허용 목록에 등록하며 표시 전용 반올림 허용 목록에도 추가한다. 모든 십진 덧셈·뺄셈·곱셈·나눗셈·나머지는 checked다. 부호 변경은 부호 비트 연산이다.

- ln: z=(v−1)/(v+1)의 atanh 급수. checked 항이 표현 정밀도에서 0이 되면 종료한다.
- exp: 입력을 1024로 나눈 Taylor 급수, checked 항이 0이 되면 종료 후 제곱 10회. 입력 <−64이면 0(생략 절대 오차 <1.61×10^-28).
- sin: 2π로 나머지를 구한 Taylor 급수, checked 항이 0이 되면 종료한다. π는 소수 28자리 상수다.
- 각 초월 함수와 확률 급수에는 2048회 상한이 있다. 표현 범위 초과·나눗셈 실패는 Computation의 실패 상태로 전파하고, 상한까지 미수렴은 None이며 과제 생성의 NumericalDomain 오류로 전파한다. 실패를 0 확률 성공으로 바꾸지 않는다.
- 확률 급수는 λₙH≥64에서 종료한다. 계수 절댓값은 2/(nπ) 이하이고 exp((x−L)/2)≤1이다. 감소하는 Gaussian 꼬리를 적분으로 감싸면 생략 꼬리는 exp(−qN²)/(πqN²) 이하(q=σ²Hπ²/(2L²)); λ의 상수 부분은 별도 감소 인자다. 이 정책의 꼬리 크기는 10^-27보다 작다.
- sqrt(H)는 D-11의 기존 checked 제곱근과 2 ulp 계약을 재사용한다.

수치 대조 계약은 확인값 세 개 및 독립 격자에서 p₀ 절대 오차 <10^-18이다(퍼센트 값으로 <10^-16%p). 독립 기준은 Python 10^100 고정소수점 정수, 원래 sinh 포함 식을 계산해 소수 28자리로 저장한 81점이다. T∈{0.05,0.1,0.2}, S/T∈{0.25,0.5,0.75}, σ∈{0.01,0.02,0.05}, H∈{5,20,120}의 전체 곱이며 Rust 구현 출력을 복사하지 않았다. 테스트는 전체 81점과 아래 영상 급수 69점, 두 기준 격자를 함께 고르는 4096회 재매개변수화 속성 표본을 대조한다. 원래 식과 정리한 식, 100자리 정수와 28자리 십진 연산을 대조한다. 기준값의 저장 절삭 오차는 10^-28 미만로 계약보다 작다.

이 격자 검증을 Decimal 전체 입력 공간의 해석적 절대 오차 증명으로 확대하지 않는다. 특히 S가 1에 아주 가깝거나 극히 작은 σ에서는 반복 상한·표현 정밀도의 제약이 있다. 미수렴을 거부하는 정책을 유지하며, 현실 σ 전체 구간의 연속적 오차 증명은 후속 검증 범위다. 근사 타입 봉인이 이 제한을 금액 산술로 옮기지 않게 한다.

같은 사실 입력에 같은 과제·이유가 나오는 결정성은 4×3 선택 조합을 4096회 생성해 검사한다. 오류에는 입력값·계좌·토스 키를 담지 않는다. 실제 키·실계좌·네트워크 호출은 없다.

기존 81점은 같은 푸리에 급수의 정밀도 대조이므로 모형 식 자체의 독립성 증거로 확대하지 않는다. 원래 생성 절차와 새 영상 급수 코드는 아래 Markdown 부록에 보존한다. d-21-reference.py 삭제는 기존 훅의 git 추적 ADR 경로 대조 테스트가 거부했다. 사용자 지시에 따른 예외로 이 경로에는 한 줄 안내만 남기며 계산 코드는 두지 않는다. 보호된 테스트나 경로 규칙은 바꾸지 않는다.

별도의 영상 급수 기준은 세 성향 × H={0.25,5,40,126} × 일간 σ={0.6,1,2,4,8}%의 60점과, 세 성향의 Hours에서 σ={0.02,0.04,0.1}%의 9점이다. 가장 작은 σ√H는 10^-4다. 모두 성공 확률이 유의하게 0보다 커 푸리에 급수의 무한 기간 항만 검증하는 격자가 아니다. Python Decimal 240자리 연산으로 영상 급수의 정규 CDF 적분을 계산한다. 제품의 푸리에 계수·고유값·절단 기준은 재사용하지 않는다. 69점에서 측정한 최대 절대 오차는 4.820×10^-25이며, 격자 회귀의 더 엄한 오차 예산은 10^-24다. 일반 수치 계약은 10^-18로 유지한다.

D-11 DailyVolatility와 AnnualVolatility의 Display도 전체 자릿수를 낸다. master의 volatility_display.rs 테스트가 원문 전체 문자열을 고정하므로 이번 회차에서는 해당 구현과 기존 테스트를 바꾸지 않는다. D-11 Display 및 파생 Debug의 표시 봉인 수정과 테스트 계약 변경을 메인에 인계한다. D-21 T·S·p₀의 표시 경로는 이번에 봉인했다.

## 과제의 지원 수치 범위

정확도 계약을 위한 내부 수치 범위는 `u=σ√H≥10^-4`, `S≤0.9`다. 범위 밖은 NumericalDomain이며 값을 바닥값으로 올리거나 손절 거리를 잘라 성공시키지 않는다. u의 하한은 로그 경계를 만들 때 1 부근 십진 상쇄의 상대 오차를 제한하기 위한 기술적 경계다. 설정·화면 항목을 추가하지 않는다. 일간 σ 0.5~5%와 표의 H는 이 범위 안이다.

이 범위의 T:S=2:1, b≤0.6에서 `L/u≤0.6 ln(28)/0.9<2.222`이므로 Gaussian 계수 q>0.99다. 따라서 열 개 미만의 확률 급수 항으로 종료 기준에 닿는다. ln의 |z|≤27/29에서 종료 항 뒤 나머지는 1.6×10^-25 미만이고, L≥10^-4이므로 이 항의 상대 영향은 1.6×10^-21 미만이다. sin 입력은 2π 미만이며, exp는 비양수 입력을 축소 후 제곱한다. 각 연산의 십진 표현 오차와 2048회 반복 상한을 보수적으로 합쳐 과제 p₀의 절대 오차 계약을 10^-18로 둔다. 이 계약은 전달된 D-11 일간 σ를 수치 입력으로 취급한다. 시장 모형의 정확도나 σ 추정의 통계 오차를 포함하지 않는다. 앞 절의 전 Decimal 범위 제한은 그대로다.

## 실행 경계

프로젝트 .codex 설정의 hooks=true는 확인했다. T3 세션에서 프로젝트 신뢰와 훅 정의 신뢰가 실제로 적용되었다는 별도 증거는 확보하지 못했으므로 세션 훅 강제를 주장하지 않는다. frozen install과 lefthook install을 실행하고 실제 just check 및 Git 커밋 훅의 결과로 검증한다. 기본 홈의 인증·키를 읽거나 변경하지 않았다. 외부 rustc 양성 대조는 성공했고, AnnualVolatility 과제 입력은 E0308, T·S·p₀의 외부 Decimal 접근은 E0624로 거부되었다.

검증 명령은 `cargo nextest run --locked -p domain -E 'test(estimate_task) | test(base_probability)'`, `just base=af70b46fc6c62a0d1a7b036c9977b3d2f3d47366 check`다. 확인값 회귀는 구현 전 미정의 타입·함수로 컴파일 실패했고 구현 뒤 통과했다. 독립 격자 대조는 각도 축약의 중복 상수 전사 오류를 검출했고 π에서 주기를 계산하도록 수정했다. 2π를 넘는 각도 7·10의 고정 회귀를 함께 둔다. 기존 master 테스트 본문·새 의존성·검사 끄기·커버리지 예외는 변경하지 않았다.

리뷰 수정 회차는 전체 crate 변이 검사 대신 estimate_task.rs·base_probability.rs의 수용·거부·수렴·산술 실패 분기를 지우는 손 변형 18개를 실행했다. 모두 컴파일된 뒤 해당 경계 테스트가 실패해 검출됐으며 생존·시간 초과·컴파일 불가는 0개다. 원본을 복원하고 정상 코드 전체 검사와 커밋 훅을 실행한다.

## 부록 A: 영상 급수 기준값의 계산 절차

흡수 경계 열핵을 영상법으로 펼친 뒤 상단 경계 유출량을 0부터 H까지 적분한다. v=σ²H, dₖ=L−x+2kL, Φ는 표준 정규 CDF다.

`p₀ = exp(−(L−x)/2) Σ sign(dₖ) [exp(|dₖ|/2) Φ((−v/2−|dₖ|)/√v) + exp(−|dₖ|/2) Φ((v/2−|dₖ|)/√v)]`

정규 CDF는 밀도 exp(−z²/2)의 적분 급수로 구하며 푸리에 사인 급수를 쓰지 않는다. 아래 절차는 테스트 전용이다. Decimal은 Python 표준 라이브러리의 고정 정밀도 십진 연산이며 float 리터럴·변환·외부 의존성이 없다. 격자의 T·S를 먼저 소수 28자리로 절삭해 Rust 입력과 정확히 맞춘 뒤 기준 p₀를 계산한다. CDF |z|>20 생략의 오차는 3×10^-89 미만이다. 이 격자의 영상 가중치와 65항을 보수적으로 합쳐도 기준 오차는 10^-60 미만이며, |k|>32 꼬리는 더 작다. 저장 절삭 오차는 10^-28 미만이다.

다음 코드 블록을 임시 파일로 추출해 python3로 실행하면 Rust의 69점 표를 재생성할 수 있다.

```python
from decimal import Decimal as D, localcontext, ROUND_DOWN
from itertools import product
import json

PI = D("3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679")

def normal_cdf(value):
    if value < 0:
        return normal_tail(-value)
    return 1 - normal_tail(value)

def normal_tail(value):
    if value > 20:
        return D(0)
    term = total = value
    for index in range(1, 3001):
        term *= -value * value / (2 * index)
        addition = term / (2 * index + 1)
        total += addition
        if abs(addition) < D("1e-210"):
            return D("0.5") - total / (2 * PI).sqrt()
    raise ArithmeticError("normal cdf did not converge")

def image_probability(target, stop, sigma, days):
    start = -(1 - stop).ln()
    length = ((1 + target) / (1 - stop)).ln()
    variance = sigma * sigma * days
    scale = variance.sqrt()
    total = D(0)
    for index in range(-32, 33):
        distance = length - start + 2 * index * length
        absolute = abs(distance)
        contribution = (absolute / 2).exp() * normal_cdf((-variance / 2 - absolute) / scale)
        contribution += (-absolute / 2).exp() * normal_cdf((variance / 2 - absolute) / scale)
        total += contribution if distance > 0 else -contribution
    return (-(length - start) / 2).exp() * total

def cases():
    profiles = [("0.8", "0.4"), ("1", "0.5"), ("1.2", "0.6")]
    grid = [(profile, days, sigma) for profile, days, sigma in product(profiles, ["0.25", "5", "40", "126"], ["0.006", "0.01", "0.02", "0.04", "0.08"])]
    grid += [(profile, "0.25", sigma) for profile, sigma in product(profiles, ["0.0002", "0.0004", "0.001"])]
    for (a, b), days, sigma in grid:
        scale = D(sigma) * D(days).sqrt()
        target = (D(a) * scale).quantize(D("1e-28"), rounding=ROUND_DOWN)
        stop = (D(b) * scale).quantize(D("1e-28"), rounding=ROUND_DOWN)
        result = image_probability(target, stop, D(sigma), D(days))
        yield [str(target), str(stop), sigma, days, format(result.quantize(D("1e-28"), rounding=ROUND_DOWN), "f")]

if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 240
        print(json.dumps(list(cases()), indent=2))
```

## 부록 B: 기존 푸리에 정밀도 기준의 재현

이 절차는 기존 81점의 출처 보존용이며 식 표현의 독립성을 주장하지 않는다.

```python
from fractions import Fraction
from itertools import product

SCALE = 10**100
PI = 31415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679


def multiply(left, right):
    return left * right // SCALE


def divide(left, right):
    return left * SCALE // right


def logarithm(value):
    ratio = divide(value - SCALE, value + SCALE)
    squared = multiply(ratio, ratio)
    power = ratio
    total = 0
    for index in range(3000):
        total += power // (2 * index + 1)
        power = multiply(power, squared)
    return 2 * total


def exponential(value):
    if value < -250 * SCALE:
        return 0
    reduced = value // 1024
    term = total = SCALE
    for index in range(1, 301):
        term = multiply(term, reduced) // index
        total += term
    for index in range(10):
        total = multiply(total, total)
    return total


def sine(value):
    reduced = (value + PI) % (2 * PI) - PI
    term = total = reduced
    for index in range(1, 301):
        term = -multiply(multiply(term, reduced), reduced) // (2 * index * (2 * index + 1))
        total += term
    return total


def number(text):
    return int(Fraction(text) * SCALE)


def probability(target, stop, sigma, days):
    start = -logarithm(SCALE - stop)
    length = logarithm(divide(SCALE + target, SCALE - stop))
    total = divide(stop, target + stop)
    for index in range(1, 4001):
        frequency = divide(index * PI, length)
        square = multiply(frequency, frequency)
        decay = multiply(multiply(sigma, sigma), square) // 2 + multiply(sigma, sigma) // 8
        decay = multiply(decay, days)
        if decay > 250 * SCALE:
            break
        coefficient = divide(
            2 * multiply(frequency, exponential(length // 2) - exponential(-length // 2)) * (-1)**index,
            multiply(multiply(length, exponential(length) - SCALE), SCALE // 4 + square),
        )
        total += multiply(multiply(multiply(exponential(start // 2), coefficient), sine(multiply(frequency, start))), exponential(-decay))
    return total


def decimal_text(value):
    sign = "-" if value < 0 else ""
    absolute = abs(value)
    return sign + str(absolute // SCALE) + "." + str(absolute % SCALE).zfill(100)[:28]


if __name__ == "__main__":
    for target, stop_fraction, sigma, days in product(["0.05", "0.1", "0.2"], ["0.25", "0.5", "0.75"], ["0.01", "0.02", "0.05"], ["5", "20", "120"]):
        stop = multiply(number(target), number(stop_fraction))
        result = probability(number(target), stop, number(sigma), number(days))
        print('("' + '","'.join([target, decimal_text(stop), sigma, days, decimal_text(result)]) + '"),')
```
