declare module "*.md" {
  import type { ComponentType } from "react";
  const MDComponent: ComponentType<Record<string, unknown>>;
  export default MDComponent;
}
