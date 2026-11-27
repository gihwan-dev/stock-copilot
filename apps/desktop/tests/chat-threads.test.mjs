import assert from "node:assert/strict";
import test from "node:test";
import { join } from "node:path";
import { chatThreads } from "../../../harness/screen-audit/chat-threads.mjs";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

const source = `
  import { createElement as h, Fragment } from "react";
  import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
  import { QueryClientProvider } from "@tanstack/react-query";
  import { createAppQueryClient } from QUERY_ENTRY;
  import { mockCommands } from MOCK_IPC_ENTRY;
  import { ChatWorkspace } from ${JSON.stringify(join(desktopRoot, "src/features/chat/index.ts"))};
  let client = createAppQueryClient();
  Object.assign(window, {
    fresh: () => { client = createAppQueryClient(); },
    invalidate: () => client.invalidateQueries(),
    mount: () => render(h(QueryClientProvider,{client},h(ChatWorkspace,null,
      ({ messages, composer }) => h(Fragment,null,messages,composer)))),
    waitFor, fireEvent, cleanup, mockCommands,
  });
`;

test("keeps delayed streams and drafts in their selected sessions and reopens stored sessions", async () => {
  const bundle = await screenDom(source);
  try {
    const w = bundle.open();
    const finishes = [];
    const commands = chatThreads(
      () => new Promise((resolve) => finishes.push(resolve)),
    );
    w.mockCommands(commands);
    let view = w.mount();
    const box = () => view.container.querySelector("textarea");
    const nav = () => view.container.querySelector('nav[aria-label="채팅"]');
    const btn = (label) =>
      [...nav().querySelectorAll("button")].find(
        (b) => b.textContent.trim() === label,
      );
    const current = () =>
      nav().querySelector('[aria-current="page"]')?.textContent;
    const ready = async () => w.waitFor(() => assert.ok(box()));
    const select = async (label) => {
      w.fireEvent.click(btn(label));
      await w.waitFor(() => assert.equal(current(), label));
      await ready();
    };
    const type = (value) => w.fireEvent.change(box(), { target: { value } });
    const submit = (value) => {
      type(value);
      w.fireEvent.keyDown(box(), { key: "Enter" });
    };
    await ready();
    assert.equal(current(), "기존 시장 대화");
    type("legacy-draft");
    w.fireEvent.click(btn("새 채팅"));
    await w.waitFor(() => assert.equal(current(), "제목 없는 대화"));
    await ready();
    assert.equal(box().value, "");
    submit("fixture-alpha");
    await w.waitFor(() => assert.equal(finishes.length, 1));
    await select("기존 시장 대화");
    assert.equal(box().value, "legacy-draft");
    finishes.shift()();
    await w.waitFor(() => assert.ok(btn("fixture-alpha")));
    assert.ok(
      !view.container.textContent.includes("예시 응답 · fixture-alpha"),
    );
    await select("fixture-alpha");
    await w.waitFor(() =>
      assert.ok(
        view.container.textContent.includes("예시 응답 · fixture-alpha"),
      ),
    );
    type("alpha-draft");
    view.unmount();
    view = w.mount();
    await ready();
    assert.equal(current(), "fixture-alpha");
    assert.equal(box().value, "alpha-draft");
    w.fireEvent.click(btn("새 채팅"));
    await w.waitFor(() => assert.equal(current(), "제목 없는 대화"));
    await ready();
    assert.equal(box().value, "");
    assert.equal(commands.listChatSessions().length, 3);
    view.unmount();
    w.fresh();
    view = w.mount();
    await ready();
    assert.equal(nav().querySelectorAll("li").length, 3);
    await select("fixture-alpha");
    assert.equal(box().value, "");
    assert.ok(view.container.textContent.includes("예시 응답 · fixture-alpha"));
    await select("기존 시장 대화");
    assert.ok(view.container.textContent.includes("저장된 예시 답변입니다."));
    w.fireEvent.click(btn("채팅"));
    assert.equal(nav().querySelectorAll("li").length, 0);
    assert.ok(box());
  } finally {
    bundle.close();
  }
});

