import {
  aiEfforts,
  aiModels,
  budgetPercents,
  type AiEffort,
  type AiModel,
  type BudgetPercent,
} from "../../../shared/ipc";
import {
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { aiText } from "../messages";

export type Choice = { readonly value: InputValue; readonly label: UiText };

type Literal = AiModel | AiEffort | BudgetPercent;

const choice = (raw: Literal, label: UiText): readonly Choice[] => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? [{ value: value.value, label }] : [];
};

export const modelChoices: readonly Choice[] = aiModels.flatMap((model) =>
  choice(model, aiText(`ai.model.${model}`, "ko")),
);

export const effortChoices: readonly Choice[] = aiEfforts.flatMap((effort) =>
  choice(effort, aiText(`ai.effort.${effort}`, "ko")),
);

export const budgetChoices: readonly Choice[] = budgetPercents.flatMap(
  (percent) => choice(percent, aiText(`ai.budget.${percent}`, "ko")),
);

export const pickModel = (value: InputValue): AiModel | undefined =>
  aiModels.find((model) => model === value);

export const pickEffort = (value: InputValue): AiEffort | undefined =>
  aiEfforts.find((effort) => effort === value);

export const pickBudget = (value: InputValue): BudgetPercent | undefined =>
  budgetPercents.find((percent) => percent === value);

export const inputOf = (raw: Literal): InputValue | undefined => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? value.value : undefined;
};
