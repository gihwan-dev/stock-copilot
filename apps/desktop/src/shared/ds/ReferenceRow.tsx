import { useId, type ReactElement } from "react";
import type { FreeText, Timestamp, UiText } from "../lib";
import { DataText, TimestampText } from "./Numbers";

export type ReferenceProps = {
  readonly number: FreeText;
  readonly sourceTitle: FreeText;
  readonly observedAt: Timestamp | null;
  readonly linkLabel: UiText;
  readonly onPress?: (() => void) | undefined;
};

export const ReferenceRow = ({
  entry: props,
}: {
  readonly entry: ReferenceProps;
}): ReactElement => {
  const numberId = useId();
  const titleId = useId();
  const actionId = useId();
  return (
    <div className="flex items-start gap-space-2">
      <span id={numberId} className="flex-none font-mono tabular-nums">
        <DataText value={props.number} variant="code" />
      </span>
      <div className="flex-1 scroll-flex">
        <div id={titleId}>
          <DataText value={props.sourceTitle} />
        </div>
        <div className="flex flex-wrap items-center gap-space-3">
          {props.observedAt === null ? null : (
            <TimestampText value={props.observedAt} variant="caption" />
          )}
          {props.onPress === undefined ? null : (
            <button
              type="button"
              role="link"
              onClick={props.onPress}
              aria-labelledby={`${actionId} ${numberId} ${titleId}`}
              className="markdown-link text-primary text-caption rounded-sm focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
            >
              <span id={actionId}>{props.linkLabel}</span>
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
