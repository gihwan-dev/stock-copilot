import { Fragment, type ReactElement } from "react";
import { Grid, GridItem, Stack } from "../../../shared/ds";
import { useWindowLayout } from "../../../shared/platform";

type Sections = {
  readonly overview: ReactElement;
  readonly positions: ReactElement | undefined;
  readonly allocation: ReactElement | undefined;
  readonly reports: ReactElement;
  readonly briefing: ReactElement;
  readonly signals: ReactElement;
};

const sectionOrder = {
  compact: {
    first: ["overview", "positions", "briefing", "signals", "allocation"],
    second: ["reports"],
  },
  "summary-row": {
    first: ["positions", "allocation", "signals"],
    second: ["briefing", "reports"],
  },
  "two-columns": {
    first: ["overview", "positions", "allocation", "signals"],
    second: ["briefing", "reports"],
  },
  "three-columns": {
    first: ["overview", "positions"],
    second: ["briefing", "reports"],
  },
} as const satisfies Record<
  ReturnType<typeof useWindowLayout>,
  { first: readonly (keyof Sections)[]; second: readonly (keyof Sections)[] }
>;

const column = (
  order: readonly (keyof Sections)[],
  sections: Sections,
): ReactElement => (
  <Stack gap="space-4">
    {order.flatMap((key) => {
      const section = sections[key];
      return section === undefined
        ? []
        : [<Fragment key={key}>{section}</Fragment>];
    })}
  </Stack>
);

export const OverviewColumns = (props: Sections): ReactElement => {
  const layout = useWindowLayout();
  const order = sectionOrder[layout];
  const third = layout === "three-columns";
  const span = layout === "compact" ? 12 : 6;
  const inner = (
    <Grid layout="fit-row">
      <GridItem span={span}>{column(order.first, props)}</GridItem>
      <GridItem span={span}>{column(order.second, props)}</GridItem>
    </Grid>
  );
  return (
    <Grid layout="fit-row">
      <GridItem span={third ? 9 : 12}>
        <Stack gap="space-4">
          {[
            ...(layout === "summary-row"
              ? [<Fragment key="summary">{props.overview}</Fragment>]
              : []),
            <Fragment key="columns">{inner}</Fragment>,
          ]}
        </Stack>
      </GridItem>
      {third
        ? [
            <GridItem key="updates" span={3}>
              {column(["signals", "allocation"], props)}
            </GridItem>,
          ]
        : []}
    </Grid>
  );
};
