# ADR 초안: DS-10 공개 props 계약

- 상태: 제안
- 기준: af70b46fc6c62a0d1a7b036c9977b3d2f3d47366
- 근거: harness 4.4 #9~11, 4.2 숫자·React 행, ADR 0009·0033·0036

## 결정

`ds/public-props`는 shared/ds의 대문자로 시작하는 공개 callable의 첫 매개변수를 실제 TypeScript checker로 검사한다. 별칭·Pick·Omit·교차·상속·제네릭 제약을 해석한다. className/style, 인덱스 시그니처, 객체 rest 및 JSX spread를 거부한다. forwardRef는 콜백 ref를 공개하므로 거부한다. ref는 React 선언에서 온 RefObject와 선택적 undefined만 허용한다. DOM과 React 이벤트는 중첩 객체·배열·콜백 인자와 반환값까지 검사한다.

간격·색·크기·글자 모양의 표준 속성은 생성 tokens.ts를 참조해야 하며, 실제 생성된 이름만 허용한다. 토큰 유니온을 복사하거나 자유 문자열·숫자로 넓힌 타입은 거부한다. 토큰 생성기를 바꾸지 않는다. 이름 삭제에 따른 TS 7·TS 6 컴파일 실패와 콜백 ref의 컴파일 실패를 별도로 고정한다.

대문자 JSX 컴포넌트의 숫자 및 중첩 숫자 props도 검사한다. 숫자 변환 예외는 기존 보호된 CHART_NUMBER_ALLOWLIST만 따른다. 목록은 비어 있다. 차트 데이터는 코어의 십진수 문자열로 전달하고 숫자 변환은 DS-06에서 해당 파일 등록 후 수행한다. DS-11의 문서에 명시된 columns 3·4·6·8·9·12 리터럴만 공개 타입 검사에서 구별한다.

문구·식별자·십진수 문자열 브랜드와 값만 넘기는 콜백은 허용한다. 문구 브랜드로 감싼 계산된 클래스도 공개 데이터로서는 문구와 구별하지 않는다. 그 값을 className·cn으로 소비하거나 객체·JSX spread로 전달하는 것은 기존 static-classes와 새 공개 props 규칙이 함께 거부한다. 기존 문자열 추측 검사를 강화하지 않는다.

## 검증

위반 예제는 harness/eslint-violations/ds10-public.json에 두고 실제 앱 설정을 읽는 OS 임시 프로젝트에서 검사한다. 규칙을 끄면 해당 진단이 없어지고, 켜면 진단이 생기는 것을 각 예제에 확인한다. 인라인 끄기는 기존 no-restricted-disable 목록에 자동 등록된다. 기존 eslint-config.test.mjs의 정확한 목록 기대에 새 규칙 하나를 추가한다. 기존 단언은 삭제하거나 약화하지 않는다. 새 의존성은 없다. 기존 cases.json의 HTML props 상속·JSX 전개 예제 기대에 public-props 진단을 더한다. 타입 checker가 없는 구문 전용 검사는 이 규칙의 대상이 아니며 실제 앱은 타입 인식 설정으로 검사한다.

## 한계

대문자 공개 callable을 컴포넌트로 취급한다. 동적으로 조립한 export와 비정상 TypeScript 프로그램의 의미는 보장하지 않는다. 유효한 소스의 컴파일 검사가 먼저 필요하다. 이중 as 단언으로 원래 props를 지우면 public-props만으로 원형을 복원하지 못한다. 실제 앱의 consistent-type-assertions가 이 경로를 거부함을 별도로 시험한다. 임의 prop 이름이 실제 스타일의 의미를 가지는지는 타입만으로 판정할 수 없다. 표준 스타일 prop 검사와 동적 클래스·DOM 소비 금지가 함께 경계를 만든다. amount·money·price는 스칼라를 거부하고 숫자 없는 객체 계약을 요구한다. 이 구조 검사는 DTO의 코어 출처나 구체적인 필드·통화·방향 의미를 증명하지 않는다. 해당 의미 검증은 실제 코어 DTO와 금액 표시 컴포넌트가 생기는 DS-05에서 함께 확인해야 한다. 현재 DTO를 새로 발명하지 않는다.
