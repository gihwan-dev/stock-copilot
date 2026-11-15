# ADR 0077: A-19 자체 점검 판정

- 상태: 확정 (2026-11-15)


## 결정

리뷰 수정 기준은 37295f64b69befe7df3af8c84dc0d5c685ca60f0이다. 수정 시작의 master 병합은 이미 최신이었다. 마무리 병합은 master cc4169f026e2a74aa778903aa694465576091849의 D-38을 받았으며 생성물 두 파일의 충돌은 지정 스크립트로 다시 생성해 해결했다. 이 문서는 메인이 확정한 리뷰 수정 결정으로 이전 초안을 대체한다. design 2.6, P-7, UX-5, REL-5와 ADR 0063·0065·0071·0073·0074를 따른다.

SelfCheckInput의 이름 있는 8개 필드가 전체 점검 기록이다. CheckState<Problem>의 Problem은 슬롯마다 별도 닫힌 타입이며 정상·대기·버그·데이터 없음은 공통 진단 상태다. 동일 의미의 NeedsAction/Classified 이중 표현과 잘못된 칸의 조치를 조용히 버리는 규칙을 제거했다. 7번은 기존 AiUsage를 재사용한다. 뉴스·공시와 선택 자격 오류는 별도 SourceCheckRecord 목록으로만 받는다. 문장·IP·키·원문 오류를 만들거나 보관하지 않는다.

판정은 금융 계산이 아닌 application 운영 상태 규칙이다. architecture 3.2에서 domain은 application을 의존할 수 없고, 3.4의 계산 규칙은 금융 값·확률·수량·가격에 해당한다. ErrorClass·AiUsage·IsolationFailure·FeedSource를 소유한 application에 유지한다. 외부 의존성·허용 목록 추가는 없다.

## 지적별 처리

| 지적 | 결정 |
|---|---|
| 1 미설치·미로그인 누락 | isolation_check_observation이 NotInstalled를 4번 Install, LoginRequired를 5번 Login, 그 밖을 6번 Environment로 소진 변환한다. AiEnvironmentProblem에는 NotInstalled와 LoginRequired가 없다. |
| 2 공통 관측의 잘못된 조합 | 고정 배열 대신 칸별 Problem 타입을 가진 SelfCheckInput으로 바꾼다. OptionalCredential은 8칸에서 제거하고 SourceCheckRecord::OptionalCredential로 기록한다. |
| 3 권한·계좌 문제 누락 | 1번 TossKeyProblem을 InvalidKey / Permissions / AccountNotFound로 정의한다. 셋 모두 띠 대상이다. 승인된 design 2.6 첫 행에 권한·계좌 안내 문장을 추가한다. |
| 4 동시 변경 이벤트 누락 | 원인 인자 없이 4~8번 필드만 비교한다. 1~3번과 4~8번이 동시에 바뀌어도 Publish다. |
| 5 미사용 전체 비교 | self_check_difference와 SelfCheckDifference를 삭제한다. |
| 6 중복 띠 값 | SelfCheckProblem·SelfCheckKind·띠의 resumes_at를 삭제한다. banner는 Option<SelfCheckAction>이며 점검은 조치 종류로 정해진다. 재개 시각은 7번 AiUsage 기록에 보존한다. |
| 7 주문 사유 해제 | 다음 주문 시도가 그 사유 없이 접수되거나 주문 스위치가 꺼지면 관측자가 해제한다. |
| 8 인계 | 띠의 두 이벤트 재조회, 고급 상태의 숨은 기록 표시와 소스 상태 이벤트 연결을 아래 작업별로 명시한다. |

## 점검과 띠 기준

| 순서 | 점검 | 띠 대상 | design 2.6 근거 및 조치 |
|---|---|---|---|
| 1 | 토스 키 | 키·권한·계좌 문제 | “키를 다시 복사해 붙여 넣으세요.” / “토스 키의 권한을 확인하세요.” / “토스에서 계좌 상태를 확인하세요.” |
| 2 | 허용 IP | IP 등록 | “WTS의 허용 IP 관리에 추가하세요.” |
| 3 | 같은 키의 중복 사용 | 다른 프로그램 종료 | “같은 토스 키를 쓰는 다른 프로그램을 끄면 다시 연결됩니다.” |
| 4 | Claude Code 설치 | 미설치 | “Claude Code를 설치하면 AI 기능이 켜집니다.” |
| 5 | Claude Code 로그인 | 미로그인 | “Claude Code에 로그인하면 AI 기능이 켜집니다.” |
| 6 | AI 실행 환경 | 없음 | “주 화면에 띄우지 않는다. 고급 > 상태에 사유를 남긴다” |
| 7 | AI 사용 한도 | 없음 | “주 화면에 띄우지 않는다. 고급 > 상태에 재개 시각을 남긴다” |
| 8 | 주문 사전 요건 | 위험 안내·거래소 연동·달러 매수 가능 금액 | “오류별 한 문장”, “위험 안내를 먼저 확인해야 주문할 수 있습니다.” |

