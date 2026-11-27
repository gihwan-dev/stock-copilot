import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

const mount = `
  import { createElement as h } from "react";
  import { QueryClientProvider } from "@tanstack/react-query";
  import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
  import { mockCommands } from MOCK_IPC_ENTRY;
  import { createAppQueryClient } from QUERY_ENTRY;
  import { ChatPanel } from ${JSON.stringify(join(desktopRoot, "src/features/chat/index.ts"))};
  let client = createAppQueryClient();
  Object.assign(window, { fireEvent, waitFor, cleanup, mockCommands, fresh: () => { client = createAppQueryClient(); },
    mount: () => render(h(QueryClientProvider, {client}, h(ChatPanel))) });
`;

test("navigation restores ordered turns, a pending reply and the draft", async () => {
  const bundle = await screenDom(mount);
  try {
    const w = bundle.open();
    let finish;
    const entries = [];
    const append = (request, role, text, status) => entries.push({
      turn_id: request.turn_id, sequence: entries.length + 1, at_unix_seconds: "1",
      role, text, status, cards: [], sources: [],
    });
    w.mockCommands({
      getChatHistory: () => ({ session_id: "main-chat", entries: [...entries], active_turn: null }),
      sendChatMessage: ({ request, onChunk }) => new Promise((resolve) => {
        append(request, "User", request.message, null);
        finish = () => {
          append(request, "Assistant", "fixture-reply", "Completed");
          onChunk.onmessage({ sequence: 1, turn_id: request.turn_id,
            chunk: { TextDelta: { text: "fixture-reply" } } });
          resolve({ ...request, status: "Completed", mode: "FreshAnalysis",
            actual_model: null, first_token_latency_ms: null,
            resets_at_unix_seconds: null, failure_reason: null });
        };
      }),
    });
    let view = w.mount();
    const box = () => view.container.querySelector("textarea");
    const ready = () => w.waitFor(() => assert.ok(box()));
    const type = (value) => w.fireEvent.change(box(), {target: {value}});
    const send = async (value) => {
      type(value);
      const button = [...view.container.querySelectorAll("button")]
        .find((item) => item.textContent === "보내기");
      w.fireEvent.click(button);
      await w.waitFor(() => assert.ok(finish));
    };
    await ready();
    await send("fixture-one");
    finish();
    finish = undefined;
    await w.waitFor(() => assert.ok(view.container.textContent.includes("fixture-reply")));
    await send("fixture-two");
    view.unmount();
    view = w.mount();
    await ready();
    assert.ok(view.container.textContent.includes("fixture-two"));
    view.unmount();
    finish();
    view = w.mount();
    await ready();
    await w.waitFor(() => assert.equal(view.container.textContent.split("fixture-reply").length, 3));
    assert.deepEqual(view.container.textContent.match(/fixture-(?:one|two|reply)/g),
      ["fixture-one", "fixture-reply", "fixture-two", "fixture-reply"]);
    type("fixture-draft");
    view.unmount();
    view = w.mount();
    await ready();
    assert.ok(box().value === "fixture-draft");
    view.unmount();
    w.fresh();
    view = w.mount();
    await ready();
    assert.ok(box().value === "");
    assert.deepEqual(view.container.textContent.match(/fixture-(?:one|two|reply)/g),
      ["fixture-one", "fixture-reply", "fixture-two", "fixture-reply"]);
  } finally {
    bundle.close();
  }
});
