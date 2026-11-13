# D-14 초안: 제안 검증과 소비 경계

상태: 초안. 최초 기준 `6a6acdc12a5ed53ee522fffab2a3833cf2a9a461`. DS-02가 반영된 로컬 master `88566415e2503aee40d19e74b5917c02f2b8e5e4`를 fast-forward로 받았다.

## 상태와 전이

상태는 architecture 4.3의 두 개뿐이다. 승인·소비됨·전송 중을 domain 상태로 추가하지 않는다.

| 입력 | 작업 | 결과 | 근거 문장과 책임 |
|---|---|---|---|
| 없음 | `Proposal<Draft>::new(id, intent, created_at, range)` | Draft 또는 값 없는 오류 | architecture 4.3: “Proposal<Draft>는 코드(DraftProposal)만 만든다.” application이 저장·복원한다 |
| Draft | `validate(self, now, current)` | 모든 항목 통과 때만 Validated, 아니면 ValidationReport | 4.3의 `domain::validate(facts, policy)`, 4.4 순서 4: “최신 시세와 잔고로 validate를 다시 돌려” 만든다 |
| Draft | 실행 소비 | 컴파일 불가 E0599 | 4.3의 Validated 다음에만 approve_and_submit이 온다 |
| Validated | validate / report / 생성자 | 컴파일 불가 E0599 | 4.3: “메모리에만. Clone 없음. 저장·복원하지 않음” |
| Validated | `into_execution(self, now, current)` | 동일한 OrderIntent 또는 오류. 성공·실패 모두 self 소비 | 4.4 순서 7: “확인 창이 떠 있는 동안 가격이나 잔고가 허용 범위를 넘게 바뀌었으면 보내지 않는다. 다시 계산해서 보내지 않는다” |
| 소비된 값 | 다시 실행 | 컴파일 불가 E0382 | 4.3: “토큰은 self를 소비하는 메서드로만 넘어간다” |

`into_execution`은 승인이나 전송이 아니다. O-03의 UserApproval이 Validated를 소유하고 O-17/O-18이 순서 6의 본인 확인, 7의 최신 잔고·시장 등 전체 재확인, 8의 소비·전송 전 기록을 수행한 다음 이 순수 소비 메서드를 호출한다. 이 메서드는 7의 시간·가격 부분만 담당한다. 8 실패 시 호출·전송하지 않는다. O-03의 ApprovedOrder 생성자는 계속 order-execution 안에 봉인한다. 반환한 OrderIntent 자체는 승인 권한이 아니다.

현재 시각을 인자로 받아도 호출 직후부터 전송 사이의 물리적 지연까지 타입으로 제거할 수는 없다. O-18은 본인 확인 이후 최신 시각·가격을 수집하고, 트랜잭션 처리 중 지연될 경우 전송 직전까지 다시 확인해야 한다. 이미 만료된 값으로 이 메서드를 호출하면 실패한다. 순서 7에서 주문을 새로 계산하지 않는다.

## 봉인과 보고서

Proposal의 모든 필드는 비공개다. Validated의 비공개 필드는 검증 시각 하나다. Clone·Copy·Default·From·직렬화·역직렬화·가변 접근자·복원 생성자는 없다. 검증 생성 지점은 비공개 finish_validation 하나이며 공개 validate가 직접 계산한 보고서로만 호출한다. 외부 보고서를 인자로 받는 공개 경로가 없다. 단위 테스트는 동일 모듈의 비공개 결과를 설정해 허용 전이를 검사한다. 테스트 생성자나 cfg로 열린 제품 API는 없다.

| ValidationCheck | 현재 계산 주체 | O-01에서 받을 사실과 판정 형태 |
|---|---|---|
| Instrument | Unknown | 종목 마스터 조회로 봉인한 종목·거래 가능 사실을 domain 순수 판정 함수로 검사 |
| MarketHours | Unknown | 봉인된 장 구간, 주문 조건, 현재 시각. 금액·소수 매도 시간 제한도 domain이 판정 |
| AvailableFundsOrQuantity | Unknown | 계좌·통화·기준 시각이 결합된 매수가능금액 또는 매도가능수량 사실과 고정 주문 |
| OppositePendingOrder | Unknown | 시장·종목·방향·기준 시각이 결합된 미체결 사실. 국내 적용 여부는 domain이 계산 |
| BuyWarnings | Unknown | 종목·기준 시각과 닫힌 유의사항 목록. 매도 적용 제외도 domain이 계산 |
| Principles | Unknown | 봉인된 정책·포트폴리오 사실과 고정 주문. domain 정책 판정 |
| Expiry | domain | 생성 시각·만료 시각·현재 시각 |
| PriceRange | domain | 고정 카드·추정 기준가, 양수 변동성, 양수 거래일 기간, 현재가 |

