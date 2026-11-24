import { expect, test } from "vitest";
import { queryKeys } from "../shared/lib";
import { detach } from "../shared/platform";
import { createAppQueryClient } from "./query";

test("detach marks targeted and full query caches invalid without exposing a generic Promise helper", () => {
  const client = createAppQueryClient();
  expect(
    client.setQueryData(queryKeys.settings, { kind: "ok", value: {} }),
  ).toBeDefined();
  expect(
    client.setQueryData(queryKeys.portfolio, { kind: "ok", value: {} }),
  ).toBeDefined();
  detach({
    kind: "invalidate-queries",
    client,
    keys: [queryKeys.settings],
  });
  expect(client.getQueryState(queryKeys.settings)?.isInvalidated).toBe(true);
  expect(client.getQueryState(queryKeys.portfolio)?.isInvalidated).toBe(false);
  detach({ kind: "invalidate-all", client });
  expect(client.getQueryState(queryKeys.portfolio)?.isInvalidated).toBe(true);
});
