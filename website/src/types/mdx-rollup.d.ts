declare module "@mdx-js/rollup" {
  import type { Plugin } from "vite";
  const mdx: (options?: Record<string, unknown>) => Plugin;
  export default mdx;
}
