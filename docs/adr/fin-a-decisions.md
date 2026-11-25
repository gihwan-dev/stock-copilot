# fin-a 묶음에서 정한 것

ADR 0173 경량 절차로 fin-a 묶음을 구현하며 문서가 정하지 않은 것을 가장 단순하고 안전한 쪽으로 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| check-lite(DS-17) | Canvas 2D 색·굵기는 속성 대입으로만 지정할 수 있어(`Object.assign`·`Reflect`·`defineProperty`는 이미 금지) 코드만으로 끄기 주석을 없앨 길이 없다. `shared/platform` 허용 목록 파일에 한해 `functional/immutable-data`의 `ignoreAccessorPattern`을 `ctx.fillStyle`·`ctx.strokeStyle`·`ctx.lineWidth` 세 속성으로만 열고, 같은 규칙을 끌 수 없는 규칙 목록(`UNDISABLEABLE_RULES`)에 넣어 TS 끄기 0건을 유지한다 | 끄기 주석 유지와 `disable_control` 기대값 완화, Canvas 대신 SVG로 다시 그리기(기존 DS-17 시험이 Canvas 호출을 고정해 버림) |
| check-lite | `harness/claude-hooks/hashes.json` 불일치(`.codex/config.toml`, `crates/testkit-ports/src/lib.rs`)는 `hashes.mjs --write`가 서명 커밋 전용(ADR 0018)이라 훅이 막아 다시 만들지 않았다. 사용자 서명 커밋으로 갱신해야 한다 | - |