사용자 조치 중 앞 순서 하나만 띠에 반환하며 가려진 기록도 전부 보존한다. 정상·대기·버그·데이터 없음은 “사용자가 할 일이 있는 문제만” 규칙에 따라 띠 대상이 아니다. 6·7번과 소스 실패는 8번 조치를 가리지 않는다. 선택 자격 오류도 ADR 0071에 따라 고급 상태 기록에만 남는다.

## 변환과 이벤트 계약

toss_check_observation은 모든 ErrorClass와 UserAction을 소진 match한다. CheckCredentials / CheckPermissions / CheckAccount는 1번의 세 문제, CheckIpAllowlist는 2번, StopCompetingKeyUse는 3번으로 변환한다. 반환값은 해당 칸만 갱신하는 관측이며 다른 칸을 정상으로 초기화하지 않는다. 칸을 지정할 사용자 조치가 없는 Wait·Bug·NoData는 토스 연결의 첫 칸에 진단으로 기록한다. 지연·진단·데이터 종류는 그대로 보존한다. 복구 관측은 해당 칸의 Healthy다.

isolation_check_observation의 반환값도 지정한 설치·로그인·환경 칸만 갱신한다. AI-13은 같은 의미를 다른 조치로 수작업 변환하지 않는다. 복구 시 해당 칸을 Healthy로 바꾼다. 전용 환경 타입 때문에 설치·로그인 실패를 6번에 넣을 수 없다.

self_check_change(previous, next)는 4~8번 필드가 전부 같으면 Unchanged, 하나라도 다르면 Publish다. 토스만 변하거나 소스·선택 자격 기록만 변한 경우는 Unchanged다. 토스 문제에 가려진 4~8번 변화, 토스 연결 변화와 동시에 반영된 4~8번 변화도 Publish다. 판정은 저장·발행·주기 실행을 하지 않는다.

## 인계

- T-02: A-05 분류를 toss_check_observation으로 바꿔 지정된 토스 칸만 갱신한다. 연결 차단·회복 사건은 저장 후 BrokerConnectionChanged를 발행한다. 같은 결과에 4~8번 변화도 반영되면 SelfCheckChanged 역시 발행 대상이다.
- AI-13·AI-16: IsolationFailure는 isolation_check_observation으로 변환한다. 미설치는 4번, 미로그인은 5번, 나머지 실패는 6번이다. AI-05는 AiUsage와 재개 시각을 7번에 보존한다.
- T-16·O-01: 8번 사유는 다음 주문 시도가 그 사유 없이 접수되거나 주문 스위치가 꺼지면 해제한다. 이 작업은 관측된 현재 상태만 판정한다. 실제 주문 거절 정규화는 T-16·O-01·O-09가 맡으며 조회 Bug를 주문 사용자 조치로 재해석하지 않는다.
- U-21·U-05: 상태 띠는 BrokerConnectionChanged와 SelfCheckChanged 둘 다에 다시 읽는다. 문구는 messages.ts에서 닫힌 SelfCheckAction으로 만든다.
- U-27: 고급 > 상태는 띠에 오르지 않는 대기·버그·6·7번·소스 실패도 반드시 보인다. 기록의 전체 필드와 재개 시각을 사용한다.
- N-01: 소스별 마지막 성공 시각과 상태 저장·소스 상태 변경 이벤트는 정의 대기다. 확정되면 고급 > 상태 갱신에 사용한다. 선택 자격 문제는 이 소스 기록에서만 받는다.
- S-11: 백업 복구의 StoredDataChanged는 자체 점검 이벤트로 대체하지 않는다.

## 공개 API와 크기

- evaluate_self_check(SelfCheckInput, Vec<SourceCheckRecord>) -> SelfCheckResult
- self_check_change(&SelfCheckResult, &SelfCheckResult) -> SelfCheckChange
- toss_check_observation(ErrorClass) -> TossCheckObservation
- isolation_check_observation(IsolationFailure) -> AiCheckObservation

띠는 Option<SelfCheckAction>, 전체 기록은 SelfCheckInput과 SourceCheckRecord 목록이다. 기존 판정·문제 선별·변화 비교 함수 본체는 합계 104줄에서 판정·비교 51줄로 줄었다(함수 선언부터 닫는 괄호까지, 속성 제외). 파일 전체는 201줄에서 206줄이며 칸별 타입과 두 경계 변환을 추가한 결과다. 파일 전체가 절반으로 줄었다고 보고하지 않는다.

