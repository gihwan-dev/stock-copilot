import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

const mount = `
  import { createElement as h } from "react";
  import { QueryClientProvider } from "@tanstack/react-query";
  import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
  import { mockCommands, events } from MOCK_IPC_ENTRY;
  import { useCoreEvent } from PLATFORM_ENTRY;
  function Panel() { useCoreEvent(); return h(ChatPanel); }
  import { createAppQueryClient } from QUERY_ENTRY;
  import { ChatPanel } from ${JSON.stringify(join(desktopRoot, "src/features/chat/index.ts"))};
  let client = createAppQueryClient();
  Object.assign(window, { fireEvent, waitFor, cleanup, mockCommands, invalidate: () => client.invalidateQueries(),
    emit: (payload) => events.uiEvent.emit(payload),
    fresh: () => { client = createAppQueryClient(); },
    mount: () => render(h(QueryClientProvider, {client}, h(Panel))) });
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
    await w.invalidate();
    await w.waitFor(() => assert.ok(box().readOnly));
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
    await w.invalidate();
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
    type("fixture-replaced-draft");
    entries.length = 0;
    await w.emit("StoredDataChanged");
    await w.waitFor(() => assert.ok(box() && !box().value && !view.container.textContent.includes("fixture-one")));
  } finally {
    bundle.close();
  }
});

test("only a storage replacement reopens a restored running turn", async () => {
  const bundle = await screenDom(mount);
  try {
    const w = bundle.open();
    const subscribers = [];
    let historyReads = 0;
    w.mockCommands({
      getChatHistory: () => {
        historyReads += 1;
        return {
          session_id: "main-chat",
          entries: [{
            turn_id: "persisted-turn", sequence: 1, at_unix_seconds: "1",
            role: "User", text: "fixture-question", status: null, cards: [], sources: [],
          }],
          active_turn: { turn_id: "persisted-turn", last_sequence: 0, state: "Running" },
        };
      },
      resubscribeChatTurn: ({ request, onChunk }) => {
        subscribers.push(onChunk);
        return { turn_id: request.turn_id, replayed_count: 0, state: "Running" };
      },
    });
    let view = w.mount();
    const emit = (subscriber, sequence, text) => subscriber.onmessage({
      turn_id: "persisted-turn", sequence, chunk: { TextDelta: { text } },
    });
    await w.waitFor(() => assert.equal(subscribers.length, 1));
    emit(subscribers[0], 1, "fixture-first");
    await w.emit({ AiApprovalRequested: "fixture-approval" });
    await w.invalidate();
    view.unmount();
    view = w.mount();
    await w.waitFor(() => assert.ok(view.container.textContent.includes("fixture-first")));
    assert.equal(subscribers.length, 1);
    assert.equal(historyReads, 1);
    await w.emit("StoredDataChanged");
    await w.waitFor(() => assert.equal(subscribers.length, 2));
    emit(subscribers[0], 2, "fixture-obsolete");
    emit(subscribers[1], 1, "fixture-current");
    await w.waitFor(() => assert.ok(view.container.textContent.includes("fixture-current")));
    assert.ok(!view.container.textContent.includes("fixture-obsolete"));
    assert.ok(!view.container.textContent.includes("fixture-first"));
  } finally {
    bundle.close();
  }
});
