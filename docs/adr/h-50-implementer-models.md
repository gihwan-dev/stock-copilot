# H-50 구현 모델 허용

- 상태: 초안

## 사용자 결정

2026-11-17 사용자가 구현에 Terra(max)를 허용하고 리뷰 구성을 유지하기로 했다. 이 작업 구현은 실제 Codex gpt-6.1-sol high다.

## 결정

구현·수정 모델은 메인이 시작할 때 정확히 `gpt-6.1-sol` 또는 `gpt-6.1-terra`에서 선택한다. 기본 추론 강도는 각각 high·max다. 접두사와 패턴 일치는 쓰지 않는다. 조사·비판은 sol high, 적대적 리뷰는 Opus 5.5를 유지한다. Terra 구현과 다른 모델의 독립성을 확보한다. sol 구현에서는 새 맥락과 별도 사본으로 독립성을 유지한다.

delegate_task의 기존 role을 사용한다. Terra는 명시적인 implementation만 허용한다. 역할 생략 경로는 sol만 허용하여 기존 호출을 보존한다. t3_thread_launch에는 역할 인자가 없어 Terra 시작에만 브리프의 정확한 `- 역할: implementation` 한 줄을 요구한다. 정확한 `- 역할: implementation` 형식의 선언만 세고 이 선언의 중복은 거부한다. 들여쓰기나 다른 목록 표기와 코드 펜스를 해석하지 않으므로 브리프에는 정해진 형식의 역할 줄을 본문에 한 번 쓴다. 조사·비판은 역할이 있는 delegate_task를 사용한다. 권한·제공자·격리·기준 검사는 유지한다.

Claude 위임 훅이 orchestrationFindings를 사용한다. Codex 훅은 이를 호출하지 않고 adapter의 허용 목록 밖 T3 도구를 기본 거부한다. Codex 회귀 실행기는 실험 모델 sol을 고정할 뿐 위임 모델 허용 검사를 하지 않아 변경하지 않는다. 리뷰 기록 검사도 모델 문자열을 확인하지 않아 변경하지 않는다. 기록에는 실제 구현 모델을 적는다.

작업 절의 구체적 문서 승인을 우선하여 tasks에 H-50 행을 추가하고 progress 운영 메모의 모델 문구만 갱신한다. 과거 리뷰·확정 ADR·리서치는 바꾸지 않는다. 기존 테스트 수정은 없으며 새 파일에 회귀를 추가한다. H-49의 변이 선택·실행 코드는 수정하지 않는다.

T3 capabilities가 Codex 옵션 이름을 reasoningEffort로 반환하므로 이를 명시적으로 허용한다. 기존 effort 키는 같은 강도로 유지하고 두 키의 혼합과 다른 키는 거부한다. Terra는 max를 명시해야 하며 생략·빈 객체·빈 배열은 거부한다. sol의 기존 생략·빈 객체·빈 배열 경로는 보존한다. 이때 high는 가드가 보장하지 않으며 실제 강도는 T3의 상속·기본값에 따른다. 운영에서는 high를 명시하고 실제 모델·강도를 기록한다.

2026-11-17 사용자 결정에 따라 이 ADR과 workflow의 H-50 규칙이 ADR 0021의 구현 모델 sol 한정 및 옵션 effort=high 한정을 대체한다. 대체 범위는 구현·수정의 sol/terra 닫힌 집합, 각 강도와 옵션 키뿐이다. 조사·비판 sol, 적대적 리뷰 Opus 및 제공자·격리·기준·소유권 규칙은 그대로 유지한다. 과거 ADR 0021 본문은 수정하지 않는다.

## 검증

새 모델·역할·강도·경계 회귀와 기존 Codex·Claude 훅 회귀를 실행한다. 프로젝트 신뢰와 세션 훅 신뢰는 현재 런타임에서 확인할 수 없다. 홈 설정이나 인증 파일은 읽지 않았다. Git 훅 설치는 첫 시도에 공용 파일 교체 오류가 났으나 재시도는 통과했다.

## 인계

- 메인: 최종 SHA의 병합과 전체 검사, ADR 확정.
- H-49: 변이 실행기 변경 없음. 공통 생성물 해시는 병합 후 스크립트로 재생성.

## 손 변형

실행 결과를 아래에 기록한다.
| 번호 | 바꾼 것 | 검출 테스트 |
|---|---|---|
| 1 | 모든 모델 허용 | ✖ delegate exact model gpt-6.1-terra role research |
| 2 | sol 접두사 허용 | ✖ delegate exact model gpt-6.1-sol-extra role implementation |
| 3 | sol 삭제 | ✖ delegate exact model gpt-6.1-sol role implementation |
| 4 | terra 삭제 | ✖ delegate exact model gpt-6.1-terra role implementation |
| 5 | 구현 역할 검사 제거 | ✖ delegate exact model gpt-6.1-terra role research |
| 6 | research를 구현으로 허용 | ✖ delegate exact model gpt-6.1-terra role research |
| 7 | review를 구현으로 허용 | ✖ delegate exact model gpt-6.1-terra role review |
| 8 | 역할 생략 terra 허용 | ✖ delegate exact model gpt-6.1-terra role undefined |
| 9 | terra 접두사 허용 | ✖ delegate exact model gpt-6.1-terra-extra role implementation |
| 10 | luna 구현 허용 | ✖ delegate exact model gpt-6-luna role implementation |
| 11 | 강도 검사 제거 | ✖ Opus review accepts effort high and rejects Codex reasoningEffort |
| 12 | Codex 강도 진단 제거 | ✖ model role and effort failures explain the rejected contract |
| 13 | terra high로 뒤집기 | ✖ delegate exact model gpt-6.1-terra role implementation |
| 14 | sol max로 뒤집기 | ✖ delegate exact model gpt-6.1-sol role implementation |
| 15 | 중복 역할 허용 | ✖ terra launch requires one exact implementation role line |
| 16 | 역할 선택 뒤집기 | ✖ launch exact model gpt-6.1-terra role implementation |
| 17 | 시작 역할 상수 구현 | ✖ model role and effort failures explain the rejected contract |
| 18 | 역할 제한을 research로 교체 | ✖ delegate exact model gpt-6.1-terra role implementation |
| 19 | 모델 역할 진단 제거 | ✖ model role and effort failures explain the rejected contract |
| 20 | Terra 명시 max 검사 제거 | ✖ terra requires an explicit max option while sol retains omission |
| 21 | 위임 제공자 검사 제거 | ✖ model boundaries gpt-6.1-sol |
| 22 | 격리 검사 제거 | ✖ model boundaries gpt-6.1-sol |
| 23 | providerInstanceId 검사 제거 | ✖ model boundaries gpt-6.1-sol |
| 24 | options 키 검사 제거 | ✖ model boundaries gpt-6.1-sol |
| 25 | Opus reasoningEffort 방어 제거 | ✖ Opus review accepts effort high and rejects Codex reasoningEffort |
| 26 | Terra effort max 경로 제거 | ✖ delegate exact model gpt-6.1-terra role implementation |
| 27 | Terra reasoningEffort max 경로 제거 | ✖ T3 reasoningEffort option gpt-6.1-terra |
| 28 | 기대값이 같은 혼합 키 허용 | ✖ mixed option keys are denied even when both values match |
| 29 | 역할 줄 시작 앵커 제거 | ✖ Terra launch role marker must start its own line |
