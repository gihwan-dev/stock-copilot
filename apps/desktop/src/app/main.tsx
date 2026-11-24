import { StrictMode } from "react";
import { getBriefings, pendingScreenSources } from "../shared/ipc";
import { mountApp } from "../shared/platform";
import { App } from "./App";

mountApp(
  <StrictMode>
    <App sources={{ ...pendingScreenSources, briefings: getBriefings }} />
  </StrictMode>,
);
