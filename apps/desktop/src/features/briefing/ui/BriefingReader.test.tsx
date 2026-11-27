import { expect, test, vi } from "vitest";
import { Card, Markdown } from "../../../shared/ds";
import type { BriefingSummary } from "../../../shared/ipc";
import {
  parseEntityId,
  parseFreeText,
  parseTimestamp,
  type ExternalUrl,
} from "../../../shared/lib";
import { briefingText } from "../messages";
import { Body } from "./BriefingReader";
import { BriefingView } from "./BriefingView";

const lead = "🌅".repeat(177);
const citation = "123";
const tail = `[${citation}] closing paragraph`;
const briefing = (): BriefingSummary => {
  const id = parseEntityId("morning-2026-11-25");
  const at = parseTimestamp("2026-11-25T08:20:00+09:00");
  const body = parseFreeText(lead + tail);
  if (id.kind === "err" || at.kind === "err" || body.kind === "err")
    return expect.unreachable();
  return {
    id: id.value,
    kind: "morning",
    finishedAt: at.value,
    outcome: { kind: "completed", body: body.value, sources: [] },
  };
};

test("the dashboard excerpt ends before a partial citation and uses the preview scale", () => {
  const body = Body({
    briefing: briefing(),
    onOpenSource: vi.fn<(url: ExternalUrl) => void>(),
    presentation: "excerpt",
  });
  expect(body).toMatchObject({
    type: Markdown,
    props: { content: lead + "…", scale: "body" },
  });
});

test("the full briefing keeps the complete citation and final paragraph at reading size", () => {
  const full = Body({
    briefing: briefing(),
    onOpenSource: vi.fn<(url: ExternalUrl) => void>(),
    presentation: "full",
  });
  expect(full.props).toHaveProperty("children.0.type", Markdown);
  expect(full.props).toHaveProperty("children.0.props.content", lead + tail);
  expect(full.props).toHaveProperty("children.0.props.scale", "reading");
});

test.each(["morning" as const, "pre-us-market" as const])(
  "the %s session is a titled card without an expand action",
  (kind) => {
    const card = BriefingView({
      briefing: { ...briefing(), kind },
      onOpenSource: vi.fn<(url: ExternalUrl) => void>(),
      onOpenHistory: undefined,
    });
    expect(card).toMatchObject({
      type: Card,
      props: { title: briefingText(kind, "ko") },
    });
    expect(card.props).not.toHaveProperty("actions");
  },
);
