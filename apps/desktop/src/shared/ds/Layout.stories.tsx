import type { ReactElement } from "react";
import { text } from "../lib";
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
import { Text } from "./Text";
import { Toolbar } from "./Toolbar";

type Component =
  | "Page"
  | "Section"
  | "Grid"
  | "GridItem"
  | "Stack"
  | "Inline"
  | "Split"
  | "Toolbar"
  | "Divider";

type Story = {
  readonly component: Component;
  readonly state: "normal" | "long";
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
  onPress: (): void => {
    return;
  },
};

const entry = (
  component: Component,
  state: "normal" | "long",
  render: () => ReactElement,
): Story => ({
  component,
  state,
  themes: ["light", "dark"],
  render,
});

export const layoutComponentStories: readonly Story[] = [
  entry("Page", "normal", (): ReactElement => (
    <Page title={title}>
      <Section title={sectionTitle}>
        <Card title={companyTwo}>
          <Text>{description}</Text>
        </Card>
      </Section>
    </Page>
  )),
  entry("Page", "normal", (): ReactElement => (
    <Page title={title} actions={action}>
      <Section title={sectionTitle}>
        <Card title={companyTwo}>
          <Text>{description}</Text>
        </Card>
      </Section>
    </Page>
  )),
  entry("Section", "normal", (): ReactElement => (
    <Section title={sectionTitle}>
      <Card title={companyTwo}>
        <Text>{description}</Text>
      </Card>
    </Section>
  )),
  entry("Grid", "normal", (): ReactElement => (
    <Grid>
      <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
        <Card title={companyTwo}>
          <Text>{description}</Text>
        </Card>
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
        <Card title={companyThree}>
          <Text>{description}</Text>
        </Card>
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 12, wide: 4 }}>
        <Card title={sectionTitle}>
          <Text>{description}</Text>
        </Card>
      </GridItem>
    </Grid>
  )),
  entry("Grid", "normal", (): ReactElement => (
    <Grid>
      <GridItem span={{ narrow: 12, medium: 8, wide: 9 }}>
        <Card title={sectionTitle}>
          <Text>{description}</Text>
        </Card>
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 4, wide: 3 }}>
        <Card title={companyTwo}>
          <Text>{description}</Text>
        </Card>
      </GridItem>
    </Grid>
  )),
  ...([3, 4, 6, 8, 9, 12] as const).map((span) =>
    entry("GridItem", "normal", (): ReactElement => (
      <Grid>
        <GridItem span={span}>
          <Card title={companyTwo}>
            <Text>{description}</Text>
          </Card>
        </GridItem>
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
        <Card title={sectionTitle}>
          <Text>{description}</Text>
        </Card>
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
];
