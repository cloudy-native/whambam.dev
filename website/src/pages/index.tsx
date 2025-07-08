import { Image } from "@heroui/image";
import { Snippet } from "@heroui/snippet";

import { subtitle, title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";

export default function IndexPage() {
  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-2xl text-center justify-center">
          <span className={title()}>Run&nbsp;</span>
          <span className={title({ color: "violet" })}>whambam&nbsp;</span>
          <br />
          <span className={title()}>to test your website performance.</span>
          <div className={subtitle({ class: "mt-4" })}>
            whambam is your new favourite web performance testing tool.
          </div>
          <div className={subtitle({ class: "mt-4" })}>
            whambam is available on Intel and Apple Silicon Macs today. Windows
            and Linux coming soon.
          </div>
        </div>

        <div className="mt-8">
          <Snippet variant="bordered">
            <span>brew tap cloudy-native/whambam</span>
            <span>brew install whambam</span>
            <span>whambam -z 10s https://example.com</span>
          </Snippet>
        </div>

        <div className="mt-8 flex justify-center">
          <Image
            shadow="sm"
            radius="lg"
            width="100%"
            alt="whambam UI screenshot"
            src="/images/ui.png"
          />
        </div>
      </section>
    </DefaultLayout>
  );
}
