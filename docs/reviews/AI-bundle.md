# ai 묶음 리뷰 기록

묶음 규칙(docs/agents/bundle-rules.md)에 따라 2행마다 리뷰 전용 하위 에이전트(Claude Opus)가 본 결과 중 차단·중요만 고치고 남긴다.

| 범위 | 심각도 | 지적 | 처리 |
|---|---|---|---|
| AI-17·AI-18 (`d54d5042..ceec0a6b`) | 중요 | `SandboxProfile`이 금지 경로를 정규화하지 않아 `/tmp`·심볼릭 링크·`..` 경로면 `subpath` 규칙이 듣지 않는데, 자가 점검은 정규화된 임시 폴더로만 시험해 "적용"을 돌려준다 | 프로필을 만들 때 실제 경로로 바꾸고(없는 끝 부분은 가장 가까운 있는 조상을 정규화한 뒤 붙임) `..`가 든 경로는 `NotNormalized`로 거부. 점검 자식은 5초 뒤 강제 종료. 테스트 `profile_uses_real_paths_and_rejects_dot_segments` |
| AI-17·AI-18 | 사소(남김) | 시작 시 점검을 부르는 조립 지점이 아직 없음, 실제 `sandbox-exec`로 `Applied`를 보는 테스트 없음(테스트 자체가 바깥 `sandbox-exec` 안에서 돌아 중첩 불가), `/usage` 결과 줄이 빠지는 CLI 변화 때 stdin 유지로 멈출 수 있음, 기록 폴더 정리가 첫 거부에서 멈춤, 금지 경로와 실행 경로 겹침 미검사 | 조립 루트·AI-12 실측 때 다시 본다 |
| AI-08·AI-14·AI-16 (`ceec0a6b..f4dcf74a`) | 중요 | 실제 어댑터의 `verify_isolation`은 로그인 실패를 `Rejected(LoginRequired)`로 내지 않아, 점검 호출 오류가 모두 6번 `CliVersion`으로 뭉개져 5번(로그인) 띠가 안 뜰 수 있다 | 점검을 못 돌린 경우를 `CliVersionGateOutcome::ProbeFailed`로 나누고 `login_recheck() == Needed`로 조립 루트가 AI-13 로그인 점검을 다시 돌려 5번을 정하게 했다. 테스트 `login_failure_and_probe_error_disable_ai` |
| AI-08·AI-14·AI-16 | 사소(고침) | 오류 결과 줄(`is_error`)의 `structured_output`도 수집 결과로 받음 | 디코더가 성공 결과에서만 구조화 출력을 남김. 테스트 `error_result_structured_output_is_not_accepted` |
| AI-08·AI-14·AI-16 | 사소(남김) | 환경 실패 때 앞선 설치·로그인 문제를 지우지 않음(4·5번은 AI-13 점검이 정함), 게이트·수집 실행기를 부르는 조립 지점이 아직 없음(조립 때 `AiGate::Disabled`면 모든 실행을 막는 테스트 필요) | 조립 작업으로 넘김 |
