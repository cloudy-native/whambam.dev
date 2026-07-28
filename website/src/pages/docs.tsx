import MarkdownPage from "@/components/MarkdownPage";
import DocsContent from "@/content/docs.md";

export default function DocsPage() {
  return <MarkdownPage titleText="Documentation" Content={DocsContent} />;
}
