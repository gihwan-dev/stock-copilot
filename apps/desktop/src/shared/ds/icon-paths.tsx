import type { ReactElement } from "react";
import { paths0 } from "./icon-paths-0";
import { paths1 } from "./icon-paths-1";
import { paths2 } from "./icon-paths-2";
const paths = { ...paths0, ...paths1, ...paths2 };
export type IconName = Exclude<keyof typeof paths, "targetIcon"> | "target";
export const IconPaths = (props: { readonly name: IconName }): ReactElement =>
  props.name === "target" ? paths.targetIcon : paths[props.name];
