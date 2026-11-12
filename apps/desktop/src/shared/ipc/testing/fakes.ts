import { clearMocks } from "@tauri-apps/api/mocks";
import { vi, type VitestUtils } from "vitest";
import { mockCommands } from "./mockBindings";
import { responses } from "./responses";

const frameworkRejection = "app_version not allowed. Plugin not found";

export const installCommandWindow = (): VitestUtils =>
  vi.stubGlobal("window", {});

export const answerWithRecordedResponses = (): void => {
  mockCommands({ appVersion: () => responses.appVersion });
};

export const rejectWithFrameworkError = (): void => {
  mockCommands({
    appVersion: () => {
      throw frameworkRejection;
    },
  });
};

export const returnRecordedCommandError = (): void => {
  mockCommands({
    appVersion: () => {
      throw responses.ipcErrors[2];
    },
  });
};

export const answerWithMalformedResponse = (): void => {
  mockCommands({ appVersion: () => ({ version: "not a version" }) });
};

export const rejectWithErrorObject = (): void => {
  mockCommands({
    appVersion: () => {
      throw new Error("invoke failed");
    },
  });
};

export const rejectWithUnknownObject = (): void => {
  mockCommands({
    appVersion: () => {
      throw { reason: "denied" };
    },
  });
};

export const rejectWithUndefined = (): void => {
  mockCommands({
    appVersion: () => {
      throw undefined;
    },
  });
};

export const resetCommands = (): VitestUtils => {
  clearMocks();
  return vi.unstubAllGlobals();
};
