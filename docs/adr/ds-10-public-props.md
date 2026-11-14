# ADR 초안: DS-10 공개 props 계약

- 상태: 제안
- 기준: af70b46fc6c62a0d1a7b036c9977b3d2f3d47366
- 근거: harness 4.4 #9~11, 4.2 숫자·React 행, ADR 0009·0033·0036

## 결정

`ds/public-props`는 shared/ds의 대문자로 시작하는 공개 값과 그 호출 가능한 속성의 첫 매개변수를 실제 TypeScript checker로 검사한다. 별칭·Pick·Omit·교차·상속·제네릭 제약을 해석한다. className/style, 인덱스 시그니처, 객체 rest 및 JSX spread를 거부한다. forwardRef는 콜백 ref를 공개하므로 거부한다. ref는 React 선언에서 온 RefObject와 선택적 undefined만 허용한다. DOM과 React 이벤트는 중첩 객체·배열·콜백 인자와 반환값까지 검사한다.

간격·색·크기·글자 모양의 표준 속성은 생성 tokens.ts를 참조해야 하며, 실제 생성된 이름만 허용한다. 토큰 유니온을 복사하거나 자유 문자열·숫자로 넓힌 타입은 거부한다. 토큰 생성기를 바꾸지 않는다. 이름 삭제에 따른 TS 7·TS 6 컴파일 실패와 콜백 ref의 컴파일 실패를 별도로 고정한다.

대문자 JSX 컴포넌트의 숫자 및 중첩 숫자 props도 검사한다. 숫자 변환 예외는 기존 보호된 CHART_NUMBER_ALLOWLIST만 따른다. 목록은 비어 있다. 차트 데이터는 코어의 십진수 문자열로 전달하고 숫자 변환은 DS-06에서 해당 파일 등록 후 수행한다. 스타일이 아닌 데이터에는 닫힌 숫자 리터럴 유니온을 허용하고 columns·span의 3·4·6·8·9·12 및 구간별 객체를 구별한다.

문구·식별자·십진수 문자열 브랜드와 값만 넘기는 콜백은 허용한다. 문구 브랜드로 감싼 계산된 클래스도 공개 데이터로서는 문구와 구별하지 않는다. 그 값을 className·cn으로 소비하거나 객체·JSX spread로 전달하는 것은 기존 static-classes와 새 공개 props 규칙이 함께 거부한다. 기존 문자열 추측 검사를 강화하지 않는다.

## 검증

위반 예제는 harness/eslint-violations/ds10-public.json에 두고 실제 앱 설정을 읽는 OS 임시 프로젝트에서 검사한다. 규칙을 끄면 해당 진단이 없어지고, 켜면 진단이 생기는 것을 각 예제에 확인한다. 인라인 끄기는 기존 no-restricted-disable 목록에 자동 등록된다. 기존 eslint-config.test.mjs의 정확한 목록 기대에 새 규칙 하나를 추가한다. 기존 단언은 삭제하거나 약화하지 않는다. 새 의존성은 없다. 기존 cases.json의 HTML props 상속·JSX 전개 예제 기대에 public-props 진단을 더한다. 타입 checker가 없는 구문 전용 검사는 이 규칙의 대상이 아니며 실제 앱은 타입 인식 설정으로 검사한다.

## 한계

대문자 공개 callable을 컴포넌트로 취급한다. 정적 타입에 없는 런타임 동적 export와 비정상 TypeScript 프로그램의 의미는 보장하지 않는다. 유효한 소스의 컴파일 검사가 먼저 필요하다. 이중 as 단언으로 원래 props를 지우면 public-props만으로 원형을 복원하지 못한다. 실제 앱의 consistent-type-assertions가 이 경로를 거부함을 별도로 시험한다. 임의 prop 이름이 실제 스타일의 의미를 가지는지는 타입만으로 판정할 수 없다. 표준 스타일 prop 검사와 동적 클래스·DOM 소비 금지가 함께 경계를 만든다. amount·money·price는 스칼라를 거부하고 넓은 숫자가 없는 객체 계약을 요구한다. 이 구조 검사는 DTO의 코어 출처나 구체적인 필드·통화·방향 의미를 증명하지 않는다. 해당 의미 검증은 실제 코어 DTO와 금액 표시 컴포넌트가 생기는 DS-05에서 함께 확인해야 한다. 현재 DTO를 새로 발명하지 않는다.

