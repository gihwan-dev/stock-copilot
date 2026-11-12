import { ipcTestingBoundary } from "./rules/ipc-testing-boundary.mjs";
import { emptyParser, forbiddenFile } from "./rules/forbidden-file.mjs";
import { noBrandCreation } from "./rules/no-brand-creation.mjs";
import { noComments } from "./rules/no-comments.mjs";
import { noComputedMember } from "./rules/no-computed-member.mjs";
import { noMoneyCoercion } from "./rules/no-money-coercion.mjs";
import { noPromiseCatch } from "./rules/no-promise-catch.mjs";
import { noRawTypes } from "./rules/no-raw-types.mjs";
import { noUiText } from "./rules/no-ui-text.mjs";
import { testFileBoundary } from "./rules/test-file-boundary.mjs";

export const local = {
  meta: { name: "local" },
  rules: {
    "no-comments": noComments,
    "no-money-coercion": noMoneyCoercion,
    "forbidden-file": forbiddenFile,
    "no-raw-types": noRawTypes,
    "no-brand-creation": noBrandCreation,
    "no-ui-text": noUiText,
    "no-promise-catch": noPromiseCatch,
    "no-computed-member": noComputedMember,
    "test-file-boundary": testFileBoundary,
    "ipc-testing-boundary": ipcTestingBoundary,
  },
};

export { emptyParser };
