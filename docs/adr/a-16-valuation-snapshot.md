# A-16 일별 평가 기록

- 상태: 초안

## 결정

기존 PortfolioRepository.latest에서 계좌의 Complete/Partial 포트폴리오를 읽고 Clock.now를 평가·기록 시각으로 사용한다. 호출자가 제공한 SnapshotCalendar와 같은 시각의 Mid 조회 결과를 domain Portfolio.evaluate와 ValuationSnapshot.observe에 전달한다. 유스케이스에서 날짜·금액 계산을 하지 않는다. Failed/AccessBlocked/기록 없음은 오류로 반환하며 오래된 성공 기록으로 대체하지 않는다. 일별 실행 시점과 달력 준비는 J-11의 책임이다. 조회 자료의 신선도 임계값은 정의하지 않는다.

ValuationRepository.append 성공 후 ValuationSnapshotTaken(account)를 한 번 낸다. AlreadyRecorded도 성공이므로 보고서에 그대로 남기고 한 번 발행한다. 저장 실패는 발행하지 않고 원 오류를 돌려준다. 구독 실패는 DeliveryReport로 보존하며 저장을 되돌리거나 재발행하지 않는다. 영속 발행 보장이나 일별 중복 정책은 추가하지 않는다. 새 포트·domain 타입·의존성·가짜 메서드는 없다.

## 기준별 충족과 정의 대기

| 기준 | 상태 | 근거와 필요한 계약 |
|---|---|---|
| 일별 평가 기록 생성·저장 | 충족 | 명시적 평가 달력, Clock 시각, 기존 domain 함수, 기존 append |
| 저장 뒤 계좌 이벤트 한 번 | 충족 | 저장 오류·구독 실패·중복 저장을 포함한 통합 테스트 |
| 빠진 날짜 목록 계산 | 정의 대기 | domain에 기록 이력·시장별 달력에서 빠진 평가일을 산출하는 함수가 없음. Timestamp 연산으로 application에서 만들지 않음 |
| 빠진 날 일봉·거래내역 재계산 | 정의 대기 | recalculate는 있으나 날짜 계획과 종목 이력·기업 행사·대조 일치 현금 원장의 공급 계약이 없음 |
| 자료 부족일을 채우지 못함으로 영속 기록 | 정의 대기 | ValuationRepository.append는 ValuationSnapshot만 받음. SnapshotCalendar 생성 실패 등 스냅샷 자체가 없는 날짜·실패 사유를 저장할 계약이 없음 |
| 가짜 테스트 위치 | 충족 | testkit-ports/tests/valuation_snapshot.rs; application/domain 개발 의존 추가 없음 |

정의 대기 부분을 건너뛰는 자동 복구 API를 제공하지 않는다. 이 변경은 A-16 전체 완료를 뜻하지 않는다.

## 인계

| 받는 작업 | 남은 일 |
|---|---|
| D-31 | 누락 평가일 계획 함수와 자료 부족일 결과 타입·재시도 기준 확정 |
| A-16 | 위 계약 이후 시작 시 누락 계획·일봉·거래내역 재계산 연결 |
| S-18 | 계좌·평가일별 중복 및 원본/재계산/채우지 못함 저장 계약 확정 |
| J-11 | 16:00 실행과 시작 시 복구, 시장별 달력 공급 연결 |

## 검증 경계

실제 키·계좌·API 없음. 프로젝트 신뢰와 훅 신뢰·세션 적용은 확인 불가이며 Git lefthook 설치만 확인했다. 기존 테스트와 보호 규칙 약화 없음. 새 ADR로 발생하는 설정·해시 생성물만 공식 생성 명령으로 갱신한다.
