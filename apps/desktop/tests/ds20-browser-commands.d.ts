import "vitest/browser";

declare module "vitest/browser" {
  interface BrowserCommands {
    prepareCatalog(): Promise<void>;
    catalogErrors(): Promise<string[]>;
  }
}

declare global {
  type BufferEncoding =
    | "ascii"
    | "utf8"
    | "utf-8"
    | "utf16le"
    | "utf-16le"
    | "ucs2"
    | "ucs-2"
    | "base64"
    | "base64url"
    | "latin1"
    | "binary"
    | "hex";
}
