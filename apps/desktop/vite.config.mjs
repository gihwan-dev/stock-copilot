import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [
    tailwindcss(),
    {
      name: "bundled-font-license",
      generateBundle() {
        this.emitFile({
          type: "asset",
          fileName: "OFL.txt",
          source: readFileSync(
            new URL("../../design/fonts/OFL.txt", import.meta.url),
          ),
        });
      },
    },
  ],
});
