import { createHash } from "node:crypto";

const INVISIBLE =
  /[\s\u00ad\u180e\u200b-\u200f\u202a-\u202e\u2060-\u2064\ufeff]/gu;

export const normalized = (text) =>
  (typeof text === "string" ? text : "")
    .normalize("NFKC")
    .replace(INVISIBLE, "")
    .toLowerCase();

export const lineDigest = (line) =>
  createHash("sha256").update(normalized(line)).digest("hex");

export const meaningfulLines = (text, minimum) =>
  (typeof text === "string" ? text : "")
    .split("\n")
    .filter((line) => normalized(line).length >= minimum);
