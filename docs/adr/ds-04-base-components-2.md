# DS-04 기본 컴포넌트 2

- 상태: 초안
- 기준: `503261f48fe530378274307dfda375c0eba2b9f4`

## 대상 확정

근거는 각 폴더 README·preview.html, 공통 bundle.js·bundle.css·index.d.ts와 design/README.md다.

| 대상       | 산출물 폴더 | 변형과 상태                                                         | 공개 계약                                                                                    |
| ---------- | ----------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Card       | Card        | 제목·동작 있음/없음                                                 | title, actions, children. DS-04에서 면과 머리·본문을 구현하고 DS-11에서 중복 구현하지 않는다 |
| Table      | Table       | 일반·촘촘, 빈 목록·값 있음, 문자·숫자 정렬                          | 숫자는 자식으로 받는 머리·행·칸 조합. 형식화·정렬·선택·통화 변환 없음                        |
| Badge      | Badge       | neutral·primary·up·down·alert, mono                                 | UiText와 닫힌 tone·mono                                                                      |
| StatusBar  | StatusBar   | warning·error·info, 내용 없음·동작 있음/없음                        | 내용 없으면 null, error는 alert, 나머지는 status                                             |
| Dialog     | Dialog      | open·closed, 일반·wide, 동작 있음/없음                              | Base UI Dialog, 닫힌 이유만 onClose에 전달                                                   |
| EmptyState | EmptyState  | 제목, 설명·아이콘·동작 선택                                         | UiText와 기존 Icon 이름, 동작 요소                                                           |
| Skeleton   | Skeleton    | text(기본 3줄), block(chart-height-sm·chart-height·chart-height-lg) | 정적 막대, aria-hidden. 반복 애니메이션 없음                                                 |

Callout은 화면 안에 머무는 별도 피드백 상자라 상태 띠에 포함하지 않는다. HoldingsTable·ScorecardTable, SignalCard·BriefingCard·DisclosureCard, 숫자·차트·채팅·레이아웃 배치는 제외한다. 설정은 design/README.md와 ScreenSettings README에 따라 Dialog다. Dialog README의 성적표 설명은 최신 공통 README의 본문 보조 화면 결정이 앞선다.

## 미해결과 검증 경계

- Badge·StatusBar 유예는 사용자 후속 결정으로 해소한다. tone은 의미 변형이며 DS 안의 빠짐없는 정적 클래스 대응표로 색 토큰에 연결한다. ADR 0040의 tone 이름 기반 색 토큰 판정을 이 범위에서 좁힌다.
- 표의 데이터 columns는 DS-10의 격자 columns 검사와 충돌한다. 브리프가 허용한 복합 표·자식 계약을 사용한다. 공개 하위에도 className·style·rest·DOM 이벤트·불린을 두지 않는다.
- 스크롤은 DS-19·DS-20 및 DS-11 ScrollArea 이후다. overflow·sticky를 추가하지 않는다.
- Skeleton의 60% 마지막 줄 폭은 토큰 근거가 없어 구현하지 않는다. aria-hidden은 bundle.js의 접근성 계약을 따른다. 애니메이션이 없어 움직임 줄이기에서도 정적이다.
- DS-07 실행 환경, DS-15 실제 CSS·양 테마 시각 회귀, DS-16 WKWebView, DS-20 넘침 실측은 후속 범위다. 이번은 Node·Testing Library·jsdom 동작 검증이다.
- 프로젝트 신뢰·훅 정의의 실제 런타임 신뢰는 프로젝트 파일만으로 증명할 수 없다. 로컬 설정 hooks=true는 확인했고 실제 검사를 직접 실행한다. 사용자 신뢰 저장소를 읽거나 바꾸지 않는다.

## 구현과 검사 대응

