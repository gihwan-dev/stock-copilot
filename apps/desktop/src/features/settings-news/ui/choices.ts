import {
  dailyMovePercents,
  intervalMinutes,
  newsPerHourCounts,
  type DailyMovePercent,
  type IntervalMinutes,
  type NewsPerHour,
} from "../../../shared/ipc";
import {
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { newsText } from "../messages";

export type Choice = { readonly value: InputValue; readonly label: UiText };

type Literal = IntervalMinutes | DailyMovePercent | NewsPerHour;

const choice = (raw: Literal, label: UiText): readonly Choice[] => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? [{ value: value.value, label }] : [];
};

export const intervalChoices: readonly Choice[] = intervalMinutes.flatMap(
  (minutes) => choice(minutes, newsText(`news.minutes.${minutes}`, "ko")),
);

export const moveChoices: readonly Choice[] = dailyMovePercents.flatMap(
  (percent) => choice(percent, newsText(`news.move.${percent}`, "ko")),
);

export const countChoices: readonly Choice[] = newsPerHourCounts.flatMap(
  (count) => choice(count, newsText(`news.count.${count}`, "ko")),
);

export const pickInterval = (value: InputValue): IntervalMinutes | undefined =>
  intervalMinutes.find((minutes) => minutes === value);

export const pickMove = (value: InputValue): DailyMovePercent | undefined =>
  dailyMovePercents.find((percent) => percent === value);

export const pickCount = (value: InputValue): NewsPerHour | undefined =>
  newsPerHourCounts.find((count) => count === value);

export const inputOf = (raw: Literal): InputValue | undefined => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? value.value : undefined;
};
