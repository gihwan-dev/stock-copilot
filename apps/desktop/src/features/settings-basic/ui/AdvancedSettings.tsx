import type { ReactElement } from "react";
import { Disclosure } from "../../../shared/ds";
import { basicText } from "../messages";

export const AdvancedSettings = (props: {
  readonly children: ReactElement;
}): ReactElement => (
  <Disclosure title={basicText("basic.advanced", "ko")}>
    {props.children}
  </Disclosure>
);
