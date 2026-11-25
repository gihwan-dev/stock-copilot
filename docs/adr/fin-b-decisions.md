# fin-b 묶음 설계 결정 기록

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| S-20 | `sqlparser`는 `=0.64.0`(잠금 파일에 없어 최신 안정판), 기본 기능을 끄고 `std`만 켠다. `visitor` 기능은 `sqlparser_derive` 의존을 더하므로 쓰지 않고 낱말 수 비교로 허용 위치를 판정한다. 자세한 규칙과 마이그레이션 잠금 절차는 `s-20-sql-structure.md` | `visitor` 기능으로 AST 전체 방문 |
| S-20 | 마이그레이션 상수는 `migrations/*.sql`과 글자까지 같은지로 가린다. 마이그레이션에 DML(데이터 옮기기)이 있어도 허용한다 | `MIGRATIONS` 배열을 구문 분석해 별칭 해석 |
| H-68 | 변이 대상은 파일 단위 경로로 적고(하위 폴더가 있는 모듈은 `<모듈>/**/*.rs`도 함께), 고정 테스트 `MUTATION_TARGET_GLOBS`를 같은 목록으로 갱신했다. 자세한 것은 `h-68-check-time.md` | 크레이트 전체 유지, 경로 패턴 규칙만 검사 |
| H-67 | 보관·병합·복원 한 묶음 명령(`staged-sync.mjs sync`)과 git 폴더의 막힘 기록(`staged-sync.mjs blocked`)으로 정했다. 자세한 것은 `h-67-staged-sync-and-blocked-exit.md`. 훅 해시 갱신은 사용자 몫으로 남겼다 | 승인 기준을 커밋 시작 시점에 고정 |
| H-68 | (2) 빌드 결과 공유와 전·후 시간 측정은 실측 항목이라 건너뛰었다. 변이 수만 `cargo mutants --list`로 쟀다 | 이 기기에서 씨앗·공유 캐시 재측정 |
