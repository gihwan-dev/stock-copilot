# S-13·S-17 실측 기록: 서명 준비와 서명 빌드의 키체인 (U-02, S-02)

- 작업: [tasks.md](../tasks.md) S-13(서명 준비), S-17(실측 S-02). 관련 결정 [ADR 0176](../adr/0176-signing-and-database-key-loss.md)
- 기록일: 2026-11-25 (fin-measure 묶음)

## S-13: 서명 준비 (U-02)

| 항목 | 결과 |
|---|---|
| Apple 개발자 멤버십 | **있음**. Developer ID 팀 `SLVS4WF9U2` |
| 설치 앱 서명 | `/Applications/Stock Copilot.app` v0.2.0: `Developer ID Application (SLVS4WF9U2)` → `Developer ID Certification Authority` → `Apple Root CA`, 보안 타임스탬프 있음, Hardened Runtime 켜짐 |
| 식별자 | `dev.stockcopilot.desktop` |
| 엔타이틀먼트 | 비어 있음. `keychain-access-groups`가 없으므로 데이터 보호 키체인이 아니라 **로그인 키체인**을 쓴다 (ADR 0176 3절의 낮춘 방식) |
| 공증 | 안 됨. `spctl -a`가 `rejected (Unnotarized Developer ID)`. 이 PC에서 직접 설치해 쓰는 데는 지장이 없지만 다른 Mac에 내려받아 열면 Gatekeeper가 막는다 |
| 프로비저닝 프로파일 | 없음(데이터 보호 키체인을 쓰려면 필요) |

## S-17: 키체인 왕복 (S-02)

| 확인 | 결과 | 확인한 쪽 |
|---|---|---|
| 첫 실행에서 DB 키 생성·저장 | 동작함 (로그인 키체인) | 메인 세션, 2026-11-25 |
| 재실행에서 같은 키로 DB 열기 | 동작함. 재실행 때 키체인 확인 창 없음 | 메인 세션, 2026-11-25 |
| 반복 재실행 | 이번 묶음의 PERF-1 측정에서 설치 앱을 10회 종료·재실행했고 매번 키체인 확인 창 없이 0.6초 안에 창이 떴다([G-09](G-09-performance.md)). DB 열기 실패도 창은 띄우므로 이것만으로 DB 열림을 증명하지는 않는다. DB 열림은 메인 세션 확인에 기댄다 | fin-measure 묶음 |
| 위험 설정 변경 확인 창(네이티브 요약 창) | **사람 몫**. 창의 표시와 버튼 동작을 사람이 보고 눌러야 함 | 사용자 |
| macOS 본인 확인 창(Touch ID, `LAContext`) | **사람 몫**. 지문·암호 입력이 필요해 자동화할 수 없음 | 사용자 |
| 본인 확인 이유 문자열 길이 한도 | **사람 몫**. 긴 이유 문자열로 창을 띄워 잘림 위치를 눈으로 확인 | 사용자 |

## 남은 것

- 사용자: 설치 앱에서 위험 설정 하나를 바꿔 요약 창을 확인하고, 본인 확인 창(Touch ID)과 이유 문자열 잘림을 확인해 위 표의 "사람 몫" 칸을 채운다.
- 데이터 보호 키체인으로 올리려면 ADR 0176 2절의 프로비저닝 프로파일과 `keychain-access-groups`가 필요하다. 지금은 로그인 키체인으로 낮춘 상태가 사용자 결정과 맞는지 메인이 확인한다.
- 다른 기기에 배포하려면 공증이 필요하다.
