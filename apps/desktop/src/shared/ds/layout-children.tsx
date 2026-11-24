import { isValidElement, type ReactElement } from "react";

export type LayoutChildren = {
  readonly children:
    ReactElement | readonly (ReactElement | readonly ReactElement[])[];
};

type WrapperMode = "block" | "inline";

const flatten = (props: LayoutChildren): readonly ReactElement[] =>
  isValidElement(props.children)
    ? [props.children]
    : props.children.flatMap((child) =>
        isValidElement(child) ? [child] : child,
      );

const keyState = (items: readonly ReactElement[]): "keyed" | "unkeyed" => {
  const keys = items.flatMap((child) =>
    child.key === null ? [] : [child.key],
  );
  return keys.length === items.length && new Set(keys).size === keys.length
    ? "keyed"
    : "unkeyed";
};

const UnkeyedWrappers = (props: {
  readonly items: readonly ReactElement[];
  readonly mode: WrapperMode;
}): ReactElement | null => {
  const first = props.items.at(0);
  if (first === undefined) return null;
  return (
    <>
      <div
        className={
          props.mode === "block"
            ? "scroll-flex w-full max-w-full"
            : "scroll-flex max-w-full"
        }
      >
        {first}
      </div>
      <UnkeyedWrappers items={props.items.slice(1)} mode={props.mode} />
    </>
  );
};

export const LayoutItems = (
  props: LayoutChildren & { readonly mode: WrapperMode },
): ReactElement => {
  const flat = flatten(props);
  if (keyState(flat) === "keyed")
    return (
      <>
        {flat.map((child): ReactElement => (
          <div
            key={child.key}
            className={
              props.mode === "block"
                ? "scroll-flex w-full max-w-full"
                : "scroll-flex max-w-full"
            }
          >
            {child}
          </div>
        ))}
      </>
    );
  return <UnkeyedWrappers items={flat} mode={props.mode} />;
};
