import type { ReactElement } from "react";
import { Stack } from "../../../shared/ds";
import { OrderCard } from "./OrderCard";
import { ProfileCard } from "./ProfileCard";
import { TossCard } from "./TossCard";

export const BasicSettingsSection = (): ReactElement => (
  <Stack gap="space-4">
    <TossCard />
    <ProfileCard />
    <OrderCard />
  </Stack>
);
