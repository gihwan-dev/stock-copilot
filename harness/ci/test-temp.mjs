import { after } from "node:test";
import { cleanup } from "./managed-temp.mjs";

export { mkdtempSync } from "./managed-temp.mjs";

after(cleanup);
