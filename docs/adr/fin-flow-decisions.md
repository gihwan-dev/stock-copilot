# fin-flow 묶음 결정 기록

경량 절차(ADR 0173)로 `fin-flow` 묶음(추정 과제 생성 → AI 추정 → 추정 저장 → 신호 평가 → 제안 초안 → 신호 카드 이벤트를 제품 경로로 잇기)을 구현하며 문서가 정하지 않아 정한 것이다.

## 정한 것

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| FF-1 | 과제 만기(비용 기간 끝)를 판정과 같은 규칙(`schedule`: 기준 시각 뒤 정규장 기준 H 거래일의 정규장 종료, 몇 시간 단위는 첫 정규장 길이의 1/4)으로 domain `estimate_maturity`가 계산한다. 달력이 모자라면 `Calendar` 실패 | 조립 루트에서 날짜 더하기 |
| FF-2 | 단일 추정 실행 `RunSingleEstimate`(application): 재료 포트 → 만기용 달력 → `CreateEstimateTask` → 실행 맥락 등록 → 분석 실행(프롬프트에는 과제 ID와 종목 식별자만) → 맥락 해제 → 과제가 소비됐으면 `Submitted`. 재료·달력이 없거나 과제가 건너뛰어지면 AI를 띄우지 않는다 | 과제를 만들지 않고 AI가 종목을 고름(SEC-4 위반) |
| FF-2 | 실행·과제 ID는 `est-<계기>-<종목 ID 앞 24자>-<UTC 초>`로 같은 문자열을 쓴다(과제 ID 64자 제한). 같은 종목·같은 초의 두 번째 발급은 `AlreadyIssued`로 실패 | 난수 ID(application에서 난수 금지) |
| FF-2 | 추정 프롬프트는 application 상수(`prompts/estimate/v1.md`의 요지)로 두고, 기록용 `EstimatePrompt`는 id `estimate`, 버전 `v1`, 해시는 상수 원문의 SHA-256이다. 배포 앱에 프롬프트 파일이 들어가지 않아 파일을 읽지 않는다 | 실행 때 파일 읽기 |
| FF-2 | 보정 조합의 모델 식별은 `claude-code`/`default`/`cli`로 고정한다. 실행 중 `ActualModel` 이벤트와 MCP 제출이 경쟁해 같은 실행의 모델이 어긋날 수 있어서다. 실제 모델별 분리는 후속 | 실제 모델 이벤트로 갱신 |
