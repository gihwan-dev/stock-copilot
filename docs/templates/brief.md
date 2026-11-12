# 브리프 <작업 ID> <제목>
- 기준 커밋: <SHA>
- 완료 기준 (tasks.md 그대로. 각 기준은 통과·실패로 판정할 수 있어야 한다)
  1. ...
- 관련 요구사항: SEC-.., ORD-..   관련 구조 규칙: architecture.md x.y
- 손댈 범위: crates/<name>/src/**, crates/<name>/tests/**
- 손대면 안 되는 범위: (보호 경로 외에 추가로)
- 정해진 결정: ADR-NNNN
- 검사 명령: just check
- 막혔을 때: 보고서 첫 줄을 "막힘:"으로 시작하고 멈춘다
