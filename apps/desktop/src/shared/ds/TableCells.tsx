import type { ReactElement } from "react";
import type { UiText } from "../lib";
type Content = { readonly children?: UiText | ReactElement | undefined };
const stickyHeaderStyles = {
  start: "text-left bg-surface-sunken text-ink-muted text-label",
  end: "text-right bg-surface-sunken text-ink-muted text-label",
  name: "w-full text-left bg-surface-sunken text-ink-muted text-label",
} as const;
export const StickyHeader = (props: {
  readonly alignment: "start" | "end" | "name";
  readonly children: UiText | ReactElement | undefined;
}): ReactElement => (
  <th scope="col" className={stickyHeaderStyles[props.alignment]}>
    <span className="block px-space-3 py-space-2 border-b-hairline border-solid border-line">
      {props.children}
    </span>
  </th>
);
export const PositionCell = (props: Content): ReactElement => (
  <td className="px-space-3 py-space-1 border-t-hairline border-line text-right text-body-sm text-ink-muted">
    <div className="h-full flex flex-col justify-center items-end">
      {props.children}
    </div>
  </td>
);
export const PositionRowHeader = (props: Content): ReactElement => (
  <th
    scope="row"
    className="px-space-3 py-space-1 border-t-hairline border-line text-left font-regular"
  >
    <div className="h-full flex flex-col justify-center items-start">
      {props.children}
    </div>
  </th>
);
