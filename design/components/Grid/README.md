# Grid · GridItem

12열 격자예요(`grid-columns`, 열 간격 `space-6`). 구간은 창이 아니라 Grid 자신의 폭으로 정해요(컨테이너 기준).

- 구간: narrow 640–899px, medium 900–1199px(`bp-content-medium`), wide 1200px 이상(`bp-content-wide`).
- `GridItem`의 `span`은 숫자 하나 또는 `{narrow, medium, wide}`. 비어 있는 구간은 더 좁은 구간 값을 따르고, narrow 기본은 12예요.
- 열 수는 **3, 4, 6, 8, 9, 12만** 허용해요. 다른 값은 12로 그리고 오류로 올려요.
- 미디어·컨테이너 쿼리엔 `var()`를 못 써서 Grid가 토큰 값을 읽어 JS로 구간을 정해요.
