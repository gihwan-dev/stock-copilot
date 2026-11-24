import type { ReactElement } from "react";
import { Disclosure, Stack } from "../../../shared/ds";
import type { ScreenSources } from "../../../shared/ipc";
import { riskText } from "../messages";
import { OrderSettingsEditor } from "./OrderSettingsEditor";
import { ProfileEditor } from "./ProfileEditor";

export const RiskEditors = (props: {
  readonly sources: Pick<
    ScreenSources,
    | "investorProfile"
    | "saveInvestorProfile"
    | "orderSettings"
    | "saveOrderSettings"
  >;
}): ReactElement => (
  <Disclosure title={riskText("risk.editTitle", "ko")}>
    <Stack gap="space-6">
      <ProfileEditor sources={props.sources} />
      <OrderSettingsEditor sources={props.sources} />
    </Stack>
  </Disclosure>
);
