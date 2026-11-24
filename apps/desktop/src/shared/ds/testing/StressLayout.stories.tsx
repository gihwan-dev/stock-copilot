import type { ReactElement } from "react";
import { AppShell } from "../AppShell";
import { Button } from "../Button";
import { Card } from "../Card";
import { Divider } from "../Divider";
import { Grid } from "../Grid";
import { GridItem } from "../GridItem";
import { Inline } from "../Inline";
import { Page } from "../Page";
import { Section } from "../Section";
import { Split } from "../Split";
import { Stack } from "../Stack";
import { Text } from "../Text";
import { Toolbar } from "../Toolbar";
import { stressFixtures } from "./stress";

type Component =
  | "AppShell"
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
  readonly render: () => ReactElement;
};

const label = stressFixtures.company;
const action = { label, onPress: (): void => undefined };

export const layoutStressStories: readonly Story[] = [
  {
    component: "AppShell",
    render: (): ReactElement => (
      <AppShell
        list={
          <Stack gap="space-2">
            <Text>{label}</Text>
            <Text>{stressFixtures.english}</Text>
          </Stack>
        }
        chat={
          <Stack gap="space-2">
            <Text>{label}</Text>
            <Text>{stressFixtures.paragraph}</Text>
          </Stack>
        }
      >
        <Page title={label} actions={action}>
          <Section title={label}>
            <Text>{stressFixtures.paragraph}</Text>
          </Section>
        </Page>
      </AppShell>
    ),
  },
  {
    component: "Page",
    render: (): ReactElement => (
      <Page title={label} actions={action}>
        <Section title={label}>
          <Text>{stressFixtures.english}</Text>
        </Section>
      </Page>
    ),
  },
  {
    component: "Section",
    render: (): ReactElement => (
      <Section title={label}>
        <Text>{stressFixtures.english}</Text>
      </Section>
    ),
  },
  {
    component: "Grid",
    render: (): ReactElement => (
      <Grid>
        <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
          <Card title={label}>
            <Text>{stressFixtures.english}</Text>
          </Card>
        </GridItem>
        <GridItem span={{ narrow: 12, medium: 6, wide: 8 }}>
          <Card title={label}>
            <Text>{stressFixtures.paragraph}</Text>
          </Card>
        </GridItem>
      </Grid>
    ),
  },
  {
    component: "GridItem",
    render: (): ReactElement => (
      <Grid>
        <GridItem span={{ narrow: 12, medium: 6, wide: 3 }}>
          <Text>{stressFixtures.english}</Text>
        </GridItem>
      </Grid>
    ),
  },
  {
    component: "Stack",
    render: (): ReactElement => (
      <Stack gap="space-4">
        <Text>{label}</Text>
        <Text>{stressFixtures.english}</Text>
      </Stack>
    ),
  },
  {
    component: "Inline",
    render: (): ReactElement => (
      <Inline gap="space-2" wrap="wrap">
        <Text>{label}</Text>
        <Text>{stressFixtures.english}</Text>
      </Inline>
    ),
  },
  {
    component: "Split",
    render: (): ReactElement => (
      <Split size="sidebar-width">
        <Text>{label}</Text>
        <Text>{stressFixtures.english}</Text>
      </Split>
    ),
  },
  {
    component: "Toolbar",
    render: (): ReactElement => (
      <Toolbar>
        <Button size="control-height-sm">{label}</Button>
        <Divider direction="vertical" />
        <Button size="control-height-sm" variant="ghost">
          {stressFixtures.english}
        </Button>
      </Toolbar>
    ),
  },
  {
    component: "Divider",
    render: (): ReactElement => (
      <>
        <Divider />
        <Text>{label}</Text>
      </>
    ),
  },
];