test("empty storage opens a writable chat and failed creation preserves its draft", async () => {
  const bundle = await screenDom(source);
  try {
    const w = bundle.open();
    const commands = chatThreads(() => Promise.resolve(), []);
    let attempts = 0;
    w.mockCommands({
      ...commands,
      createChatSession: (payload) => {
        attempts += 1;
        if (attempts === 1) throw { kind: "retryable" };
        return commands.createChatSession(payload);
      },
    });
    const view = w.mount();
    const box = () => view.container.querySelector("textarea");
    const add = () => view.getByRole("button", { name: "새 채팅" });
    await w.waitFor(() => assert.ok(box()));
    w.fireEvent.change(box(), { target: { value: "initial-draft" } });
    w.fireEvent.click(add());
    await view.findByText("대화를 만들지 못했어요. 다시 시도해 주세요.");
    assert.equal(box().value, "initial-draft");
    assert.equal(commands.listChatSessions().length, 0);
    w.fireEvent.click(add());
    await view.findByRole("button", {
      name: "제목 없는 대화",
      current: "page",
    });
    await w.waitFor(() => assert.ok(box() && box().value === ""));
    assert.equal(commands.listChatSessions().length, 2);
    w.fireEvent.click(view.getByRole("button", { name: "기본 대화" }));
    await w.waitFor(() => assert.ok(box() && box().value === "initial-draft"));
    w.fireEvent.keyDown(box(), { key: "Enter" });
    await view.findByText("예시 응답 · initial-draft");
    assert.ok(
      commands
        .listChatSessions()
        .some((entry) => entry.session_id === "main-chat"),
    );
  } finally {
    bundle.close();
  }
});

test("first empty creation stores one row and empty siblings have distinct labels", async () => {
  const bundle = await screenDom(source);
  try {
    const w = bundle.open();
    const commands = chatThreads(() => Promise.resolve(), []);
    w.mockCommands(commands);
    const view = w.mount();
    await view.findByRole("button", { name: "기본 대화" });
    const add = view.getByRole("button", { name: "새 채팅" });
    w.fireEvent.click(add);
    await view.findByRole("button", {
      name: "제목 없는 대화",
      current: "page",
    });
    assert.equal(view.queryByRole("button", { name: "기본 대화" }), null);
    assert.equal(commands.listChatSessions().length, 1);
    w.fireEvent.click(add);
    const first = await view.findByRole("button", { name: "제목 없는 대화 1" });
    const second = await view.findByRole("button", {
      name: "제목 없는 대화 2",
    });
    assert.notEqual(first, second);
    assert.equal(commands.listChatSessions().length, 2);
    assert.equal(view.queryByRole("button", { name: "기본 대화" }), null);
  } finally {
    bundle.close();
  }
});

test("list failure blocks initial sending and retains the selected writable session on refresh", async () => {
  const bundle = await screenDom(source);
  try {
    const w = bundle.open();
    const sent = [];
    const commands = chatThreads(
      (session) => {
        sent.push(session);
        return Promise.resolve();
      },
      [
        ["custom-a", "fixture-A"],
        ["custom-b", "fixture-B"],
      ],
    );
    let failed = true;
    w.mockCommands({
      ...commands,
      listChatSessions: () => {
        if (failed) throw { kind: "retryable" };
        return commands.listChatSessions();
      },
    });
    const view = w.mount();
    const retry = await view.findByRole("button", { name: "다시 불러오기" });
    assert.equal(view.container.querySelector("textarea"), null);
    failed = false;
    w.fireEvent.click(retry);
    const first = await view.findByRole("button", { name: "fixture-A" });
    assert.equal(view.queryByRole("button", { name: "기본 대화" }), null);
    w.fireEvent.click(first);
    await view.findByRole("button", { name: "fixture-A", current: "page" });
    failed = true;
    await w.invalidate();
    await view.findByRole("button", { name: "다시 불러오기" });
    const box = view.container.querySelector("textarea");
    assert.ok(box);
    assert.equal(first.getAttribute("aria-current"), "page");
    w.fireEvent.change(box, { target: { value: "fixture-next" } });
    w.fireEvent.keyDown(box, { key: "Enter" });
    await w.waitFor(() => assert.deepEqual(sent, ["custom-a"]));
  } finally {
    bundle.close();
  }
});
