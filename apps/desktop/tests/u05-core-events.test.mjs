import assert from "node:assert/strict";
import test from "node:test";
import { screenDom } from "./screen-dom.mjs";

test("U-05 mounted app subscribes to core ui-event and invalidates queries on core events", async () => {
  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, cleanup, waitFor } from "@testing-library/react";
    import { clearMocks } from "@tauri-apps/api/mocks";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands, events } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    import { queryKeys } from LIB_ENTRY;

    window.cleanup = cleanup;
    window.clearMocks = clearMocks;

    window.runCoreEventFlow = async () => {
      let appVersionCalls = 0;
      mockCommands({
        appVersion: () => {
          appVersionCalls += 1;
          return responses.appVersion;
        },
      });
      const queryClient = createAppQueryClient();
      const view = render(h(App, { initialPath: "/", queryClient }));

      await waitFor(() => {
        if (appVersionCalls < 1) throw new Error("initial appVersion not fetched");
      });

      const seedAll = async () => {
        for (const key of [
          queryKeys.settings,
          queryKeys.portfolio,
          queryKeys.allocation,
          queryKeys.transactions,
          queryKeys.selfCheck,
          queryKeys.status,
        ]) {
          await queryClient.prefetchQuery({
            queryKey: key,
            queryFn: async () => ({ seeded: true }),
          });
        }
      };

      await seedAll();
      await events.uiEvent.emit("SettingsChanged");
      await waitFor(() => {
        if (!queryClient.getQueryState(queryKeys.settings)?.isInvalidated) {
          throw new Error("settings not invalidated");
        }
      });
      const settingsCheck = {
        settings: queryClient.getQueryState(queryKeys.settings)?.isInvalidated,
        allocation: queryClient.getQueryState(queryKeys.allocation)?.isInvalidated,
        selfCheck: queryClient.getQueryState(queryKeys.selfCheck)?.isInvalidated,
        portfolio: queryClient.getQueryState(queryKeys.portfolio)?.isInvalidated,
      };

      await seedAll();
      await events.uiEvent.emit("PortfolioSyncRecorded");
      await waitFor(() => {
        if (!queryClient.getQueryState(queryKeys.portfolio)?.isInvalidated) {
          throw new Error("portfolio not invalidated");
        }
      });
      const portfolioCheck = {
        portfolio: queryClient.getQueryState(queryKeys.portfolio)?.isInvalidated,
        allocation: queryClient.getQueryState(queryKeys.allocation)?.isInvalidated,
        transactions: queryClient.getQueryState(queryKeys.transactions)?.isInvalidated,
      };

      await seedAll();
      await events.uiEvent.emit("TransactionsSyncRecorded");
      await waitFor(() => {
        if (!queryClient.getQueryState(queryKeys.transactions)?.isInvalidated) {
          throw new Error("transactions not invalidated");
        }
      });

      await seedAll();
      await events.uiEvent.emit("SelfCheckChanged");
      await waitFor(() => {
        if (!queryClient.getQueryState(queryKeys.selfCheck)?.isInvalidated) {
          throw new Error("selfCheck not invalidated");
        }
      });
      const statusInvalidated = queryClient.getQueryState(queryKeys.status)?.isInvalidated;

      const beforeResetCalls = appVersionCalls;
      await events.uiEvent.emit("StoredDataChanged");
      await waitFor(() => {
        if (appVersionCalls <= beforeResetCalls) {
          throw new Error("appVersion was not refetched after StoredDataChanged");
        }
      });

      const beforeAccountCalls = appVersionCalls;
      await events.uiEvent.emit("AccountSelectionChanged");
      await waitFor(() => {
        if (appVersionCalls <= beforeAccountCalls) {
          throw new Error("appVersion was not refetched after AccountSelectionChanged");
        }
      });

      const afterAccountCalls = appVersionCalls;
      view.unmount();
      await events.uiEvent.emit("StoredDataChanged");
      await new Promise((resolve) => setTimeout(resolve, 20));
      const afterUnmountCalls = appVersionCalls;

      return {
        settingsCheck,
        portfolioCheck,
        statusInvalidated,
        afterAccountCalls,
        afterUnmountCalls,
      };
    };
  `);
  try {
    const win = bundle.open();
    const result = await win.runCoreEventFlow();
    assert.deepEqual(
      { ...result.settingsCheck },
      {
        settings: true,
        allocation: true,
        selfCheck: true,
        portfolio: false,
      },
    );
    assert.deepEqual(
      { ...result.portfolioCheck },
      {
        portfolio: true,
        allocation: true,
        transactions: false,
      },
    );
    assert.equal(result.statusInvalidated, true);
    assert.equal(result.afterUnmountCalls, result.afterAccountCalls);
  } finally {
    bundle.close();
  }
});
