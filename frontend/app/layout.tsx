import type { Metadata, Viewport } from "next";

import { Header } from "@/components/header";
import { SiteFooter } from "@/components/site-footer";
import { Providers } from "@/app/providers";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "Hackathon Hunter — China hackathons, found by AI",
    template: "%s · Hackathon Hunter",
  },
  description:
    "A personal AI agent that discovers hackathons in China, extracts structured details, and delivers a daily digest.",
  applicationName: "Hackathon Hunter",
  authors: [{ name: "Hackathon Hunter" }],
  keywords: [
    "hackathon",
    "China",
    "黑客松",
    "编程大赛",
    "AI agent",
    "SearXNG",
    "9router",
  ],
  openGraph: {
    title: "Hackathon Hunter",
    description:
      "Find your next hackathon in China. Discovered by SearXNG, extracted by 9router.",
    type: "website",
  },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f8fafc" },
    { media: "(prefers-color-scheme: dark)", color: "#070b14" },
  ],
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <Providers>
          <div className="flex min-h-screen flex-col">
            <Header />
            <main className="mx-auto w-full max-w-7xl flex-1 px-4 pb-16 sm:px-6 lg:px-8">
              {children}
            </main>
            <SiteFooter />
          </div>
        </Providers>
      </body>
    </html>
  );
}
