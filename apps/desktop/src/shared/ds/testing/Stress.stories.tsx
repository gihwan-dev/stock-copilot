import type { ReactElement } from "react";
import { emptyInputValue, parseInputValue, parseMoney } from "../../lib";
import { Text } from "../Text";
import { Heading } from "../Heading";
import { Button } from "../Button";
import { IconButton } from "../IconButton";
import { Icon } from "../Icon";
import { TextField } from "../TextField";
import { Select } from "../Select";
import { Checkbox } from "../Checkbox";
import { Switch } from "../Switch";
import { SegmentedControl } from "../SegmentedControl";
import { Disclosure } from "../Disclosure";
import { Card } from "../Card";
import { Table } from "../Table";
import { Dialog } from "../Dialog";
import { EmptyState } from "../EmptyState";
import { Skeleton } from "../Skeleton";
import { Badge } from "../Badge";
import { StatusBar } from "../StatusBar";
import { Money } from "../Numbers";
import { contentStressStories } from "./StressContent.stories";
import { layoutStressStories } from "./StressLayout.stories";
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
  | "Divider"
  | "Text"
  | "Heading"
  | "Button"
  | "IconButton"
  | "Icon"
  | "TextField"
  | "Select"
  | "Checkbox"
  | "Switch"
  | "SegmentedControl"
  | "Disclosure"
  | "Card"
  | "Table"
  | "Dialog"
  | "EmptyState"
  | "Skeleton"
  | "Badge"
  | "StatusBar"
  | "LineChart"
  | "AreaChart"
  | "DonutChart"
  | "Treemap"
  | "CandleChart"
  | "Money"
  | "Percent"
  | "Quantity"
  | "Probability"
  | "PriceChange"
  | "DirectionBadge"
  | "DataText"
  | "TimestampText"
  | "Markdown"
  | "ChatInput"
  | "StepScreen"
  | "ListRow";
type Story = {
  readonly component: Component;
  readonly state: "stress";
  readonly themes: readonly ["light", "dark"];
  readonly render: (theme?: "light" | "dark") => ReactElement | null;
};

const label = stressFixtures.company;
const optionValue = parseInputValue(stressFixtures.english);
const value = optionValue.kind === "ok" ? optionValue.value : emptyInputValue;
const digits = parseInputValue(stressFixtures.digits);
const inputDigits = digits.kind === "ok" ? digits.value : emptyInputValue;
const options = [
  { value, label },
  { value: emptyInputValue, label: stressFixtures.english },
] as const;
const action = { label, onPress: (): void => undefined };
const money = parseMoney(stressFixtures.money, "KRW");
const onSend = (): void => undefined;

const stories: readonly Omit<Story, "state" | "themes">[] = [
  { component: "Text", render: (): ReactElement => <Text>{label}</Text> },
  {
    component: "Text",
    render: (): ReactElement => <Text>{stressFixtures.english}</Text>,
  },
  {
    component: "Heading",
    render: (): ReactElement => <Heading>{label}</Heading>,
  },
  { component: "Button", render: (): ReactElement => <Button>{label}</Button> },
  {
    component: "IconButton",
    render: (): ReactElement => (
      <IconButton icon="x" label={label} onClick={onSend} />
    ),
  },
  {
    component: "Icon",
    render: (): ReactElement => <Icon name="wallet" label={label} />,
  },
  {
    component: "TextField",
    render: (): ReactElement => (
      <TextField
        label={label}
        defaultValue={inputDigits}
        hint={stressFixtures.english}
      />
    ),
  },
  {
    component: "Select",
    render: (): ReactElement => <Select label={label} options={options} />,
  },
  {
    component: "Checkbox",
    render: (): ReactElement => (
      <Checkbox label={label} description={stressFixtures.english} />
    ),
  },
  {
    component: "Switch",
    render: (): ReactElement => (
      <Switch label={label} description={stressFixtures.english} />
    ),
  },
  {
    component: "SegmentedControl",
    render: (): ReactElement => (
      <SegmentedControl label={label} options={options} />
    ),
  },
  {
    component: "Disclosure",
    render: (): ReactElement => (
      <Disclosure title={label} defaultOpen="open">
        <Text>{stressFixtures.english}</Text>
      </Disclosure>
    ),
  },
  {
    component: "Card",
    render: (): ReactElement => (
      <Card title={label} actions={action}>
        <Text>{stressFixtures.english}</Text>
      </Card>
    ),
  },
  {
    component: "Table",
    render: (): ReactElement => (
      <Table.Root caption={label}>
        <Table.Head>
          <Table.Row>
            <Table.Header>{label}</Table.Header>
            <Table.Header>{stressFixtures.english}</Table.Header>
          </Table.Row>
        </Table.Head>
        <Table.Body>
          {stressFixtures.holdings.map((id): ReactElement => (
            <tr key={id}>
              <Table.RowHeader>{label}</Table.RowHeader>
              <Table.Cell>
                {money.kind === "ok" ? (
                  <Money value={money.value} />
                ) : (
                  stressFixtures.empty
                )}
              </Table.Cell>
            </tr>
          ))}
        </Table.Body>
      </Table.Root>
    ),
  },
  {
    component: "Dialog",
    render: (): ReactElement => (
      <Dialog
        open="open"
        title={label}
        actions={{ cancel: action, confirm: action }}
      >
        <Text>{stressFixtures.english}</Text>
      </Dialog>
    ),
  },
  {
    component: "EmptyState",
    render: (): ReactElement => (
      <EmptyState title={label} action={action}>
        {stressFixtures.english}
      </EmptyState>
    ),
  },
  {
    component: "EmptyState",
    render: (): ReactElement => (
      <EmptyState title={stressFixtures.empty}>
        {stressFixtures.english}
      </EmptyState>
    ),
  },
  {
    component: "Skeleton",
    render: (): ReactElement => (
      <>
        <Skeleton lines={6} />
        <Text>{label}</Text>
      </>
    ),
  },
  { component: "Badge", render: (): ReactElement => <Badge>{label}</Badge> },
  {
    component: "StatusBar",
    render: (): ReactElement => <StatusBar action={action}>{label}</StatusBar>,
  },
  ...contentStressStories,
  ...layoutStressStories,
];

export const stressComponentStories: readonly Story[] = stories.map(
  (story) => ({
    ...story,
    state: "stress",
    themes: ["light", "dark"],
  }),
);
