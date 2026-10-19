"use client";

import { ThemeProvider, useTheme } from "next-themes";
import { Toaster } from "sonner";
import type { ReactNode } from "react";

import { LanguageProvider } from "@/lib/i18n";

function ThemedToaster() {
  const { theme } = useTheme();
  return (
    <Toaster
      theme={(theme as "light" | "dark" | "system") ?? "system"}
      position="top-right"
      closeButton
      richColors
      toastOptions={{
        className: "font-sans",
      }}
    />
  );
}

export function Providers({ children }: { children: ReactNode }) {
  return (
    <ThemeProvider
      attribute="class"
      defaultTheme="system"
      enableSystem
      disableTransitionOnChange
    >
      <LanguageProvider>
        {children}
        <ThemedToaster />
      </LanguageProvider>
    </ThemeProvider>
  );
}
