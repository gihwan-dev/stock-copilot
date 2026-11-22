# S-06 논거 노트 저장소
- 상태: 초안

## 결정

종목 ID를 노트 키로 두고 이력 각 행을 순서 번호와 버전 없는 필드 길이 접두 형식으로 보존한다. 출처, 작업, 모델 사유와 사용자 결정을 원문대로 기록한다. 전체 상태나 확인 필요 여부는 저장하지 않고 이력을 도메인 연산으로 재생한다. 필드의 도메인 생성, 이력 순서, 정규 표현과 재생 결과가 어긋나면 읽기는 실패한다. 노트 저장은 기존 이력이 요청 이력의 접두인지 비교하고 새로운 행만 한 트랜잭션으로 추가한다. 이력 수정·삭제와 기존 순번의 교체 삽입은 트리거가 거부한다.

`ThesisRepository`의 기존 `store`, `load`를 구현한다. 신규 포트는 없다. 빈 노트와 없는 노트를 구분한다. 같은 종목에 이전 이력을 다르게 덮어쓰려는 저장은 거부한다.

## 인계

- J-02: 논거 MCP 도구에 저장소를 연결하고 저장 실패를 처리한다.
- J-08: 노트와 이력 조회를 화면에 연결한다.

## 검증

도메인 생성과 이력 재생, 확인 필요·사용자 확인·판정 집계, 서로 다른 종목, 손상 입력, 수정·삭제 및 `INSERT OR REPLACE`·`UPSERT` 거부, 중간 실패의 트랜잭션 롤백과 노트 없이 이력만 남은 손상을 테스트한다. 교체 삽입 시험은 격리 복사본의 기존 구현에서 실패하고 트리거 추가 후 통과했다.

## 손 변형

별도 mktemp 복사본과 빌드 디렉터리에서 각 변형을 따로 컴파일·실행했다. 교체 방지 수정 전 기준 테스트 10개와 수정 후 11개가 통과했다. 아래 23개는 모두 테스트 실패로 검출됐다. 변형마다 첫 번째로 출력된 실패 테스트를 적는다.

| 번호 | 변형 | 잡은 테스트 |
|---|---|---|
| 1 | 이력 순서 비교 반전 | `thesis_tests::sqlite_rejects_update_and_delete_of_past_history` |
| 2 | 이력 순서 증가 폭 변경 | `thesis_tests::thesis_status_is_derived_from_replayed_history` |
| 3 | 기존 접두 일치 판정 반전 | `thesis_tests::thesis_store_rejects_shortened_or_divergent_history` |
| 4 | 추가 시 한 이력 건너뜀 | `thesis_tests::thesis_history_roundtrip_replays_pending_confirmation_and_isolates_instruments` |
| 5 | 사용자 확인을 거절로 재생 | `thesis_tests::thesis_status_is_derived_from_replayed_history` |
| 6 | 주장을 다른 역할로 기록 | `thesis_tests::sqlite_rejects_update_and_delete_of_past_history` |
| 7 | 주장 문장 대신 지표 저장 | `thesis_tests::thesis_store_rejects_shortened_or_divergent_history` |
| 8 | 모델 대신 변경 사유 저장 | `thesis_tests::thesis_history_roundtrip_replays_pending_confirmation_and_isolates_instruments` |
| 9 | 프롬프트 버전 대신 모델 저장 | `thesis_tests::thesis_status_is_derived_from_replayed_history` |
| 10 | 웹 출처를 도구 출처로 기록 | `thesis_tests::thesis_history_roundtrip_replays_pending_confirmation_and_isolates_instruments` |
| 11 | 기계 조건 비교 방향 변경 | `thesis_tests::criterion_and_all_source_variants_roundtrip` |
| 12 | 기계 조건의 기준값 변경 | `thesis_tests::criterion_and_all_source_variants_roundtrip` |
| 13 | 관측 지표에 도구 식별자 저장 | `thesis_tests::criterion_and_all_source_variants_roundtrip` |
| 14 | DART 출처를 KIND로 기록 | `thesis_tests::criterion_and_all_source_variants_roundtrip` |
| 15 | 조건 미충족을 충족으로 기록 | `thesis_tests::criterion_and_all_source_variants_roundtrip` |
| 16 | 깨진 주장을 약화로 기록 | `thesis_tests::thesis_status_is_derived_from_replayed_history` |
| 17 | 확인을 거절로 기록 | `thesis_tests::thesis_store_rejects_shortened_or_divergent_history` |
| 18 | 연합뉴스를 한국경제로 복원 | `thesis_tests::thesis_status_is_derived_from_replayed_history` |
| 19 | 남은 입력 검사 반전 | `thesis_tests::thesis_status_is_derived_from_replayed_history` |
| 20 | 출처 수 상한을 하나로 축소 | `thesis_tests::criterion_and_all_source_variants_roundtrip` |
| 21 | 수정 금지 트리거 제거 | `thesis_tests::sqlite_rejects_update_and_delete_of_past_history` |
| 22 | 고아 이력을 없는 노트로 처리 | `thesis_tests::orphan_history_is_corruption_instead_of_an_absent_note` |
| 23 | 기존 이력의 교체 삽입 금지 트리거 제거 | `thesis_tests::sqlite_rejects_conflict_replacement_of_past_history` |
