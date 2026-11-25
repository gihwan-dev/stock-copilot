import { StrictMode } from "react";
import { mountApp } from "../shared/platform";
import { CoreApp } from "./App";

mountApp(
  <StrictMode>
    <CoreApp />
  </StrictMode>,
);
