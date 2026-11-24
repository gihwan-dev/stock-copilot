# 차트·추정 코어 묶음 결정 표

문서가 정하지 않았거나 잠금 파일·선행 작업 부재로 선택이 필요했던 항목을 ADR 0173에 따라 정리한다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| DS-17 | `lightweight-charts`가 잠금 파일(`pnpm-lock.yaml`)과 TS 의존 허용 목록(`harness/ts-deps/allowlist.json`)에 없어 외부 패키지를 추가하지 않고 HTML5 Canvas 2D로 `CandleChart`를 직접 구현했다. `shared/platform/canvasTheme.ts`가 CSS 토큰(`--color-*`)을 읽어 `CandleChart`에 넘기고 테마·크기 변경 시 다시 그리며, U-05가 없으므로 `CandleChart`는 자료·테마·선택 팔레트를 props로 받는 순수 선언형 컴포넌트로 구현했다. | `lightweight-charts` 패키지 신규 설치, SVG 전용 캔들 차트 |
