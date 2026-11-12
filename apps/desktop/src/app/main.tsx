import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { rootElement } from "../shared/platform";
import { App } from "./App";

const container = rootElement();

if (container !== null) {
  createRoot(container).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
