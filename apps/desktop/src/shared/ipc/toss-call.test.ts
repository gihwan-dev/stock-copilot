import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { ok } from "../lib";
import { parseSecretText, type SecretText } from "./schema";
import { coreSetupClient } from "./settings-client";
import {
  failTossCommands,
  installCommandWindow,
  resetCommands,
} from "./testing/fakes";
import { mockCommands } from "./testing/mockBindings";
import { responses } from "./testing/responses";

beforeEach(installCommandWindow);
afterEach(resetCommands);

const secret = (input: "fake-id" | "fake-secret"): SecretText => {
  const parsed = parseSecretText(input);
  expect(parsed.kind).toBe("ok");
  return parsed.kind === "ok" ? parsed.value : expect.fail("secret");
};

test("the core connection check returns the recorded connection state", async () => {
  mockCommands({ checkTossConnection: () => responses.checkTossConnection });
  await expect(coreSetupClient.checkTossConnection()).resolves.toStrictEqual(
    ok({ state: "connected" }),
  );
});

test("submitted keys are sent once as the request and the reply state is returned", async () => {
  const handler = vi.fn(() => responses.submitTossKeys);
  mockCommands({ submitTossKeys: handler });
  await expect(
    coreSetupClient.submitTossKeys({
      clientId: secret("fake-id"),
      secret: secret("fake-secret"),
    }),
  ).resolves.toStrictEqual(ok({ state: "ip-not-allowed" }));
  expect(handler).toHaveBeenCalledOnce();
  expect(handler).toHaveBeenCalledWith({
    request: { client_id: "fake-id", secret: "fake-secret" },
  });
});

test("a retryable core error stays a command failure", async () => {
  failTossCommands();
  await expect(
    coreSetupClient.submitTossKeys({
      clientId: secret("fake-id"),
      secret: secret("fake-secret"),
    }),
  ).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "retryable" } },
  });
});
