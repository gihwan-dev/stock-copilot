import * as allowlists from "./eslint/allowlists.mjs";
import { appConfig } from "./eslint/app-config.mjs";

export default appConfig(allowlists);
