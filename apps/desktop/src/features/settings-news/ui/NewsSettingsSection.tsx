import type { ReactElement } from "react";
import { Card, Section, Skeleton, Text } from "../../../shared/ds";
import type { UiText } from "../../../shared/lib";
import { newsText } from "../messages";
import { useNewsSaver, useNewsSettings, type SaveState } from "../model";
import { AliasCard } from "./AliasCard";
import { useNewsClient } from "./client";
import { ContactCard, WakeCard } from "./ContactWakeCards";
import { KeysCard } from "./KeysCard";
import { SourcesCard } from "./SourcesCard";

const saveText = (state: SaveState): UiText | undefined => {
  switch (state) {
    case "idle":
    case "pending":
      return undefined;
    case "saved":
      return newsText("news.saved", "ko");
    case "failed":
      return newsText("news.saveFailed", "ko");
  }
};

const Notice = (props: { readonly state: SaveState }): ReactElement | null => {
  const sentence = saveText(props.state);
  if (sentence === undefined) return null;
  return (
    <Text tone={props.state === "failed" ? "alert" : "ink-muted"}>
      {sentence}
    </Text>
  );
};

export const NewsSettingsSection = (): ReactElement => {
  const client = useNewsClient();
  const view = useNewsSettings(client);
  const saver = useNewsSaver(client);
  switch (view.status) {
    case "loading":
      return (
        <Section title={newsText("news.title", "ko")}>
          <Card title={newsText("news.loading", "ko")}>
            <Skeleton variant="text" />
          </Card>
        </Section>
      );
    case "error":
      return (
        <Section title={newsText("news.title", "ko")}>
          <Text tone="alert">{newsText("news.loadFailed", "ko")}</Text>
        </Section>
      );
    case "ready":
      return (
        <Section title={newsText("news.title", "ko")}>
          <Notice state={saver.state} />
          <SourcesCard sources={view.data.sources} save={saver.save} />
          <KeysCard keys={view.data.keys} routes={view.data.routes} />
          <ContactCard
            contact={view.data.sec_contact}
            route={view.data.routes?.sec}
            save={saver.save}
          />
          <WakeCard wake={view.data.wake} save={saver.save} />
          <AliasCard aliases={view.data.aliases} save={saver.save} />
        </Section>
      );
  }
};
