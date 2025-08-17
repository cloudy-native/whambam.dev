import { LinkedInIcon } from "@/components/icons";
import { Navbar } from "@/components/navbar";
import { siteConfig } from "@/config/site";
import { Link } from "@heroui/link";

export default function DefaultLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="relative flex flex-col h-screen">
      <Navbar />
      <main className="container mx-auto max-w-7xl px-6 flex-grow pt-16">
        {children}
      </main>
      <footer className="w-full flex items-center justify-center py-3">
        <p>
          Copyright &copy; {new Date().getFullYear()}, whambam.dev. Made with ❤️
          by Stephen Harrison{" "}
          <Link isExternal href={siteConfig.links.linkedin} title="LinkedIn">
            <LinkedInIcon className="text-default-500" />
          </Link>
          .
        </p>
      </footer>
    </div>
  );
}
