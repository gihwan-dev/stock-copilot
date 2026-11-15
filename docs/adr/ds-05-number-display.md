# DS-05: 코어가 정한 숫자의 표시

- 상태: 초안
- 관련 작업: DS-05, D-01, DS-04, DS-10, DS-19, DS-20

## 결정

숫자는 코어가 반올림과 자릿수 결정을 끝낸 십진수 문자열이다. 프런트는 부호, 쉼표, 통화와 단위만 붙인다. R-14의 Intl 실험은 지원 가능성의 근거이며, 이번 명시 지시가 Intl 사용보다 앞선다. 비율 0.1516을 15.16%로 옮기는 환산도 코어가 한다. Percent와 Probability의 입력은 퍼센트로 이미 환산된 표시 숫자다. 산출물 index.d.ts의 rate 설명도 2.43을 2.43%로 정의한다. 프런트는 곱셈·나눗셈·반올림을 하지 않는다.

architecture 5.1은 숫자 컴포넌트 이름과 넘침 규칙을 지정하지만 숫자의 prop 표는 없다. design/components/index.d.ts의 Money/Percent value와 PriceChange value/rate를 바탕으로 원시 문자열 대신 불투명 값만 받는다. 표시에 불필요한 size·signed·pill은 추가하지 않는다. PriceChange의 금액 이름은 의미가 분명한 amount로 한다. 통화는 Money 안의 통화를 쓴다. Quantity·Probability는 value 하나다. Money만 full/compact notation을 선택한다. 기본 full은 코어 자릿수를 모두 보존한다. compact도 자릿수를 줄이지 않고 정수부를 네 자리씩 나누어 만·억으로 읽는다. 소수 부분은 원 단위 부분에 붙여 보존한다. 큰 억 개수는 세 자리 쉼표로 구분하며 조 단위를 새로 만들지 않는다.

공개 컴포넌트는 Money(value, notation?), Percent(value), Quantity(value), Probability(value), PriceChange(amount, rate, direction), DirectionBadge(direction) 여섯 개다. 방향은 up/down/flat 닫힌 값이다. 보합은 중립색과 – 기호, 상승은 빨강과 ▲, 하락은 파랑과 ▼다. PriceChange의 방향 기호는 role=img와 messages.ts의 접근성 이름을 가진다. 실제 role/name 조회로 검증하며 색에 의존하지 않는다. 방향과 입력 부호가 다르면 방향 prop이 부호와 색을 결정한다. ADR 0025(D-01)대로 표시 0이면 숫자의 부호만 제거하며 원래 방향은 유지한다. 일반 금액·비중·수량·확률에는 가격색을 붙이지 않는다.

formatNumber는 판별 유니온 입력을 받고 available/unavailable과 표시 문자열을 반환한다. malformed 십진수는 unavailable/—다. 문자열 문법 파싱도 마지막 줄바꿈을 거부한다. 오류 때 예외·로그·추가 사용자 절차를 만들지 않는다. FormattedNumber는 브랜드 문자열이며 화면 문구 UiText와 구분한다. 단위·기호·방향 이름·자리 표시는 messages.ts에 둔다.

숫자는 tabular-nums와 whitespace-nowrap만 쓴다. 잘라내기·말줄임·고정 폭은 없다. 부족한 폭은 숫자를 담은 표·카드의 가로 스크롤이 담당한다. 숫자 자체에 중첩 스크롤은 만들지 않는다. 토큰 up/down/ink-muted가 라이트·다크를 결정한다. 모든 공개 컴포넌트의 목록 이야기와 양 테마, 방향별 이야기를 제공한다.

## 승인된 보호 변경