여섯 O-01 자리는 현재 모두 Unknown이며 비공개 배열에 있다. 하나라도 Failed 또는 Unknown이면 거부한다. 비적용도 호출자가 Passed로 넣지 않고 O-01의 domain 함수가 시장·방향으로 판정한다. O-01은 이 모듈을 확장해 봉인된 GateFacts를 받아 항목별 순수 함수 결과로 배열을 계산하도록 한다. 사실 타입의 필드·생성자는 각 입력 검증 경계에 봉인하고, 계좌·종목·시각을 제안과 대조한다. 외부 코드가 CheckResult::Passed를 조립해 주입하는 setter·builder는 앞으로도 열지 않는다. application은 포트로 사실을 수집하지만 결과를 정하지 않는다.

저장된 ValidationReport는 UI 힌트·감사 자료이며 실행 권한이 아니다. O-01 구현 전에는 공개 validate가 항상 실패하는 것이 의도된 동작이다.

## 시각

design 6.5의 10분을 600초 TimeSpan으로 만들고 Timestamp::checked_add로 만료 시각을 계산한다. 이 둘을 and_then으로 연결해 불가능한 고정 기간 오류용 별도 변환 분기를 두지 않았다. 시각 범위 초과는 ProposalError::Time이고 입력을 담지 않는다. `created_at <= now < expires_at`만 유효하다. 정확히 10분이면 만료다. 포함 경계의 명시 규칙은 문서에서 찾지 못해 보수적으로 정했다. 검증 시각이 생성보다 이르거나 소비 시각이 검증 시각보다 이르면 거부한다. 시간은 읽지 않고 인자로만 받는다.

## 가격 범위와 산술

open-issues의 주문·만료·계수 관련 항목을 검색했다. open-issues 1.1의 a·b는 목표·손절 계수이며 제안 만료 계수 후보는 없다. design 6.5는 이 계수를 D-14 ADR에 맡긴다.

**초안 k = 0.5**: 전체 H 기간의 예상 변동 폭 1σ보다 좁은 반 σ 범위만 허용해, 카드가 만들어진 뒤 정보가 크게 변한 주문은 다시 제안하도록 하는 보수적인 출발점이다. 실측·백테스트로 최적화한 값이나 사용자가 확정한 금융 기준은 아니다. 사용자 설정은 늘리지 않는다. 목표·손절 계수 a·b와 혼용하지 않는다.

`|P − P₀| <= 0.5 × σ × √H × P₀`를 양변 제곱해 다음으로 판정한다.

`(P − P₀)² <= (0.5 × σ)² × H × P₀²`

P와 P₀는 양수·동일 통화이며 σ·H도 양수다. H의 단위는 거래일이고 소수 거래일도 허용한다. 카드 기준가와 추정 기준가 양쪽 모두 통과해야 한다. 경계는 포함한다. σ는 D-11 병합 전 최소 양수 비율 ProposalVolatility로 받고, 병합 뒤 Volatility와 연결한다.

exact_arithmetic에 proposal_price_range와 그 내부 squared_range만 추가했다. 기존 subtract·multiply의 정확성 검사를 사용하고 제곱근·부동소수점·새 근사 연산은 추가하지 않는다. 모든 중간 곱을 Decimal에 정확히 표현할 수 있어야 한다. 통화 불일치, 뺄셈·곱셈 넘침, 정밀도 손실로 판정 불가면 Failed(이탈)다. 넓은 정수로 전체 부등식을 직접 비교하면 판정할 수 있는 일부 값도 이 정책에서는 보수적으로 거부한다. 기존 함수와 기존 테스트 본문은 바꾸지 않았으며 새 함수는 Clippy 요구에 따라 테스트 모듈 앞에 삽입했다.

## 제안 ID와 주문 키

ProposalId는 ClientOrderId 형식(1~36 ASCII 영숫자·하이픈·밑줄)을 비공개로 감싼다. 기존 ClientOrderId 파서가 형식을 검증한 값을 ProposalId::new로 감싸 ID를 먼저 만들고 as_client_order_id로 그 ID와 동일한 키를 얻는다. 결정 함수는 항등 대응이다. 짧은 해시·절삭·새 랜덤 값으로 변환하지 않는다. 제안 생성자는 전달된 OrderIntent의 키가 ID에서 나온 키와 같은지 검사한다. 불일치면 OrderKeyMismatch다. 이후 키·조건을 바꿀 접근자는 없다.

발급 유일성과 같은 Draft를 다시 복원·검증하는 경우의 중복 전송은 별개다. O-18은 ProposalId 고유 제약과 전송 전 기록으로 차단한다. O-19는 같은 고정 키·고정 내용의 재시도만 허용하며 재시도 10분은 제안 만료 시각과 다른 전송 프로토콜의 시간이다.

## 읽기 경계와 Debug

id·created_at·expires_at·intent는 읽기 접근자뿐이다. intent의 기존 접근자로 시장·종목·방향·수량·가격·계좌 키·주문 키를 읽을 수 있다. D-32가 수수료·예상 금액·비중·계좌 뒤 네 자리와 키 앞 여덟 자리를 계산해 OrderSummary로 만든다. Proposal은 표시 문자열·마스킹·통화 환산을 계산하지 않는다. 전체 AccountId는 결합 키로만 쓰고 요약이나 UI로 그대로 보내지 않는다.

