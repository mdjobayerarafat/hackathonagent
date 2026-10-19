"use client";

import { Activity, LayoutDashboard, Settings, Sparkles } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";

import { LanguageToggle } from "@/components/language-toggle";
import { RunAgentButton } from "@/components/run-agent-button";
import { ThemeToggle } from "@/components/theme-toggle";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export function Header() {
  const pathname = usePathname();
  const { t } = useI18n();

  const links = [
    { href: "/", label: t("nav.dashboard"), icon: LayoutDashboard },
    { href: "/runs", label: t("nav.runs"), icon: Activity },
    { href: "/settings", label: t("nav.settings"), icon: Settings },
  ];

  return (
    <header className="sticky top-0 z-40 border-b border-border/60 bg-background/75 backdrop-blur-xl">
      <div className="mx-auto flex h-16 w-full max-w-7xl items-center gap-3 px-4 sm:px-6 lg:px-8">
        <Link href="/" className="group flex items-center gap-2.5">
          <span className="relative flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 via-violet-500 to-fuchsia-500 text-white shadow-md shadow-indigo-500/25 transition-transform group-hover:-rotate-6">
            <Sparkles className="h-[18px] w-[18px]" />
          </span>
          <span className="flex flex-col leading-none">
            <span className="font-display text-[15px] font-extrabold tracking-tight">
              Hackathon Hunter
            </span>
            <span className="mt-1 hidden whitespace-nowrap text-[10px] font-medium uppercase tracking-[0.18em] text-muted-foreground lg:block">
              {t("brand.tagline")}
            </span>
          </span>
        </Link>

        <nav className="ml-2 flex items-center gap-0.5 sm:ml-5">
          {links.map(({ href, label, icon: Icon }) => {
            const active =
              href === "/" ? pathname === "/" : pathname.startsWith(href);
            return (
              <Link
                key={href}
                href={href}
                aria-label={label}
                title={label}
                className={cn(
                  "flex items-center gap-2 rounded-lg px-2.5 py-2 text-sm font-semibold transition-colors sm:px-3",
                  active
                    ? "bg-primary/10 text-primary"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )}
              >
                <Icon className="h-4 w-4" />
                <span className="hidden md:inline">{label}</span>
              </Link>
            );
          })}
        </nav>

        <div className="ml-auto flex items-center gap-1 sm:gap-1.5">
          <LanguageToggle />
          <ThemeToggle />
          <div className="mx-1 hidden h-5 w-px bg-border sm:block" />
          <RunAgentButton />
        </div>
      </div>
    </header>
  );
}