## 검사

이 작업이 추가한 테스트만 새 설계로 바꾸고, 다른 기존 테스트는 수정하지 않는다. 기대 조치는 수정된 design 표의 고정 표다. 각 슬롯의 정상·비조치·문제 상태를 표로 열거하고 8×6×6×6×6×13×4×8 = 4,313,088개 조합을 전수 검사한다. 숫자는 대기 시간 등 페이로드의 대표값을 포함한 유한 상태 표의 곱이며 모든 시간 값의 전수 검사라는 뜻은 아니다. 4,096회 속성 검사는 별도 소스 실패를 함께 넣어 전체 기록과 최초 조치를 대조한다.

ErrorClass·UserAction·IsolationFailure의 테스트 소진 match는 새 변형이 생기면 기대 표 검토를 요구한다. 고정 변환 표와 변환 후 미설치·미로그인 띠 결과를 검사한다. 이벤트는 각 슬롯의 모든 표 상태와 복구, 토스 세 칸의 동시 변화, 뉴스만의 변화를 검사한다. 서로 다른 진단·지연·시각·소스 값을 사용한다. 6번에 설치 문제를 대입하는 별도 임시 컴파일 검사도 수행한다.

프로젝트·훅 신뢰 적용 자체를 인증하는 도구는 없으므로 정상 Git 훅의 실제 실행을 전체 검사 증거로 사용한다. 자동 변이 검사는 실행하지 않는다. 손 변형은 새 설계로 다시 실행한 아래 결과만 이번 증거로 사용한다.

## 손 변형 결과

최종 설계의 별도 mktemp 복사본에서 36개를 각각 컴파일한 뒤 실제 테스트로 모두 검출했다. 생존 0, 컴파일 불가 0이다. 변형 전과 원복 후 원본 테스트도 통과했다. 실험 복사본에만 cap-lints를 적용했으며 제품 검사 설정은 변경하지 않았다. 설치 문제를 6번에 대입하는 별도 컴파일 실험은 E0308로 거부됐다.

| 번호 | 바꾼 것 | 컴파일 | 결과 | 잡은 테스트 예시 |
|---|---|---|---|---|
| 1 | 키 조치 교체 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 2 | 키 조치 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 3 | IP 조치 교체 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 4 | IP 조치 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 5 | 중복키 조치 교체 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 6 | 중복키 조치 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 7 | 설치 조치 교체 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 8 | 설치 조치 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 9 | 로그인 조치 교체 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 10 | 로그인 조치 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 11 | 주문 조치 교체 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 12 | 주문 조치 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 13 | claude_install 이벤트 비교 누락 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 14 | claude_login 이벤트 비교 누락 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 15 | ai_environment 이벤트 비교 누락 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 16 | ai_usage 이벤트 비교 누락 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 17 | order_prerequisite 이벤트 비교 누락 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 18 | 이벤트 비교 반전 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 19 | 토스만 변경돼도 발행 | 성공 | 검출 | `tests::event_compares_exactly_slots_four_through_eight_even_with_broker_changes` |
| 20 | 뉴스 기록 누락 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 21 | 한도 기록 유실 | 성공 | 검출 | `tests::design_table_and_all_slot_states_are_preserved` |
| 22 | A05 InvalidKey 조치 변경 | 성공 | 검출 | `tests::a05_conversion_matches_fixed_table` |
| 23 | A05 Permissions 조치 변경 | 성공 | 검출 | `tests::a05_conversion_matches_fixed_table` |
| 24 | A05 AccountNotFound 조치 변경 | 성공 | 검출 | `tests::a05_conversion_matches_fixed_table` |
| 25 | A05 IP를 중복키로 변경 | 성공 | 검출 | `tests::a05_conversion_matches_fixed_table` |
| 26 | A05 대기값 유실 | 성공 | 검출 | `tests::a05_conversion_matches_fixed_table` |
| 27 | AI NotInstalled 칸 변경 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 28 | AI LoginRequired 칸 변경 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 29 | AI 환경 Tools 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 30 | AI 환경 McpServers 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 31 | AI 환경 Plugins 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 32 | AI 환경 Skills 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 33 | AI 환경 SlashCommands 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 34 | AI 환경 PermissionMode 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 35 | AI 환경 ApiKeySource 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
| 36 | AI 환경 CliVersion 사유 교체 | 성공 | 검출 | `tests::isolation_conversion_routes_install_login_and_environment` |
