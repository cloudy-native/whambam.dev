import DefaultLayout from "@/layouts/default";
import { title } from "@/components/primitives";
import type { ComponentType } from "react";

interface MarkdownPageProps {
  titleText?: string;
  Content: ComponentType<Record<string, unknown>>;
}

export default function MarkdownPage({ titleText, Content }: MarkdownPageProps) {
  return (
    <DefaultLayout>
      <main className="w-full p-8">
        {titleText ? (
          <section className="mb-8 scroll-mt-24">
            <h1 className={title({ size: "lg" })}>{titleText}</h1>
          </section>
        ) : null}
        <section
          className="prose prose-zinc dark:prose-invert max-w-none
          prose-headings:scroll-mt-24 prose-h1:mt-0 prose-h2:mt-10 prose-h3:mt-8
          prose-a:text-blue-600 dark:prose-a:text-blue-400 hover:prose-a:underline
          prose-pre:bg-zinc-50 dark:prose-pre:bg-zinc-950 prose-pre:text-zinc-900 dark:prose-pre:text-zinc-100
          prose-pre:border prose-pre:border-zinc-200 dark:prose-pre:border-zinc-800 prose-pre:rounded-lg
          prose-code:before:content-[''] prose-code:after:content-['']
          prose-img:rounded-lg prose-hr:my-10"
        >
          <Content />
        </section>
      </main>
    </DefaultLayout>
  );
}
