import { ok, type Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type {
  OrderSettingsDraft,
  ProfileDraft,
  RiskSaveOutcome,
} from "./risk-editor";
import {
  orderRequest,
  profileRequest,
  settleRiskSave,
  settleRiskSettings,
  type RiskSettingsView,
} from "./risk-schema";
import { wireOutcome, type IpcFailure } from "./schema";

export const getRiskSettings = (): Promise<
  Result<RiskSettingsView, IpcFailure>
> =>
  settleCall(commands.getRiskSettings({ scope: "current" }), (outcome) =>
    settleRiskSettings(wireOutcome(outcome)),
  );

export const investorProfile = (): Promise<Result<ProfileDraft, IpcFailure>> =>
  getRiskSettings().then((result) =>
    result.kind === "ok" ? ok(result.value.profile) : result,
  );

export const orderSettings = (): Promise<
  Result<OrderSettingsDraft, IpcFailure>
> =>
  getRiskSettings().then((result) =>
    result.kind === "ok" ? ok(result.value.order) : result,
  );

export const saveInvestorProfile = (
  draft: ProfileDraft,
): Promise<Result<RiskSaveOutcome, IpcFailure>> =>
  settleCall(
    commands.saveInvestorProfile({ draft: profileRequest(draft) }),
    (outcome) => settleRiskSave(wireOutcome(outcome)),
  );

export const saveOrderSettings = (
  draft: OrderSettingsDraft,
): Promise<Result<RiskSaveOutcome, IpcFailure>> =>
  settleCall(
    commands.saveOrderSettings({ draft: orderRequest(draft) }),
    (outcome) => settleRiskSave(wireOutcome(outcome)),
  );