- Card 4, Table 3, Dialog 4, EmptyState 4, Skeleton 5: 이야기 20개, 각 light·dark. 별도 `feedbackComponentStories`를 기존 이야기 진입점에서 내보낸다. 기존 `baseComponentStories` 393개와 검증 단언은 그대로다.
- Skeleton.lines는 산출물에서 실제 사용한 3·6만 지원한다(bundle.js의 6줄, preview의 3줄). 임의 숫자를 위한 원시 타입 예외나 새 브랜드 생성기를 만들지 않는다.
- Table은 `Table.Root/Head/Body/Row/Header/RowHeader/Cell`이다. `align=end`인 숫자 칸은 오른쪽, 전체 표는 tabular-nums다. 숫자는 DS-05 자식으로 전달한다. 자동 열 형식화·열 배열·행 레코드 API는 만들지 않는다. 빈 Root는 기본 또는 지정 문구를 표시한다. 머리·본문을 조립하는 경우 빈 행 조립도 호출자가 맡는다.
- Dialog는 Base UI Root·Portal·Backdrop·Viewport·Popup·Title·Close를 쓴다. 제목은 자동 ID로 연결하고 aria-modal을 명시한다. `escape-key/outside-press/close-press`에서 `escape/outside/close-button` 값만 내보낸다. 다른 라이브러리 이유를 공개하지 않는다.
- 첫 입력 초점은 input.ts의 `focusDialogInput(ref): void`에서 소비한다. 숨김·비활성 입력을 제외한 첫 입력, 없으면 버튼, 없으면 Popup을 초점 대상으로 쓴다. useEffect·DOM 반환·콜백 ref가 없다. ds-dom은 실제 input.ts 선언의 이 함수로 넘기는 ref 인자만 추가 허용한다. boundaries는 Dialog.tsx 한 소비자만 더한다. 다른 파일과 같은 이름의 가짜 함수·직접 ref.current 읽기는 새 대조군에서 계속 거부한다.
- Base UI 내부 세부 객체의 공개 전달 검사는 유지한다. DS-10은 후속 결정의 의미 변형 증명만 추가한다.
- 추가 스타일은 bundle.css 241의 fixed/inset:0을 옮긴 dialog-overlay, 277의 글줄 높이를 생성 namespace로 옮긴 skeleton-line, 244의 아래 테두리 border-b-hairline, Skeleton README가 지정한 chart-height 세 높이뿐이다. Tailwind 생성 스냅샷은 기존 snapshot 함수로 재생성했다. fixed·inset-0·임의 높이·임의 z-index는 계속 실패한다. 가격색 허용 목록은 후속 결정에서 Badge.tsx만 추가하며 토큰 값·새 의존성·검사 끄기는 없다.
- 기존 테스트 변경은 ds-generation.test.mjs의 정확한 theme 기대에 utility 네 선언 추가, ds-screen-bootstrap.test.mjs의 input 내보내기와 반환 기대에 focusDialogInput/void 추가, knip.test.mjs의 정확한 운영 유예 기대에 새 파일 여섯 개 추가뿐이다. 단언 형태·기존 기대 원소는 유지한다.
- 첫 구현의 knip 운영 유예는 기존 48개에서 새 파일 여섯 개를 추가한 54개였다. 이번에는 Badge.tsx·StatusBar.tsx·Status.stories.tsx 세 개만 더해 57개다.
- 설정은 디자인 산출물에서 Dialog를 쓴다. architecture 5.1의 별도 macOS 설정 창 결정과 산출물 사이에는 간극이 있으므로 이번 작업은 설정 화면을 조립하거나 창 방식을 바꾸지 않는다.

- Dialog.onClose가 없는 경우에는 산출물 bundle.js와 같이 닫기 버튼을 표시하지 않는다. 콜백이 없는 제어 상태는 유지하고 외부 콜백도 발생하지 않는다. 해당 회귀는 수정 전 실패·수정 후 통과를 확인한다.

## 의미 변형 후속 결정

사용자 지시로 `tone`, `variant`, `severity`, `status` 및 해당 접미사의 의미 prop은 컴포넌트 모듈 또는 DS 타입 파일에 선언한 닫힌 문자열 리터럴 유니온을 받는다. 색 자체를 받는 textColor·bg·color의 생성 색 토큰 계약은 유지한다. 직접 생성 토큰을 받던 Text의 기존 API도 유지한다.

대응은 실제 className이 소비하는 const 객체의 리터럴 값 또는 exhaustive switch의 리터럴 반환으로 제한한다. 빠진 값·동적 값·대응표 미사용·넓힌 문자열·템플릿 타입·다른 모듈 유니온의 확장·의미와 생성 토큰 타입 혼합은 실패한다. DS-02와 같은 클래스 집합·변형·파일별 가격색 권한을 사용한다. 임의 문자열의 클래스 소비는 허용하지 않는다. Button과 Skeleton의 기존 variant도 같은 증명 조건에 맞춰 정적 대응표를 실제로 사용한다.

