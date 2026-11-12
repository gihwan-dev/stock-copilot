import { moneyKey, type Money } from "./money";
export const formatMoney = (money: Money): string =>
  new Intl.NumberFormat("ko-KR").format(money[moneyKey]);
