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

export const resetCommands = (): VitestUtils => {
  clearMocks();
  return vi.unstubAllGlobals();
};
