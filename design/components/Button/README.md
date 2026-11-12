# Button

화면의 동작을 실행하는 버튼이에요. `primary`는 화면당 하나, 나머지는 `secondary`, 툴바·닫기는 `ghost`를 써요.

- 제공: `children`(동사로 끝나는 짧은 문구), `onClick`, 필요하면 `disabled`.
- `size="sm"`은 카드 머리줄과 Callout 안에서만(`control-height-sm`, 좌우 `control-pad-sm`).
- hover는 variant마다 있어요(`primary-hover`, `up-hover`, `down-hover`, secondary·ghost는 `surface-sunken`). `disabled`면 `opacity-disabled`이고 hover에 반응하지 않아요.
- `buy`(빨강 `up` + `on-up`)와 `sell`(파랑 `down` + `on-down`)은 **SignalCard 안에서만** 써요. 문구는 “매수 주문 넣기”, “매도 주문 넣기”처럼 단어를 꼭 넣어요. 누르면 앱 밖의 macOS 확인 창(Touch ID)이 뜨고, 앱 안에 따로 최종 확인 화면은 없어요.
- 그 밖의 곳에서 빨강·파랑 버튼은 쓰지 않아요(가격 색과 헷갈려요). 주문 정정·취소 버튼도 만들지 않아요(토스증권 앱에서 해요).
