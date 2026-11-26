import { appendFileSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { buildResponses } from "./portfolio-model.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const catalog = JSON.parse(
  readFileSync(join(here, "..", "mcp", "tools_list.json"), "utf8"),
);
const scenarioPath = process.env.CHAT_EVAL_SCENARIO;
const callLog = process.env.CHAT_EVAL_CALL_LOG;
const scenario = JSON.parse(readFileSync(scenarioPath, "utf8"));
const responses = buildResponses(scenario);

const send = (message) => process.stdout.write(`${JSON.stringify(message)}\n`);

const toolResult = (payload, isError) => ({
  content: [{ type: "text", text: JSON.stringify(payload) }],
  isError,
  structuredContent: payload,
});

const unavailable = (field) =>
  toolResult(
    {
      code: "data_unavailable",
      field,
      message: "no data for the requested input",
      retryable: false,
    },
    true,
  );

const lookup = (name, args) => {
  const entry = responses[name];
  if (entry === undefined) return unavailable(null);
  if (entry && entry.__bySymbol) {
    const symbol = String(args.symbol ?? "").toUpperCase();
    const hit = entry.__bySymbol[symbol];
    return hit === undefined ? unavailable("symbol") : toolResult(hit, false);
  }
  return toolResult(entry, false);
};

const handle = (message) => {
  const { id, method, params } = message;
  if (id === undefined) return;
  if (method === "initialize") {
    send({
      jsonrpc: "2.0",
      id,
      result: {
        capabilities: { tools: {} },
        protocolVersion: params?.protocolVersion ?? "2025-06-18",
        serverInfo: { name: "stock-copilot-mcp", version: "0.1.0" },
      },
    });
    return;
  }
  if (method === "tools/list") {
    send({ jsonrpc: "2.0", id, result: { tools: catalog } });
    return;
  }
  if (method === "tools/call") {
    const name = params?.name ?? "";
    const args = params?.arguments ?? {};
    const known = catalog.some((tool) => tool.name === name);
    const result = known
      ? lookup(name, args)
      : toolResult(
          {
            code: "unknown_tool",
            field: "name",
            message: "requested tool is not registered",
            retryable: false,
          },
          true,
        );
    if (callLog) {
      appendFileSync(
        callLog,
        `${JSON.stringify({ name, args, isError: result.isError })}\n`,
      );
    }
    send({ jsonrpc: "2.0", id, result });
    return;
  }
  if (method === "ping") {
    send({ jsonrpc: "2.0", id, result: {} });
    return;
  }
  send({
    jsonrpc: "2.0",
    id,
    error: { code: -32601, message: "method not found" },
  });
};

createInterface({ input: process.stdin }).on("line", (line) => {
  if (line.trim() === "") return;
  try {
    handle(JSON.parse(line));
  } catch {
    send({
      jsonrpc: "2.0",
      id: null,
      error: { code: -32700, message: "parse error" },
    });
  }
});