Badge 산출물은 up/down을 가격·수급 방향으로 명시한다. 가격색 허용 목록에는 Badge.tsx 한 파일만 더한다. 화면이 방향이 아닌 곳에 up/down 배지를 쓰는 것은 린트로 막지 못한다. 방향 값은 코어가 보낸 값으로만 쓴다(architecture 5.3). DS-05의 숫자 표시 컴포넌트가 방향 배지를 감싸는 것이 맞는지는 DS-05에서 정한다.

StatusBar는 내용이 없으면 동작까지 포함해 아무것도 그리지 않는다. 기본 warning, error는 alert 역할, warning/info는 status 역할이다. warning/error는 alert-soft·alert, info는 surface-sunken·line·ink-muted를 쓴다. 가장 심각한 문제 하나의 선택은 화면 조립의 책임이다.

후속 이야기: Badge의 다섯 tone × 두 mono 상태 10개, StatusBar의 세 tone × 동작 유무 6개·내용 없음 3개·기본 warning 1개로 10개다. 별도 statusComponentStories 진입점으로 light·dark를 등록한다. 기존 DS-03 393개와 앞선 DS-04 20개는 그대로다.

경고/오류 색은 가격 색과 다르다. 라이트 alert=#a8520a, up=#d0202d, down=#1f5bd1; 다크 alert=#f1a34a, up=#ff5d5d, down=#5e9cff이다. 토큰 자체는 수정하지 않고 두 테마의 색 차이를 테스트한다.

정적 대응표는 별도 ds-semantic-variants 검사 모듈에서 타입 출처·리터럴·키의 완전성·실제 className 소비를 함께 확인한다. 조건식 안에서도 유효한 각 대응만 허용하고 순환 별칭은 거부한다. 타입을 옮기거나 인라인 끄기로 열린 클래스 경로를 만들지 않는다. DS-10 손 변형 15개는 유지한다. 기존 변형 테스트에는 새 검사 모듈의 import 경로 해석만 추가한다.

기존 테스트 기대 변경: ds-internal-review.test.mjs 가격색 목록에 Badge.tsx 한 경로, knip.test.mjs 운영 유예 목록에 새 DS 파일 세 경로를 추가한다. DS-04 충돌 회귀 둘은 실제 컴포넌트의 모든 린트 진단이 없는 통과 대조군으로 바꾸고, DS-04 타입 행에 두 컴포넌트의 양성·음성 사례를 추가한다. 단언 형태를 유지한다. Tailwind 클래스 스냅샷은 Badge 산출물 78행의 whitespace-nowrap 한 키워드에 대해서만 재생성한다.

후속 기준은 `1e2b4f34b8dffaa679fbc5e90f08276f23d6f122`이다. 마무리 때 진행된 로컬 master `6e966736fd33f1aa9143c595835016be0fbb5dd7`까지 병합한다. crates의 병합 결과는 해당 master와 동일하며 직접 편집하지 않는다. 충돌한 권한 설정과 훅 해시는 저장소 스크립트로 다시 만든다.

최종 의미 변형 예제는 통과 12개·위반 23개다. DS 하위 타입 파일도 직접 가져올 수 있으며, 생성 토큰을 선택하는 Extract/Exclude 안의 템플릿 타입도 허용하지 않는다. 구문 전용 린트처럼 program이 없는 환경에서는 새 대응표 경로를 허용하지 않고 기존 리터럴 클래스 판정을 유지한다.

가격색 권한은 대응표·switch 선언 파일과 실제 클래스 소비 파일 양쪽에서 확인한다. Badge의 가격색 대응표를 다른 DS 파일로 가져가 쓰는 객체·switch 경로는 실패하고, 가격색 없는 공유 대응표는 통과한다. 예제에서 바꾼 의존 파일도 먼저 린트해 TypeScript의 이전 모듈 캐시가 음성 사례를 가리지 않도록 한다.

통합 검사의 기준은 수용한 master `6e966736fd33f1aa9143c595835016be0fbb5dd7`이다. 커밋 훅이 HEAD와 master의 분기점을 사용하므로, master에서 이미 수용한 application/ports 테스트 변경을 다시 수정으로 보지 않도록 master를 첫 부모로 통합 커밋을 만들고 ds-04를 fast-forward한다. 검사·기존 테스트·master 참조는 바꾸지 않는다.
