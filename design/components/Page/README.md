# Page

화면 하나의 틀이에요. 머리줄(제목·동작 버튼), 문제가 있을 때만 보이는 상태 띠, 스크롤되는 본문으로 이뤄져요.

- 제공: `title`, 선택 `leading`(뒤로 가기 같은 앞쪽 버튼), `actions`(IconButton·Button 묶음), `status`(StatusBar), `children`.
- 머리줄 높이 `toolbar-height`. 본문 폭은 최대 `content-max`, 안쪽 여백 `space-6`, 블록 사이 `space-6`.
- 화면은 하나뿐이라 탭이나 내비게이션을 넣지 않아요. 설정·성적표는 Dialog로, 종목 상세는 같은 Page 자리에 `leading` 뒤로 가기와 함께 보여줘요.