| 파일 | 이유 |
|---|---|
| apps/desktop/src/shared/lib/money.ts | 불투명 키를 공개하지 않고 readDecimalForNumberDisplay 함수 하나로 표시 문자열만 읽음. 십진수 파싱의 마지막 줄바꿈 거부 |
| apps/desktop/src/shared/lib/brand.ts | 표시 문자열 브랜드 선언 |
| apps/desktop/src/shared/lib/index.ts | 형식화 함수와 방향·표시 타입 공개. 읽기 함수는 재수출하지 않음 |
| apps/desktop/src/shared/ds/index.ts | 여섯 숫자 컴포넌트 공개 |
| apps/desktop/eslint/rules/number-display-boundary.mjs | 실제 money.ts 선언을 추적하여 읽기 함수의 별칭·재수출·참조를 money.ts/format.ts 밖에서 거부. format의 Number/parseFloat/parseInt/Intl 금지. 화면·DS의 기존 숫자 변환 검사와 차트 전용 권한은 유지 |
| apps/desktop/eslint/app-config.mjs, local-plugin.mjs | 새 경계 검사 등록과 끄기 금지 |
| apps/desktop/eslint/rules/no-computed-member.mjs | 정확한 money.ts 안의 unique symbol 읽기만 허용. 화면에는 열지 않음 |
| apps/desktop/eslint/rules/no-money-coercion.mjs | 정확한 money.ts 안의 in 검사만 허용. 화면 산술·키 추출 금지는 유지 |
| apps/desktop/eslint/rules/no-ui-text.mjs | format.ts가 만든 FormattedNumber를 JSX에서 표시 허용. 문자열·템플릿 직접 작성 금지는 유지 |
| apps/desktop/eslint/ds-allowlists.mjs | Numbers.tsx 한 파일만 가격색 사용 허용 |
| harness/eslint-violations/ds05-number-display.json | 읽기 경계·변환·직접 가격색 위반 및 허용 대조군 |

| harness/knip-check/production-allowlist.json | 첫 화면 사용처 전 Numbers.tsx·Numbers.stories.tsx·format.ts 세 파일만 기존 운영 유예 방식으로 등록. 남은 항목과 실제 보고 차이는 계속 실패 |

새 외부 의존성·검사 끄기는 없다. Rust impl은 수정하지 않는다.

## 예제 대조

| 문서/입력 | 출력 |
|---|---|
| R-14 원화 84312500 | 84,312,500원 |
| R-14 달러 182.34 | $182.34 |
| R-14 300, 0.123456 | 300주, 0.123456주 |
| R-14 상승 1700, 2.43 | ▲ +1,700원 (+2.43%) |
| R-14 하락 -850, -1.12 | ▼ −850원 (−1.12%) |
| R-14 비중 48.0 | 48.0% |
| R-14 compact 84312500 | 8,431만 2,500원 |
| R-14 만 단위 예제 1234567898 | 12억 3,456만 7,898원 (R-14 혼합 표기 방식의 그룹 내부 쉼표 적용) |
| R-14 큰 십진수 12345678901234567.89 | $12,345,678,901,234,567.89 |
| R-11 비율 0.1516 → 코어 표시 15.16 | 15.16% |
| R-11 환산 예제 코어 표시 16875000 | 16,875,000원 |
| ADR 0025 코어 표시 -0.00 / down | ▼ 0.00원 (0.00%) |

R-14의 Intl 기본 반올림·통화 접두어·조 단위 실험 출력은 채택한 제품 표기가 아니다. 원화 ₩, 달러 US$, 조 단위, 받은 자릿수 절삭을 재현하지 않는다.

## 검증

구현 전에 새 테스트를 실행하여 누락된 export로 실패하는 red를 확인했다. 문서 예제와 큰 값·끝자리 0·음수 0·방향 불일치·형식 불가 입력의 실행 결과는 target/DS-05에 보존한다. 기존 테스트 변경은 사용자가 승인한 정확한 목록과 린트 기대값 갱신뿐이다. 프로젝트/훅 신뢰 저장 상태는 홈 설정을 조회하지 않아 미확인이다. 저장소 hooks=true와 Git 훅 설치는 확인했다. 조회용 core.hooksPath probe가 실행되어 Codex 명령 가드 적용은 확인되지 않았다. 실제 검사를 직접 실행하며 전체 검사와 test-diff 판정을 구분하고, 사용자가 승인한 test-diff 예외만 기록한다.

## 인계

- DS-07: numberComponentStories를 실제 목록 화면과 시각 회귀 실행 환경에 연결한다.
- DS-16: 실제 WKWebView 숫자 렌더링과 글꼴을 확인한다.
- DS-19·DS-20: 숫자를 담는 카드·표의 가로 스크롤과 실제 창 넘침 검증을 이어간다.
- IPC/화면 연결 작업: 코어에서 환산·반올림·방향이 확정된 표시 값을 전달한다. 원시 비율을 Percent에 직접 넘기지 않는다.


