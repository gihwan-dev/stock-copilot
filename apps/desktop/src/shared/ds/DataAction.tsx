import { useId, type ReactElement } from "react";
import type { UiText } from "../lib";
import { Badge } from "./Badge";

export const dataClasses = {
  emphasis: "status-sentence text-title-sm font-semibold text-ink",
  body: "status-sentence text-body text-ink",
  "body-sm": "status-sentence text-body-sm text-ink-muted",
  caption: "status-sentence text-caption text-ink-subtle",
  code: "status-sentence text-code text-ink-muted",
} as const;

export const DataAction = (props: {
  readonly onPress: () => void;
  readonly children: ReactElement;
  readonly alert: UiText | undefined;
}): ReactElement => {
  const statusId = useId();
  return (
    <div className="flex flex-wrap items-start gap-space-2 max-w-full">
      <button
        type="button"
        onClick={props.onPress}
        aria-describedby={props.alert === undefined ? undefined : statusId}
        className="block max-w-full text-title-sm text-left text-ink rounded-sm cursor-pointer hover:text-primary focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
      >
        {props.children}
      </button>
      {props.alert === undefined ? null : (
        <span id={statusId} className="inline-flex flex-none">
          <Badge tone="alert" outline="visible">
            {props.alert}
          </Badge>
        </span>
      )}
    </div>
  );
};