Proposal의 사용자 정의 Debug는 ID와 생성·만료 시각만 출력하며 OrderIntent와 AccountId는 출력하지 않는다. 기존 OrderIntent 자체의 Debug를 바꾸지는 않았다. 오류 열거형에는 원래 입력값이 없다.

## 검증 증거

만료 경계·역행, 모든 항목의 실패·미확인, 정상 검증·소비, 가격 경계·한 단위 밖, 각 산술 단계 판정 불가, 양쪽 기준가, KRW·USD, ID·키 불일치와 소비 후 유지 테스트를 작성했다. 가격 속성 테스트는 Config::with_cases(4096)이며 σ=n/1000, H=d/100의 독립 i128 정수식 `4×10^8×차이² <= n²×d×기준가²`와 대조한다.

외부 실험은 `cargo build --locked -p domain --lib` 뒤 별도 crate를 rustc 1.99.1로 컴파일했다. domain 내부 테스트의 권한을 사용하지 않았다. `rustc --edition=2024 --crate-type=lib --extern domain=target/debug/libdomain.rlib -L dependency=target/debug/deps <probe.rs>` 형태이며 직렬화 실험에는 기존 작업 공간 빌드의 serde rlib를 --extern으로 추가했다. 의존성을 추가하지 않았다. 실험 파일·stderr·results.json은 target/d14-compile에 있다.

| 외부 실험 | 결과 |
|---|---|
| Draft 생성, validate 호출, Validated 소비 함수 | 각각 exit 0 |
| `Validated { validated_at: t }`, 전부 통과 보고서 리터럴 | E0451 |
| `Proposal::<Validated>::new`, Draft 실행, Validated 재검증·report | E0599 |
| Proposal<Validated>·Validated의 Clone, Proposal의 Copy·Default·From<Draft> | E0277 |
| Proposal의 intent 필드 직접 접근 | E0616 |
| finish_validation 직접 호출 | E0624 |
| 같은 Validated에 into_execution 두 번 | E0382 |
| Proposal<Validated> Serialize·DeserializeOwned, Validated Serialize | E0277 |

대표 소비 실험은 `fn probe(p: Proposal<Validated>, t: Timestamp, c: Price) { let _first = p.into_execution(t,c); let _second = p.into_execution(t,c); }`이다. 새 trait 구현이나 필드 공개를 막는 영속 compile-fail 하네스는 ADR 0022와 동일하게 별도 하네스 작업의 범위다. 이번 외부 실험은 실행한 스냅샷 증거이며 just check에 자동 등록한 회귀는 아니다.

새 의존성·검사 끄기·실제 키·계좌 접근은 없다. 프로젝트 설정 hooks=true와 Git 훅 설치는 확인했으나 현재 세션의 프로젝트·훅 신뢰 적용 여부는 확정하지 못했다. 직접 검사와 커밋 훅의 실제 결과를 사용한다.

## 마지막 변이 실행과 보완

`b662b5d6facadb2e83b74913f861d575353fe4a4`에서 `just mutants-diff 6a6acdc12a5ed53ee522fffab2a3833cf2a9a461`를 한 번 실행했다. 하네스의 변경분 범위는 수정된 crate 전체이므로 654개를 검사했다. 377개 검출, 276개 컴파일 불가, 1개 생존, 타임아웃 0개였다. 생존은 Proposal Debug를 `Ok(Default::default())`로 바꾸는 변이였다. 계좌 키가 없다는 기존 단언만으로는 빈 출력을 잡지 못했다.

새 회귀 debug_exposes_identity_and_times_but_not_account는 타입명·제안 ID·생성 시각·만료 시각이 출력되고 계좌 키는 없는지 검사한다. 동일한 빈 출력 변이를 수동 주입해 해당 테스트가 `rendered.contains("Proposal")` 단언에서 실패(exit 100)하는 것을 확인했다. 변이를 복구하면 같은 테스트가 통과(exit 0)한다. 경고를 낮추지 않고 컴파일하기 위해 수동 변이는 self·formatter 참조를 지역 변수로 받았으며 출력 동작은 원래 생존 변이와 동일하다. 제품 Debug 구현은 바꾸지 않았다.

전체 cargo-mutants는 지시대로 재실행하지 않았다. 따라서 전체 실행의 측정값은 생존 1개이고, 그 한 개를 추가 회귀의 수동 red/green으로 차단했다는 근거를 구분한다. 변이 결과를 최종 SHA의 전체 재측정 생존 0개로 보고하지 않는다.

변이 실행 중 master에 D-08이 병합되어 최종 정리에서 `d4ab5a2287b04757ca63066afa8e57c8bd232622`를 병합했다. lib.rs의 양쪽 모듈을 모두 보존했고 생성물 충돌은 쓰기 스크립트로 다시 만들었다. D-08 제품 코드와 기존 테스트는 수정하지 않았다. 최종 병합 트리는 커밋 훅 전체 검사로 검증한다. 위 654개 변이 결과에 새로 병합된 D-08 코드는 포함되지 않는다.