생성물 .codex/config.toml은 새 ADR 보호 경로를 등록하고, harness/claude-hooks/hashes.json은 그에 따른 설정 해시를 기존 생성기로 갱신한다.

## 승인된 기존 기대값 변경

| 파일 | 전 | 후 |
|---|---|---|
| apps/desktop/tests/ds-internal-review.test.mjs | PRICE_COLOR_FILES: [] | PRICE_COLOR_FILES: ["src/shared/ds/Numbers.tsx"] |
| apps/desktop/tests/eslint-config.test.mjs | 끄기 금지 목록에 새 규칙 없음 | local/number-display-boundary 한 항목 추가 |
| apps/desktop/tests/knip.test.mjs | 새 숫자 표시 파일 없는 정확한 운영 유예 목록 | Numbers.tsx·Numbers.stories.tsx·format.ts 세 파일만 추가 |
| apps/desktop/tests/ds-final-runtime.test.mjs | format.ts의 Intl.NumberFormat 예제 오류 [] | 새 conversion 오류 한 개의 규칙·심각도·메시지·위치 전체를 정확히 단언 |
| harness/eslint-violations/cases.json | 숫자 규칙 목록에 표시 경계 검사 없음 | local/number-display-boundary 추가 |
| harness/eslint-violations/cases.json | format.ts는 허용 목록 대조군으로 기대 오류 없음 | Number·Intl 위반의 정확한 conversion 메시지 기대 |
| harness/eslint-violations/cases.json | reader-export.ts 미등록 | 읽기 함수 재수출의 정확한 reader 메시지 기대 |

중앙 money.ts 예제에는 읽기 함수를 추가하고, format.ts에는 합법 읽기 사용과 금지 변환 증인을 함께 둔다. 승인된 목록은 정확한 원소 목록으로 유지하고, 기존 테스트의 입력과 나머지 단언은 보존한다.

## 손 변형 실행 결과

최종 접근성 보완을 포함한 현재 코드의 복사본에서 컴파일을 통과한 24개를 실제 실행하여 24개 모두 테스트 실패로 검출했다. 7번은 컴파일 실패여서 검출 수에서 제외한다. 상세 로그와 원본 복사본은 target/DS-05/mutations.json이 가리키는 임시 디렉터리에 있다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 결과 |
|---|---|---|
| 1 | 쉼표 제거 | document examples 검출 |
| 2 | 두 자리 그룹 | document examples 검출 |
| 3 | compact 경계 4→5 | document examples 검출 |
| 4 | 하위 그룹 세 자리 | document examples 검출 |
| 5 | 상위 그룹 세 자리 | document examples 검출 |
| 6 | 만·억 교체 | document examples 검출 |
| 7 | 중간 만 그룹 누락 |  컴파일 실패 |
| 8 | 끝 0 그룹 표시 | document examples 검출 |
| 9 | 음수 부호 미제거 | document examples 검출 |
| 10 | compact 미적용 | document examples 검출 |
| 11 | 소수 구분자 누락 | document examples 검출 |
| 12 | compact 소수 0 누락 | document examples 검출 |
| 13 | 음수 0 부호 유지 | document examples 검출 |
| 14 | 상승 부호 반전 | direction overrides 검출 |
| 15 | 하락 부호 하이픈 | direction overrides 검출 |
| 16 | 일반 음수 부호 제거 | document examples 검출 |
| 17 | 원 접미 누락 | document examples 검출 |
| 18 | 주 접미 누락 | document examples 검출 |
| 19 | 퍼센트 접미 누락 | document examples 검출 |
| 20 | 통화 접두 반전 | document examples 검출 |
| 21 | 상승 가격색 반전 | direction overrides 검출 |
| 22 | 하락 접근성 이름 반전 | direction overrides 검출 |
| 23 | 하락 기호 반전 | direction overrides 검출 |
| 24 | 방향 전달 제거 | direction overrides 검출 |
| 25 | 방향 기호 접근성 역할 제거 | direction overrides 검출 |
