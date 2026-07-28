import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tsconfigPaths from "vite-tsconfig-paths";
import mdx from "@mdx-js/rollup";
import remarkGfm from "remark-gfm";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    mdx({
      // Process both .mdx and .md files as MDX so they become React components
      include: ["**/*.mdx", "**/*.md"],
      remarkPlugins: [remarkGfm],
    }),
    react(),
    tsconfigPaths(),
  ],
});