## 적대적 리뷰 수정

공개 값의 호출 시그니처와 속성을 재귀로 검사한다. `Table.Row` 형태, `Object.assign` 교차 타입, 두 단계 중첩도 같은 계약이다. 순환은 방문 집합으로 끊고 공개 값 탐색은 8단계, props 값 탐색은 32단계에서 실패시킨다.

표준 lib와 React 선언의 내부 멤버는 펼치지 않는다. Promise·ReadonlySet·ReadonlyMap·ReadonlyArray는 타입 인자를 검사하고, FC·ComponentType은 사용자 props만 검사한다. DOM 출처와 React의 `Event`로 끝나는 이벤트 선언은 이 생략 전에 거부한다. 따라서 정상 라이브러리 내부 제네릭·any는 새 API 누출로 오인하지 않고 Promise<HTMLElement>·ReadonlySet<MouseEvent>·FC<{className:Text}>는 계속 거부한다.

공개 props 전체는 구체 타입이어야 한다. `<T extends object = ...>(props:Readonly<T>)`는 거부한다. 구체적인 props 객체의 데이터 자리에 있는 제약 없는 T는 허용한다. 그 T가 요소 속성에 닿으면 별도의 openGeneric 검사로 거부한다. JSX 전개·객체 rest는 public-props/static-classes, DOM 접근은 no-dom-values, 스타일 소비는 static-classes/no-presentation-props가 계속 막는다. 제약이 있는 T는 그 제약을 검사한다.

숫자 정책의 근거는 harness 4.4 #10의 "간격, 색, 크기, 글자 모양을 받는 props는 토큰 이름의 유니온 타입이다. 숫자나 자유 문자열을 받지 않는다"와 architecture 5.1의 "격자 항목이 차지하는 열 수는 3, 4, 6, 8, 9, 12 중 하나만 고를 수 있고, 구간별로 지정한다"이다. 금지 범위는 스타일 숫자이며 모든 숫자 리터럴을 뜻하지 않는다. 일반 데이터의 닫힌 유한 숫자 리터럴 유니온(`level:1|2|3`)을 허용한다. `columns`·`span`은 3·4·6·8·9·12와 narrow·medium·wide 구간별 객체만 허용한다. `number`, `number`가 섞인 넓은 유니온, 계산으로 넓어진 숫자는 계속 거부한다. 크기·간격 등의 숫자 리터럴도 토큰 계약에 따라 계속 거부한다.

textColor·bg·tone과 중첩 객체·배열의 스타일 필드에도 토큰 검사를 적용한다. 색·간격·반경·fontFamily는 해당 생성 토큰 분류와 이름을 함께 검사한다. 나머지 글자 세부 모양·복합 size 등의 분류는 전체 토큰 이름 집합까지만 검사하며 DS-03의 컴포넌트별 타입 테스트로 정확한 토큰 그룹을 고정해야 한다. 임의의 새 prop 이름이 스타일인지 의미만으로 추론하지는 않는다. `export default`와 클래스 컴포넌트는 기존 내보내기·클래스 금지 규칙이 막는다.

손 변형은 규칙 모듈을 메모리에서만 바꾸어 15개 분기를 각각 제거한다. React 이벤트, forwardRef, 중첩 인덱스, DOM 출처, 제네릭 제약, 콜백 반환값, 배열 원소를 포함해 각 분기만으로 잡히는 증인을 고정한다. 실제 규칙이 증인을 거부하고 해당 분기를 지운 규칙은 같은 증인을 놓치는지 검사한다. 새 의존성이나 기존 master 테스트의 변경은 없다.
