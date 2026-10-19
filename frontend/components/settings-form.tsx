"use client";

import {
  AlertTriangle,
  BrainCircuit,
  CalendarClock,
  CheckCircle2,
  Database,
  Eye,
  EyeOff,
  Info,
  Loader2,
  Mail,
  Save,
  Search,
  Send,
  Settings2,
  ShieldCheck,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { toast } from "sonner";

import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { ApiError, api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { SettingsUpdate, SettingsView } from "@/lib/types";
import { cn } from "@/lib/utils";

interface FormState {
  database_url: string;
  test_database_url: string;
  searxng_url: string;
  ninerouter_base_url: string;
  ninerouter_model: string;
  ninerouter_api_key: string;
  app_api_key: string;
  frontend_origins: string;
  dashboard_url: string;
  resend_api_key: string;
  email_from: string;
  email_to: string;
  auto_send_email: boolean;
  confidence_threshold: string;
  cron_schedule: string;
  cron_timezone: string;
  test_recipient: string;
}

function toForm(settings: SettingsView): FormState {
  return {
    database_url: settings.database_url,
    test_database_url: settings.test_database_url,
    searxng_url: settings.searxng_url,
    ninerouter_base_url: settings.ninerouter_base_url,
    ninerouter_model: settings.ninerouter_model,
    ninerouter_api_key: "",
    app_api_key: "",
    frontend_origins: settings.frontend_origins,
    dashboard_url: settings.dashboard_url,
    resend_api_key: "",
    email_from: settings.email_from,
    email_to: settings.email_to,
    auto_send_email: settings.auto_send_email,
    confidence_threshold: String(settings.confidence_threshold),
    cron_schedule: settings.cron_schedule,
    cron_timezone: settings.cron_timezone,
    test_recipient: "",
  };
}

export function SettingsForm() {
  const { t } = useI18n();
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [form, setForm] = useState<FormState | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    api
      .getSettings(controller.signal)
      .then((value) => {
        if (controller.signal.aborted) return;
        setSettings(value);
        setForm(toForm(value));
      })
      .catch((err: unknown) => {
        if (err instanceof DOMException && err.name === "AbortError") return;
        if (controller.signal.aborted) return;
        setError(err instanceof ApiError ? err.message : t("error.loadBody"));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [reloadKey, t]);

  const update = <K extends keyof FormState>(key: K, value: FormState[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  };

  const buildPayload = useCallback((): SettingsUpdate => {
    if (!form) return {};
    const threshold = Number(form.confidence_threshold);
    return {
      database_url: form.database_url.trim(),
      test_database_url: form.test_database_url.trim(),
      searxng_url: form.searxng_url.trim(),
      ninerouter_base_url: form.ninerouter_base_url.trim(),
      ninerouter_model: form.ninerouter_model.trim(),
      ninerouter_api_key: form.ninerouter_api_key.trim() || undefined,
      app_api_key: form.app_api_key.trim() || undefined,
      frontend_origins: form.frontend_origins.trim(),
      dashboard_url: form.dashboard_url.trim(),
      resend_api_key: form.resend_api_key.trim() || undefined,
      email_from: form.email_from.trim(),
      email_to: form.email_to.trim(),
      auto_send_email: form.auto_send_email,
      confidence_threshold: Number.isFinite(threshold) ? threshold : undefined,
      cron_schedule: form.cron_schedule.trim(),
      cron_timezone: form.cron_timezone.trim(),
    };
  }, [form]);

  const persist = useCallback(
    async (silent: boolean): Promise<SettingsView | null> => {
      setSaving(true);
      try {
        const saved = await api.updateSettings(buildPayload());
        setSettings(saved);
        setForm((current) => ({
          ...toForm(saved),
          test_recipient: current?.test_recipient ?? "",
        }));
        if (!silent) {
          toast.success(
            saved.restart_required.includes("database_url")
              ? t("settings.savedRestart")
              : t("settings.saved"),
          );
        }
        return saved;
      } catch (err) {
        toast.error(t("settings.errorSave"), {
          description:
            err instanceof ApiError ? err.message : t("error.action"),
        });
        return null;
      } finally {
        setSaving(false);
      }
    },
    [buildPayload, t],
  );

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    await persist(false);
  };

  const handleTestEmail = async () => {
    setTesting(true);
    try {
      const saved = await persist(true);
      if (!saved) return;
      if (!saved.resend_api_key_set) {
        toast.error(t("settings.emailNotReady"), {
          description: t("settings.resendKeyHint"),
        });
        return;
      }
      const result = await api.testEmail(form?.test_recipient.trim() || undefined);
      toast.success(
        t("settings.testSent", { to: result.recipients.join(", ") }),
        { description: result.resend_id ?? undefined },
      );
    } catch (err) {
      toast.error(t("settings.emailNotReady"), {
        description:
          err instanceof ApiError ? err.message : t("error.action"),
      });
    } finally {
      setTesting(false);
    }
  };

  if (loading && !form) {
    return (
      <div className="space-y-4 pt-10">
        <Skeleton className="h-9 w-52" />
        <Skeleton className="h-4 w-96 max-w-full" />
        {Array.from({ length: 4 }).map((_, index) => (
          <Skeleton key={index} className="h-48 w-full rounded-2xl" />
        ))}
      </div>
    );
  }

  if (error || !form || !settings) {
    return (
      <div className="pt-10">
        <EmptyState
          icon={<AlertTriangle className="h-6 w-6" />}
          tone="danger"
          title={t("settings.errorLoad")}
          body={error ?? t("error.loadBody")}
          actionLabel={t("error.retry")}
          onAction={() => setReloadKey((key) => key + 1)}
        />
      </div>
    );
  }

  const restartRequired = settings.restart_required.includes("database_url");

  return (
    <form onSubmit={handleSubmit} autoComplete="off" className="pt-10 pb-28 sm:pt-12">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div className="flex items-start gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
            <Settings2 className="h-5 w-5" />
          </span>
          <div>
            <h1 className="font-display text-2xl font-extrabold tracking-tight sm:text-3xl">
              {t("settings.title")}
            </h1>
            <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
              {t("settings.subtitle")}
            </p>
          </div>
        </div>
        <Badge variant="secondary" className="w-fit gap-1.5">
          <ShieldCheck className="h-3.5 w-3.5 text-emerald-500" />
          {t("settings.serverBadge")}
        </Badge>
      </div>

      {restartRequired ? (
        <div className="mt-6 flex gap-3 rounded-2xl border border-amber-500/30 bg-amber-500/10 p-4">
          <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-amber-600 dark:text-amber-400" />
          <div>
            <p className="text-sm font-bold text-amber-900 dark:text-amber-200">
              {t("settings.restartTitle")}
            </p>
            <p className="mt-1 text-sm leading-relaxed text-amber-800/85 dark:text-amber-200/75">
              {t("settings.restartBody")}
            </p>
            <code className="mt-2 inline-block rounded-md bg-amber-950/10 px-2 py-1 font-mono text-[11px] text-amber-900 dark:bg-black/20 dark:text-amber-200">
              docker compose restart backend
            </code>
          </div>
        </div>
      ) : null}

      <div className="mt-6 grid gap-5 xl:grid-cols-2">
        <SectionCard
          icon={<Database className="h-4 w-4" />}
          title={t("settings.section.connections")}
          description={t("settings.connectionsHint")}
        >
          <Field
            label={t("settings.databaseUrl")}
            hint={t("settings.databaseUrlHint")}
            className="sm:col-span-2"
          >
            <Input
              value={form.database_url}
              onChange={(event) => update("database_url", event.target.value)}
              className="font-mono text-xs"
              spellCheck={false}
            />
          </Field>
          <Field
            label={t("settings.testDatabaseUrl")}
            hint={t("settings.testDatabaseUrlHint")}
            className="sm:col-span-2"
          >
            <Input
              value={form.test_database_url}
              onChange={(event) => update("test_database_url", event.target.value)}
              className="font-mono text-xs"
              placeholder="postgres://…"
              spellCheck={false}
            />
          </Field>
          <Field
            label={t("settings.searxngUrl")}
            hint={t("settings.searxngUrlHint")}
            className="sm:col-span-2"
          >
            <div className="relative">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={form.searxng_url}
                onChange={(event) => update("searxng_url", event.target.value)}
                className="pl-9 font-mono text-xs"
                spellCheck={false}
              />
            </div>
          </Field>
          <Field
            label={t("settings.frontendOrigins")}
            hint={t("settings.frontendOriginsHint")}
            className="sm:col-span-2"
          >
            <Input
              value={form.frontend_origins}
              onChange={(event) => update("frontend_origins", event.target.value)}
              className="font-mono text-xs"
              spellCheck={false}
            />
          </Field>
        </SectionCard>

        <SectionCard
          icon={<BrainCircuit className="h-4 w-4" />}
          title={t("settings.section.ai")}
          description={t("settings.aiHint")}
          badge={
            <Badge variant={settings.llm_configured ? "success" : "warning"}>
              {settings.llm_configured
                ? t("settings.configured")
                : t("settings.notConfigured")}
            </Badge>
          }
        >
          <Field
            label={t("settings.baseUrl")}
            hint={t("settings.baseUrlHint")}
            className="sm:col-span-2"
          >
            <Input
              value={form.ninerouter_base_url}
              onChange={(event) =>
                update("ninerouter_base_url", event.target.value)
              }
              className="font-mono text-xs"
              spellCheck={false}
            />
          </Field>
          <Field label={t("settings.model")} hint={t("settings.modelHint")}>
            <Input
              value={form.ninerouter_model}
              onChange={(event) =>
                update("ninerouter_model", event.target.value)
              }
              className="font-mono text-xs"
              spellCheck={false}
            />
          </Field>
          <Field label={t("settings.confidence")} hint={t("settings.confidenceHint")}>
            <Input
              type="number"
              min={0}
              max={1}
              step={0.05}
              value={form.confidence_threshold}
              onChange={(event) =>
                update("confidence_threshold", event.target.value)
              }
            />
          </Field>
          <Field
            label={t("settings.apiKey")}
            hint={t("settings.apiKeyHint")}
            className="sm:col-span-2"
          >
            <SecretInput
              value={form.ninerouter_api_key}
              onChange={(value) => update("ninerouter_api_key", value)}
              configured={settings.ninerouter_api_key_set}
            />
          </Field>
        </SectionCard>

        <SectionCard
          icon={<Mail className="h-4 w-4" />}
          title={t("settings.section.email")}
          description={t("settings.emailHint")}
          badge={
            <Badge variant={settings.email_configured ? "success" : "warning"}>
              {settings.email_configured
                ? t("settings.emailReady")
                : t("settings.emailNotReady")}
            </Badge>
          }
        >
          <Field
            label={t("settings.resendKey")}
            hint={t("settings.resendKeyHint")}
            className="sm:col-span-2"
          >
            <SecretInput
              value={form.resend_api_key}
              onChange={(value) => update("resend_api_key", value)}
              configured={settings.resend_api_key_set}
            />
          </Field>
          <Field label={t("settings.emailFrom")} hint={t("settings.emailFromHint")}>
            <Input
              value={form.email_from}
              onChange={(event) => update("email_from", event.target.value)}
              spellCheck={false}
            />
          </Field>
          <Field label={t("settings.emailTo")} hint={t("settings.emailToHint")}>
            <Input
              value={form.email_to}
              onChange={(event) => update("email_to", event.target.value)}
              placeholder="you@example.com"
              spellCheck={false}
            />
          </Field>

          <div className="sm:col-span-2 flex items-center justify-between gap-4 rounded-xl border border-border/70 bg-muted/30 p-3.5">
            <div>
              <p className="text-sm font-semibold">{t("settings.autoSend")}</p>
              <p className="mt-0.5 text-xs text-muted-foreground">
                {t("settings.autoSendHint")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={form.auto_send_email}
              onClick={() => update("auto_send_email", !form.auto_send_email)}
              className={cn(
                "relative flex h-6 w-10 shrink-0 items-center rounded-full p-0.5 transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
                form.auto_send_email ? "bg-primary" : "bg-muted-foreground/30",
              )}
            >
              <span
                className={cn(
                  "h-5 w-5 rounded-full bg-white shadow transition-transform",
                  form.auto_send_email && "translate-x-4",
                )}
              />
            </button>
          </div>

          <div className="sm:col-span-2 rounded-xl border border-dashed border-border p-4">
            <div className="flex items-start gap-2">
              <Info className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
              <p className="text-xs leading-relaxed text-muted-foreground">
                {t("settings.testHint")}
              </p>
            </div>
            <div className="mt-3 flex flex-col gap-2 sm:flex-row">
              <Input
                value={form.test_recipient}
                onChange={(event) =>
                  update("test_recipient", event.target.value)
                }
                placeholder={t("settings.testRecipient")}
                className="sm:flex-1"
                spellCheck={false}
              />
              <Button
                type="button"
                variant="outline"
                onClick={handleTestEmail}
                disabled={saving || testing}
                className="gap-2"
              >
                {testing ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Send className="h-4 w-4" />
                )}
                {testing ? t("settings.testing") : t("settings.testEmail")}
              </Button>
            </div>
          </div>
        </SectionCard>

        <SectionCard
          icon={<CalendarClock className="h-4 w-4" />}
          title={t("settings.section.schedule")}
          description={t("settings.scheduleHint")}
          badge={
            <Badge variant="success" className="gap-1">
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-500" />
              {t("settings.scheduleLive")}
            </Badge>
          }
        >
          <Field label={t("settings.cron")} hint={t("settings.cronHint")}>
            <Input
              value={form.cron_schedule}
              onChange={(event) => update("cron_schedule", event.target.value)}
              className="font-mono text-sm"
              placeholder="0 0 8 * * *"
              spellCheck={false}
            />
          </Field>
          <Field label={t("settings.timezone")} hint={t("settings.timezoneHint")}>
            <Input
              value={form.cron_timezone}
              onChange={(event) => update("cron_timezone", event.target.value)}
              className="font-mono text-sm"
              list="settings-timezones"
              spellCheck={false}
            />
            <datalist id="settings-timezones">
              <option value="Asia/Shanghai" />
              <option value="Asia/Tokyo" />
              <option value="Asia/Singapore" />
              <option value="Europe/London" />
              <option value="America/New_York" />
              <option value="UTC" />
            </datalist>
          </Field>
          <div className="sm:col-span-2 flex items-start gap-2 rounded-xl border border-border/70 bg-muted/30 p-3.5 text-xs leading-relaxed text-muted-foreground">
            <CalendarClock className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
            <span>
              <strong className="text-foreground">{form.cron_timezone}</strong> ·{" "}
              <span className="font-mono">{form.cron_schedule}</span>
            </span>
          </div>
        </SectionCard>

        <SectionCard
          icon={<ShieldCheck className="h-4 w-4" />}
          title={t("settings.section.access")}
          description={t("settings.accessHint")}
        >
          <Field
            label={t("settings.dashboardUrl")}
            hint={t("settings.dashboardUrlHint")}
            className="sm:col-span-2"
          >
            <Input
              value={form.dashboard_url}
              onChange={(event) => update("dashboard_url", event.target.value)}
              className="font-mono text-xs"
              placeholder="http://localhost:3100"
              spellCheck={false}
            />
          </Field>
          <Field
            label={t("settings.appKey")}
            hint={t("settings.appKeyHint")}
            className="sm:col-span-2"
          >
            <SecretInput
              value={form.app_api_key}
              onChange={(value) => update("app_api_key", value)}
              configured={settings.app_api_key_set}
            />
          </Field>
          <div className="sm:col-span-2 flex items-start gap-2 rounded-xl border border-border/70 bg-muted/30 p-3.5 text-xs leading-relaxed text-muted-foreground">
            <Info className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
            {t("settings.securityNote")}
          </div>
        </SectionCard>
      </div>

      <p className="mt-4 truncate font-mono text-[10px] text-muted-foreground/70">
        {t("settings.file")}: {settings.settings_file}
      </p>

      <div className="fixed inset-x-0 bottom-0 z-30 border-t border-border/70 bg-background/85 px-4 py-3 backdrop-blur-xl sm:px-6">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-3">
          <p className="hidden items-center gap-1.5 text-xs text-muted-foreground sm:flex">
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
            {t("settings.databaseRestart")}
          </p>
          <Button
            type="submit"
            disabled={saving || testing}
            className="ml-auto min-w-40 gap-2"
          >
            {saving ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Save className="h-4 w-4" />
            )}
            {saving ? t("settings.saving") : t("settings.save")}
          </Button>
        </div>
      </div>
    </form>
  );
}

function SectionCard({
  icon,
  title,
  description,
  badge,
  children,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  badge?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Card className="overflow-hidden border-border/70 bg-card/85 shadow-soft">
      <div className="flex items-start justify-between gap-3 border-b border-border/60 bg-muted/20 px-5 py-4">
        <div className="flex items-start gap-3">
          <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            {icon}
          </span>
          <div>
            <h2 className="text-sm font-extrabold tracking-tight">{title}</h2>
            <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
              {description}
            </p>
          </div>
        </div>
        {badge}
      </div>
      <div className="grid gap-4 p-5 sm:grid-cols-2">{children}</div>
    </Card>
  );
}

function Field({
  label,
  hint,
  className,
  children,
}: {
  label: string;
  hint?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <label className={cn("block space-y-1.5", className)}>
      <span className="block text-[11px] font-bold uppercase tracking-[0.08em] text-muted-foreground">
        {label}
      </span>
      {children}
      {hint ? (
        <span className="block text-[11px] leading-relaxed text-muted-foreground/80">
          {hint}
        </span>
      ) : null}
    </label>
  );
}

function SecretInput({
  value,
  onChange,
  configured,
}: {
  value: string;
  onChange: (value: string) => void;
  configured: boolean;
}) {
  const { t } = useI18n();
  const [visible, setVisible] = useState(false);

  return (
    <div className="relative">
      <Input
        type={visible ? "text" : "password"}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={
          configured
            ? `••••••••  (${t("settings.configured")})`
            : t("settings.notConfigured")
        }
        className="pr-10 font-mono text-xs"
        spellCheck={false}
        autoComplete="new-password"
      />
      <button
        type="button"
        onClick={() => setVisible((current) => !current)}
        className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        aria-label={visible ? "Hide value" : "Show value"}
      >
        {visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
      </button>
    </div>
  );
}
