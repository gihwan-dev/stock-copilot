import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { createRequire } from "node:module";

const root = resolve(import.meta.dirname, "../..");
const desktop = join(root, "apps/desktop");
const require = createRequire(join(desktop, "package.json"));

export const buildAudit = async (output) => {
  const { build } = await import(require.resolve("vite"));
  const { default: tailwindcss } = await import(
    require.resolve("@tailwindcss/vite")
  );
  const stage = join(desktop, "node_modules/.screen-audit");
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  const entry = relative(stage, join(root, "harness/screen-audit/app.mjs"));
  writeFileSync(
    join(stage, "index.html"),
    `<!doctype html>
<html lang="ko">
  <head>
    <meta charset="UTF-8" />
    <link rel="stylesheet" href="/src/shared/ds/theme.css" />
    <title>Stock Copilot</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="./${entry}"></script>
  </body>
</html>
`,
  );
  await build({
    root: desktop,
    configFile: false,
    logLevel: "error",
    plugins: [tailwindcss()],
    resolve: {
      alias: [
        { find: /^react$/u, replacement: require.resolve("react") },
        {
          find: /^react-dom\/client$/u,
          replacement: require.resolve("react-dom/client"),
        },
      ],
    },
    define: { "process.env.NODE_ENV": '"development"' },
    server: { fs: { allow: [root] } },
    build: {
      outDir: output,
      emptyOutDir: true,
      minify: false,
      rollupOptions: { input: join(stage, "index.html") },
    },
  });
  rmSync(stage, { recursive: true, force: true });
  return `/${relative(desktop, join(stage, "index.html"))}`;
};
