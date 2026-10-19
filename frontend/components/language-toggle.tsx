"use client";

import { Languages } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n";

export function LanguageToggle() {
  const { toggleLang, t } = useI18n();
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={toggleLang}
      className="h-9 gap-1.5 rounded-full px-3 text-xs font-bold"
      aria-label="Toggle language"
    >
      <Languages className="h-4 w-4" />
      {t("lang.switch")}
    </Button>
  );
}
