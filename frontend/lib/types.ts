export type HackathonStatus = "new" | "notified" | "saved" | "dismissed";

export interface Hackathon {
  id: string;
  title: string;
  title_zh: string | null;
  organizer: string | null;
  description: string;
  location: string | null;
  is_online: boolean | null;
  start_date: string | null;
  end_date: string | null;
  registration_deadline: string | null;
  registration_url: string | null;
  source_url: string;
  prize_summary: string | null;
  prize_amount_cny: number | null;
  tracks_or_themes: string[];
  eligibility: string | null;
  tags: string[];
  confidence: number;
  status: HackathonStatus;
  first_seen_at: string;
  last_seen_at: string;
  created_at: string;
  updated_at: string;
}

export interface HackathonPage {
  items: Hackathon[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface HackathonFilters {
  status?: string;
  tag?: string;
  online?: boolean;
  location?: string;
  deadline_before?: string;
  deadline_after?: string;
  min_prize?: number;
  q?: string;
  page?: number;
  page_size?: number;
  sort?: string;
}

export interface RunLog {
  at: string;
  level: "info" | "warn" | "error" | string;
  message: string;
}

export interface RunError {
  at: string;
  message: string;
  context?: Record<string, unknown> | null;
}

export interface SearchRun {
  id: string;
  trigger: "manual" | "cron";
  status: "running" | "completed" | "failed";
  phase: string;
  progress: number;
  started_at: string;
  finished_at: string | null;
  queries_count: number;
  found_count: number;
  new_count: number;
  errors: RunError[];
  logs: RunLog[];
}

export interface RunAccepted {
  run_id: string;
  status: string;
}

export interface DigestResult {
  sent: boolean;
  count: number;
  resend_id: string | null;
  hackathon_ids: string[];
}

export interface SettingsView {
  database_url: string;
  test_database_url: string;
  searxng_url: string;
  ninerouter_base_url: string;
  ninerouter_model: string;
  ninerouter_api_key_set: boolean;
  app_api_key_set: boolean;
  resend_api_key_set: boolean;
  frontend_origins: string;
  dashboard_url: string;
  email_from: string;
  email_to: string;
  auto_send_email: boolean;
  confidence_threshold: number;
  cron_schedule: string;
  cron_timezone: string;
  email_configured: boolean;
  llm_configured: boolean;
  settings_file: string;
  restart_required: string[];
}

export interface SettingsUpdate {
  database_url?: string;
  test_database_url?: string;
  searxng_url?: string;
  ninerouter_base_url?: string;
  ninerouter_api_key?: string;
  ninerouter_model?: string;
  app_api_key?: string;
  frontend_origins?: string;
  dashboard_url?: string;
  resend_api_key?: string;
  email_from?: string;
  email_to?: string;
  auto_send_email?: boolean;
  confidence_threshold?: number;
  cron_schedule?: string;
  cron_timezone?: string;
}

export interface TestEmailResult {
  sent: boolean;
  resend_id: string | null;
  recipients: string[];
}
