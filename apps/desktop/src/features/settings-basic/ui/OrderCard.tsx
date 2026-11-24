import type { ReactElement } from "react";
import { Card, Skeleton, Stack, Switch, Text } from "../../../shared/ds";
import { basicText } from "../messages";
import { useOrderSwitch } from "../model";
import { useBasicClient } from "./client";

const Body = (): ReactElement => {
  const control = useOrderSwitch(useBasicClient());
  switch (control.view.status) {
    case "loading":
      return <Skeleton variant="text" />;
    case "error":
      return <Text tone="alert">{basicText("basic.orderFailed", "ko")}</Text>;
    case "ready": {
      const state = control.view.data.state;
      return (
        <Stack gap="space-2">
          <Switch
            label={basicText("basic.orderLabel", "ko")}
            description={basicText(
              state === "on" ? "basic.orderOn" : "basic.orderOff",
              "ko",
            )}
            checked={state}
            disabled={control.pending === "pending" ? "disabled" : "enabled"}
            onChange={control.request}
          />
          {control.failed === "failed"
            ? [
                <Text key="failed" tone="alert">
                  {basicText("basic.saveFailed", "ko")}
                </Text>,
              ]
            : []}
        </Stack>
      );
    }
  }
};

export const OrderCard = (): ReactElement => (
  <Card title={basicText("basic.orderTitle", "ko")}>
    <Body />
  </Card>
);
