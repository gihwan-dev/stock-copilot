import type { ReactElement } from "react";
import { text, type UiText } from "../lib";
import { AppFrame } from "./AppFrame";
import { AppShell } from "./AppShell";
import { Badge } from "./Badge";
import { Button } from "./Button";
import { Card } from "./Card";
import type { Action } from "./control-types";
import { Divider } from "./Divider";
import { Grid } from "./Grid";
import { GridItem } from "./GridItem";
import { Inline } from "./Inline";
import { Page } from "./Page";
import { Section } from "./Section";
import { Split } from "./Split";
import { Stack } from "./Stack";
import { StepScreen } from "./StepScreen";
import { Text } from "./Text";
import { Toolbar } from "./Toolbar";

type Component =
  | "AppFrame"
  | "AppShell"
  | "Page"
  | "Section"
  | "Grid"
  | "GridItem"
  | "Stack"
  | "Inline"
  | "Split"
  | "Toolbar"
  | "Divider"
  | "StepScreen";

type Story = {
  readonly component: Component;
  readonly state: "normal" | "open" | "long";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};

const title = text("app.title", "ko");
const sectionTitle = text("catalog.holdings", "ko");
const company = text("catalog.company", "ko");
const companyTwo = text("catalog.companyTwo", "ko");
const companyThree = text("catalog.companyThree", "ko");
const description = text("catalog.description", "ko");
const day = text("ds.day", "ko");
const month = text("ds.month", "ko");
const year = text("ds.year", "ko");
const action: Action = {
  label: text("ds.retry", "ko"),
  onPress: (): void => undefined,
};

const entry = (
  component: Component,
  state: "normal" | "open" | "long",
  render: () => ReactElement,
): Story => ({ component, state, themes: ["light", "dark"], render });

const sampleCard = (heading: UiText): ReactElement => (
  <Card title={heading}>
    <Text>{description}</Text>
  </Card>
);

const sampleShellPage = (heading: UiText): ReactElement => (
  <Page title={title}>
    <Section title={sectionTitle}>{sampleCard(heading)}</Section>
  </Page>
);

const sampleList = (first: UiText): ReactElement => (
  <Stack gap="space-2">
    <Text>{first}</Text>
    <Text>{companyThree}</Text>
  </Stack>
);

const sampleChat = (lead: UiText): ReactElement => (
  <Stack gap="space-2">
    <Text variant="label">{lead}</Text>
    <Text>{description}</Text>
  </Stack>
);

