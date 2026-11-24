import { onlineManager } from "@tanstack/react-query";
import { afterEach, expect, test } from "vitest";
import { queryKeys, screenKeys } from "../shared/lib";
import { createAppQueryClient } from "./query";

afterEach(() => {
  onlineManager.setOnline(true);
});

test("queries and mutations reach the local core even while the network is offline", async () => {
  onlineManager.setOnline(false);
  const client = createAppQueryClient();
  expect(client.getDefaultOptions().queries?.networkMode).toBe("always");
  expect(client.getDefaultOptions().mutations?.networkMode).toBe("always");
  const stored = await client.query({
    queryKey: screenKeys.portfolio,
    queryFn: () => Promise.resolve({ kind: "ok", value: "stored" } as const),
  });
  expect(stored).toStrictEqual({ kind: "ok", value: "stored" });
  const history = await client.query({
    queryKey: [...queryKeys.status, "chat-history"],
    queryFn: () => Promise.resolve({ kind: "ok", value: "history" } as const),
  });
  expect(history).toStrictEqual({ kind: "ok", value: "history" });
  const mutation = client.getMutationCache().build(client, {
    mutationFn: () => Promise.resolve("sent" as const),
  });
  await expect(mutation.execute(undefined)).resolves.toBe("sent");
});
