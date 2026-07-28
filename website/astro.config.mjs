import { defineConfig } from "astro/config";
import tailwind from "@astrojs/tailwind";
import remarkGfm from "remark-gfm";

// https://astro.build/config
export default defineConfig({
  site: "https://whambam.dev",
  integrations: [tailwind()],
  markdown: {
    remarkPlugins: [remarkGfm],
    shikiConfig: {
      theme: "github-dark",
      wrap: true,
    },
  },
});
