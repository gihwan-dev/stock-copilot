import { createContext, use, useMemo, type ReactElement } from "react";

type WidgetGap = "0" | "4" | "8" | "12";
type WidgetPadding = "0" | "4" | "8" | "16";

type Appearance = {
  readonly gap: WidgetGap;
  readonly padding: WidgetPadding;
  readonly radius: WidgetGap;
};

const AppearanceContext = createContext<
  readonly [WidgetGap, WidgetPadding, WidgetGap]
>(["0", "8", "0"]);

export const useWidgetAppearance = (): Appearance => {
  const [gap, padding, radius] = use(AppearanceContext);
  return { gap, padding, radius };
};

type Props = {
  readonly children: ReactElement;
  readonly widgetGap?: WidgetGap;
  readonly widgetPadding?: WidgetPadding;
  readonly widgetRadius?: WidgetGap;
};

export const AppFrame = ({
  children,
  widgetGap = "0",
  widgetPadding = "8",
  widgetRadius = "0",
}: Props): ReactElement => {
  const appearance = useMemo(
    () => [widgetGap, widgetPadding, widgetRadius] as const,
    [widgetGap, widgetPadding, widgetRadius],
  );
  return (
    <div
      data-app-frame=""
      data-widget-gap={widgetGap}
      data-widget-padding={widgetPadding}
      data-widget-radius={widgetRadius}
      className="flex flex-col w-full h-screen bg-bg text-ink scroll-flex"
    >
      <AppearanceContext value={appearance}>{children}</AppearanceContext>
    </div>
  );
};
