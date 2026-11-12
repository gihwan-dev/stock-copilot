import type { z } from "zod";
import {
  moneyKey,
  priceKey,
  type Currency,
  type Money,
  type Price,
} from "./money";

export type DisplayText = string & Readonly<z.core.$brand<"DisplayText">>;

const formatters: Readonly<Record<Currency, Intl.NumberFormat>> = {
  KRW: new Intl.NumberFormat("ko-KR", { style: "currency", currency: "KRW" }),
  USD: new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }),
};

const display = (formatted: string): DisplayText =>
  formatted.replaceAll("-", "−") as DisplayText;

export const formatMoney = (money: Money): DisplayText =>
  display(formatters[money.currency].format(money[moneyKey]));

export const formatPrice = (price: Price): DisplayText =>
  display(formatters[price.currency].format(price[priceKey]));
