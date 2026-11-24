import type { ReactElement } from "react";
import { Button, Card, Stack, Text } from "../../../shared/ds";
import { aiText } from "../messages";
import { usePrivacyLink } from "../model";

export const PrivacyCard = (): ReactElement => {
  const link = usePrivacyLink();
  return (
    <Card title={aiText("ai.privacyTitle", "ko")}>
      <Stack gap="space-3">
        <Text>{aiText("ai.privacyBody", "ko")}</Text>
        <Button variant="secondary" onClick={link.open}>
          {aiText("ai.privacyOpen", "ko")}
        </Button>
        {link.state === "failed"
          ? [
              <Text key="failed" tone="alert">
                {aiText("ai.privacyFailed", "ko")}
              </Text>,
            ]
          : []}
      </Stack>
    </Card>
  );
};
