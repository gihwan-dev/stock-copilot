import { StrictMode } from "react";
import { mountApp } from "../shared/platform";
import { App } from "./App";

mountApp(
  <StrictMode>
    <App />
  </StrictMode>,
);
