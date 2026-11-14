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

- Badge와 StatusBar의 산출물 tone은 생성 색 토큰 이름과 다르다. DS-10은 tone에 생성 토큰 참조를 요구한다. 공개 계약을 넓히는 예외는 이번 내부 오탐 보정 허용에 포함되지 않는다. 해당 컴포넌트는 규칙을 유지한 채 미루고 나머지를 구현한다.
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
- Base UI 내부 세부 객체의 공개 전달 검사에는 변경이 없다. DS-10 공개 props 규칙도 변경하지 않는다.
- 추가 스타일은 bundle.css 241의 fixed/inset:0을 옮긴 dialog-overlay, 277의 글줄 높이를 생성 namespace로 옮긴 skeleton-line, 244의 아래 테두리 border-b-hairline, Skeleton README가 지정한 chart-height 세 높이뿐이다. Tailwind 생성 스냅샷은 기존 snapshot 함수로 재생성했다. fixed·inset-0·임의 높이·임의 z-index는 계속 실패한다. 가격색 허용 목록은 비어 있고 토큰 값·새 의존성·검사 끄기는 없다.
- 기존 테스트 변경은 ds-generation.test.mjs의 정확한 theme 기대에 utility 네 선언 추가, ds-screen-bootstrap.test.mjs의 input 내보내기와 반환 기대에 focusDialogInput/void 추가, knip.test.mjs의 정확한 운영 유예 기대에 새 파일 여섯 개 추가뿐이다. 단언 형태·기존 기대 원소는 유지한다.
- knip 운영 유예는 기존 48개에서 새 파일 여섯 개만 추가한 54개다.
- 설정은 디자인 산출물에서 Dialog를 쓴다. architecture 5.1의 별도 macOS 설정 창 결정과 산출물 사이에는 간극이 있으므로 이번 작업은 설정 화면을 조립하거나 창 방식을 바꾸지 않는다.

- Dialog.onClose가 없는 경우에는 산출물 bundle.js와 같이 닫기 버튼을 표시하지 않는다. 콜백이 없는 제어 상태는 유지하고 외부 콜백도 발생하지 않는다. 해당 회귀는 수정 전 실패·수정 후 통과를 확인한다.
