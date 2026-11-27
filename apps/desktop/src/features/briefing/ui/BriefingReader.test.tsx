import { expect, test, vi } from "vitest";
import { Card, Dialog } from "../../../shared/ds";
import type { BriefingSummary } from "../../../shared/ipc";
import {
  parseEntityId,
  parseFreeText,
  parseTimestamp,
  type ExternalUrl,
} from "../../../shared/lib";
import { briefingText } from "../messages";
import { Reader } from "./BriefingReader";
import { ReadingCard } from "./BriefingView";

const briefing = (): BriefingSummary => {
  const id = parseEntityId("morning-2026-11-25");
  const at = parseTimestamp("2026-11-25T08:20:00+09:00");
  const body = parseFreeText("body");
  if (id.kind === "err" || at.kind === "err" || body.kind === "err")
    return expect.unreachable();
  return {
    id: id.value,
    kind: "morning",
    finishedAt: at.value,
    outcome: { kind: "completed", body: body.value, sources: [] },
  };
};

const reader = (
  onClose: () => void,
  onOpenHistory: (() => void) | undefined,
): ReturnType<typeof Reader> =>
  Reader({
    briefing: briefing(),
    onOpenSource: vi.fn<(url: ExternalUrl) => void>(),
    onOpenHistory,
    onClose,
  });

test("reader is an open wide dialog titled with the briefing kind", () => {
  const element = reader(vi.fn<() => void>(), undefined);
  expect(element.type).toBe(Dialog);
  expect(element.props.open).toBe("open");
  expect(element.props.wide).toBe("wide");
  expect(element.props.title).toBe(briefingText("morning", "ko"));
});

test("escape, the close mark and the close button all close the reader", () => {
  const onClose = vi.fn<() => void>();
  const props = reader(onClose, undefined).props;
  expect(props.onClose?.("escape")).toBeUndefined();
  expect(props.onClose?.("close-button")).toBeUndefined();
  expect(props.actions?.cancel?.label).toBe(briefingText("closeReader", "ko"));
  expect(props.actions?.cancel?.onPress()).toBeUndefined();
  expect(onClose).toHaveBeenCalledTimes(3);
  expect(props.actions?.confirm).toBeUndefined();
});

test("reader leads to past briefings when there are some", () => {
  const onOpenHistory = vi.fn<() => void>();
  const props = reader(vi.fn<() => void>(), onOpenHistory).props;
  expect(props.actions?.confirm?.label).toBe(briefingText("openHistory", "ko"));
  expect(props.actions?.confirm?.onPress()).toBeUndefined();
  expect(onOpenHistory).toHaveBeenCalledTimes(1);
});

test.each(["morning" as const, "pre-us-market" as const])(
  "the %s card header opens the reader",
  (kind) => {
    const changeReader = vi.fn<(next: "open" | "closed") => void>();
    const card = ReadingCard({
      view: {
        briefing: { ...briefing(), kind },
        onOpenSource: vi.fn<(url: ExternalUrl) => void>(),
        onOpenHistory: undefined,
        layout: "fill",
      },
      reader: "closed",
      changeReader,
    });
    expect(card.type).toBe(Card);
    expect(card.props.layout).toBe("fill");
    expect(card.props.actions?.label).toBe(briefingText("openReader", "ko"));
    expect(card.props.actions?.onPress()).toBeUndefined();
    expect(changeReader).toHaveBeenCalledWith("open");
  },
);
