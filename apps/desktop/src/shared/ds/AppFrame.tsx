import type { ReactElement } from "react";

type Props = {
  readonly children: ReactElement;
};

export const AppFrame = (props: Props): ReactElement => (
  <div
    data-app-frame=""
    className="flex flex-col w-full h-screen bg-bg text-ink scroll-flex"
  >
    {props.children}
  </div>
);
