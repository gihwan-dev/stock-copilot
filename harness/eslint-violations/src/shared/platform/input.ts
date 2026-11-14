export const receiveValue = (): Promise<"ok" | "error"> =>
  Promise.resolve<"ok">("ok").then(
    (value): "ok" => value,
    (): "error" => "error",
  );

export const inputCommit = (): "commit" | "ignore" => "ignore";

export const legacyComposition = (event: {
  readonly keyCode: KeyboardEvent["keyCode"];
}): "composing" | "idle" => (event.keyCode === 229 ? "composing" : "idle");
