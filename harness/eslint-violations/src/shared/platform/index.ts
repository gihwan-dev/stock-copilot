import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";

export const mountApp = (app: Readonly<ReactElement>): void => {
  const container = document.getElementById("root");
  if (container !== null) {
    createRoot(container).render(app);
  }
};

export const receiveValue = (): Promise<"ok" | "error"> =>
  Promise.resolve<"ok">("ok").then(
    (value): "ok" => value,
    (): "error" => "error",
  );

export const inputCommit = (): "commit" | "ignore" => "ignore";