export const layoutComponentStories: readonly Story[] = [
  entry("AppFrame", "normal", (): ReactElement => (
    <AppFrame>
      <AppShell>{sampleShellPage(companyTwo)}</AppShell>
    </AppFrame>
  )),
  entry("AppShell", "normal", (): ReactElement => (
    <AppShell
      list={sampleList(companyTwo)}
      chat={sampleChat(text("shell.chat", "ko"))}
    >
      {sampleShellPage(companyTwo)}
    </AppShell>
  )),
  entry("AppShell", "open", (): ReactElement => (
    <AppShell
      defaultChatOpen="open"
      defaultListOpen="open"
      list={sampleList(companyTwo)}
      chat={sampleChat(text("shell.chat", "ko"))}
    >
      {sampleShellPage(companyTwo)}
    </AppShell>
  )),
  entry("AppShell", "long", (): ReactElement => (
    <AppShell list={sampleList(company)} chat={sampleChat(company)}>
      {sampleShellPage(company)}
    </AppShell>
  )),
  entry("Page", "normal", (): ReactElement => sampleShellPage(companyTwo)),
  entry("Page", "normal", (): ReactElement => (
    <Page title={title} actions={action}>
      <Section title={sectionTitle}>{sampleCard(companyTwo)}</Section>
    </Page>
  )),
  entry("Section", "normal", (): ReactElement => (
    <Section title={sectionTitle}>{sampleCard(companyTwo)}</Section>
  )),
  entry("Grid", "normal", (): ReactElement => (
    <Grid>
      <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
        {sampleCard(companyTwo)}
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
        {sampleCard(companyThree)}
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 12, wide: 4 }}>
        {sampleCard(sectionTitle)}
      </GridItem>
    </Grid>
  )),
  entry("Grid", "normal", (): ReactElement => (
    <Grid>
      <GridItem span={{ narrow: 12, medium: 8, wide: 9 }}>
        {sampleCard(sectionTitle)}
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 4, wide: 3 }}>
        {sampleCard(companyTwo)}
      </GridItem>
    </Grid>
  )),
  ...([3, 4, 6, 8, 9, 12] as const).map((span) =>
    entry("GridItem", "normal", (): ReactElement => (
      <Grid>
        <GridItem span={span}>{sampleCard(companyTwo)}</GridItem>
      </Grid>
    )),
  ),
  entry("GridItem", "long", (): ReactElement => (
    <Grid>
      <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
        <Text>{company}</Text>
      </GridItem>
    </Grid>
  )),
  ...(["space-2", "space-4", "space-6", "space-8"] as const).map((gap) =>
    entry("Stack", "normal", (): ReactElement => (
      <Stack gap={gap}>
        <Text>{companyTwo}</Text>
        <Text>{companyThree}</Text>
      </Stack>
    )),
  ),
  entry("Stack", "long", (): ReactElement => (
    <Stack gap="space-4">
      <Text>{company}</Text>
      <Text>{description}</Text>
    </Stack>
  )),
  entry("Inline", "normal", (): ReactElement => (
    <Inline gap="space-2" align="center" justify="start" wrap="wrap">
      <Badge>{companyTwo}</Badge>
      <Badge tone="primary">{companyThree}</Badge>
    </Inline>
  )),
  entry("Inline", "normal", (): ReactElement => (
    <Inline gap="space-4" align="start" justify="between" wrap="wrap">
      <Text>{companyTwo}</Text>
      <Text variant="caption">{day}</Text>
    </Inline>
  )),
  entry("Inline", "normal", (): ReactElement => (
    <Inline gap="space-2" align="end" justify="end" wrap="nowrap">
      <Text>{day}</Text>
      <Text>{month}</Text>
    </Inline>
  )),
  entry("Inline", "long", (): ReactElement => (
    <Inline gap="space-2" wrap="nowrap">
      <Text>{company}</Text>
      <Text>{companyTwo}</Text>
    </Inline>
  )),
  ...(
    [
      "sidebar-width",
      "sidebar-collapsed",
      "chat-width",
      "chat-width-min",
    ] as const
  ).map((size) =>
    entry("Split", "normal", (): ReactElement => (
      <Split size={size}>
        <Card title={companyTwo}>
          <Text>{day}</Text>
        </Card>
        {sampleCard(sectionTitle)}
      </Split>
    )),
  ),
  entry("Split", "long", (): ReactElement => (
    <Split size="sidebar-width">
      <Text>{company}</Text>
      <Text>{description}</Text>
    </Split>
  )),
  entry("Toolbar", "normal", (): ReactElement => (
    <Toolbar>
      <Button size="control-height-sm">{day}</Button>
      <Button size="control-height-sm">{month}</Button>
      <Divider direction="vertical" />
      <Button size="control-height-sm" variant="ghost">
        {year}
      </Button>
    </Toolbar>
  )),
  entry("Divider", "normal", (): ReactElement => <Divider />),
  entry("Divider", "normal", (): ReactElement => (
    <Divider direction="vertical" />
  )),
  entry("StepScreen", "normal", (): ReactElement => (
    <StepScreen
      progress={["now", "next"]}
      progressLabel={sectionTitle}
      title={company}
      description={description}
      primary={action}
      secondary={action}
    >
      <Text>{day}</Text>
    </StepScreen>
  )),
];
