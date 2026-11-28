import type { ReactElement } from "react";
import type { UiText } from "../lib";
import type { Availability } from "./control-types";
import { ReferenceRow, type ReferenceProps } from "./ReferenceRow";
import { Heading } from "./Heading";
import { IconButton } from "./IconButton";
import { SwitchControl, type SwitchState } from "./Switch";

type Trailing =
  | { readonly kind: "none" }
  | {
      readonly kind: "link";
      readonly label: UiText;
      readonly onPress: () => void;
    }
  | {
      readonly kind: "fold";
      readonly label: UiText;
      readonly state: "open" | "closed";
      readonly onPress: () => void;
    }
  | {
      readonly kind: "switch";
      readonly checked: SwitchState;
      readonly disabled?: Availability;
      readonly onChange: (state: SwitchState) => void;
    };

type Props = {
  readonly title: UiText;
  readonly value?: UiText;
  readonly trailing: Trailing;
  readonly children?: ReactElement | readonly ReactElement[];
};

const End = (props: {
  readonly title: UiText;
  readonly value: UiText | undefined;
  readonly trailing: Trailing;
}): ReactElement | null => {
  switch (props.trailing.kind) {
    case "none":
      return null;
    case "link":
      return (
        <IconButton
          icon="chevron-right"
          label={props.trailing.label}
          onClick={props.trailing.onPress}
        />
      );
    case "fold":
      return (
        <IconButton
          icon={props.trailing.state === "open" ? "chevron-up" : "chevron-down"}
          label={props.trailing.label}
          expanded={props.trailing.state}
          onClick={props.trailing.onPress}
        />
      );
    case "switch": {
      const trailing = props.trailing;
      return SwitchControl({
        props: {
          label: props.title,
          ...(props.value === undefined ? {} : { description: props.value }),
          checked: trailing.checked,
          disabled: trailing.disabled ?? "enabled",
          onChange: trailing.onChange,
        },
        state: trailing.checked,
        change: (next): void => {
          if (trailing.disabled !== "disabled") trailing.onChange(next);
        },
      });
    }
  }
};

const settingRow = (props: Props): ReactElement => (
  <div className="flex flex-col gap-space-3 px-space-4 py-space-4 border-b-hairline border-solid border-line">
    <div className="flex items-center gap-space-3">
      <div className="flex flex-col flex-1 gap-space-1 scroll-flex">
        <Heading level={3}>{props.title}</Heading>
        {props.value === undefined ? null : (
          <p className="status-sentence text-body-sm text-ink-muted">
            {props.value}
          </p>
        )}
      </div>
      <End title={props.title} value={props.value} trailing={props.trailing} />
    </div>
    {props.children === undefined ? null : (
      <div className="flex flex-col gap-space-3">{props.children}</div>
    )}
  </div>
);

type FieldProps = {
  readonly variant: "field";
  readonly label: UiText;
  readonly children: ReactElement;
};
export const ListRow = (
  props:
    Props | (ReferenceProps & { readonly variant: "reference" }) | FieldProps,
): ReactElement => {
  if (!("variant" in props)) return settingRow(props);
  if (props.variant === "reference") return <ReferenceRow entry={props} />;
  return (
    <div className="grid grid-cols-layout items-start gap-space-2">
      <span className="col-span-3 text-label text-ink-muted pt-space-1">
        {props.label}
      </span>
      <div className="col-span-9 scroll-flex">{props.children}</div>
    </div>
  );
};
