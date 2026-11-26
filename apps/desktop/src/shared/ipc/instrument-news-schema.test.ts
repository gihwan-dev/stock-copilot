import { expect, test } from "vitest";
import { text } from "../lib";
import { settleInstrumentDetail } from "./instruments-schema";
import { wire } from "./schema";
import { responses } from "./testing/responses";

const yonhap = {
  title: "Samsung expands HBM supply",
  source: "Yonhap",
  published_at: "2026-11-25T09:40:00+09:00",
  url: "https://www.yna.co.kr/view/AKR20261125000100003",
};

const relayed = {
  title: "Samsung third quarter results - Maeil",
  source: "GoogleNews",
  published_at: "2026-11-25T08:10:00+09:00",
  url: "http://insecure.example.com/a",
};

const unknownSource = { ...yonhap, source: "Naver" };

test("recent news lines carry the source label and an openable https link", () => {
  const result = settleInstrumentDetail({
    status: "ok",
    data: wire({ ...responses.getInstrumentDetail, news: [yonhap, relayed] }),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const lines = result.value.news ?? [];
  expect(lines.map((line) => line.source)).toStrictEqual([
    text("source.yonhap", "ko"),
    text("source.google-news", "ko"),
  ]);
  expect(lines.map((line) => line.url)).toStrictEqual([yonhap.url, undefined]);
  expect(new Set(lines.map((line) => line.id)).size).toBe(2);
});

test("an unknown news source rejects the whole detail response", () => {
  const result = settleInstrumentDetail({
    status: "ok",
    data: wire({ ...responses.getInstrumentDetail, news: [unknownSource] }),
  });
  expect(result.kind).toBe("err");
});

test("the recorded detail response has no news yet", () => {
  const result = settleInstrumentDetail({
    status: "ok",
    data: wire(responses.getInstrumentDetail),
  });
  expect(result.kind === "ok" ? result.value.news : undefined).toStrictEqual(
    [],
  );
});
