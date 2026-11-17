import { createServer } from "node:http";
import { readFileSync, existsSync, statSync } from "node:fs";
import { resolve, extname, sep } from "node:path";
const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};
export const staticServer = (build) =>
  createServer((request, response) => {
    try {
      const file = resolve(
        build,
        `.${decodeURIComponent(new URL(request.url, "http://localhost").pathname)}`,
      );
      if (
        !file.startsWith(build + sep) ||
        !existsSync(file) ||
        !statSync(file).isFile()
      ) {
        response.writeHead(404);
        response.end();
        return;
      }
      response.setHeader(
        "Content-Type",
        types[extname(file)] ?? "application/octet-stream",
      );
      response.end(readFileSync(file));
    } catch {
      response.writeHead(400);
      response.end();
    }
  });
