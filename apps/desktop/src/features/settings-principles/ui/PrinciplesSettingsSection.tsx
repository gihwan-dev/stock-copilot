import type { ReactElement } from "react";
import { Card, Section, Skeleton, Text } from "../../../shared/ds";
import { principlesText } from "../messages";
import { usePrinciples, usePrinciplesSaver } from "../model";
import { usePrinciplesClient } from "./client";
import { PrinciplesForm } from "./PrinciplesForm";

const Body = (): ReactElement => {
  const client = usePrinciplesClient();
  const view = usePrinciples(client);
  const saver = usePrinciplesSaver(client);
  switch (view.status) {
    case "loading":
      return (
        <Card title={principlesText("principles.loading", "ko")}>
          <Skeleton variant="text" />
        </Card>
      );
    case "error":
      return (
        <Text tone="alert">
          {principlesText("principles.loadFailed", "ko")}
        </Text>
      );
    case "ready":
      return (
        <PrinciplesForm
          principles={view.data}
          state={saver.state}
          save={saver.save}
        />
      );
  }
};

export const PrinciplesSettingsSection = (): ReactElement => (
  <Section title={principlesText("principles.title", "ko")}>
    <Body />
  </Section>
);
