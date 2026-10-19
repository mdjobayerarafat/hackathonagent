import type {
  DigestResult,
  Hackathon,
  HackathonFilters,
  HackathonPage,
  RunAccepted,
  SearchRun,
  SettingsUpdate,
  SettingsView,
  TestEmailResult,
} from "@/lib/types";

const API_URL = (process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080").replace(/\/$/, "");
const API_KEY = process.env.NEXT_PUBLIC_APP_API_KEY ?? "";

export class ApiError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

async function request<T>(
  path: string,
  init: RequestInit = {},
  signal?: AbortSignal,
): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("Accept", "application/json");
  if (init.body) headers.set("Content-Type", "application/json");
  if (API_KEY) headers.set("x-api-key", API_KEY);

  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      ...init,
      headers,
      signal,
      cache: "no-store",
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") throw error;
    throw new ApiError(
      0,
      `Cannot reach the backend at ${API_URL}. Is it running?`,
    );
  }

  if (!response.ok) {
    let message = `Request failed with status ${response.status}`;
    try {
      const body = (await response.json()) as { error?: string };
      if (body.error) message = body.error;
    } catch {
      // The backend may return an empty body for proxy-level errors.
    }
    if (response.status === 401) {
      message = "Missing or invalid API key. Check NEXT_PUBLIC_APP_API_KEY.";
    }
    throw new ApiError(response.status, message);
  }

  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

function toSearchParams(filters: HackathonFilters): string {
  const params = new URLSearchParams();
  if (filters.status && filters.status !== "all") params.set("status", filters.status);
  if (filters.tag) params.set("tag", filters.tag);
  if (filters.online !== undefined) params.set("online", String(filters.online));
  if (filters.location) params.set("location", filters.location);
  if (filters.deadline_before) params.set("deadline_before", filters.deadline_before);
  if (filters.deadline_after) params.set("deadline_after", filters.deadline_after);
  if (filters.min_prize !== undefined) params.set("min_prize", String(filters.min_prize));
  if (filters.q) params.set("q", filters.q);
  if (filters.page) params.set("page", String(filters.page));
  if (filters.page_size) params.set("page_size", String(filters.page_size));
  if (filters.sort) params.set("sort", filters.sort);
  return params.toString();
}

export const api = {
  listHackathons(filters: HackathonFilters, signal?: AbortSignal) {
    const query = toSearchParams(filters);
    return request<HackathonPage>(`/api/hackathons${query ? `?${query}` : ""}`, {}, signal);
  },

  getHackathon(id: string, signal?: AbortSignal) {
    return request<Hackathon>(`/api/hackathons/${id}`, {}, signal);
  },

  updateStatus(id: string, status: "saved" | "dismissed") {
    return request<Hackathon>(`/api/hackathons/${id}`, {
      method: "PATCH",
      body: JSON.stringify({ status }),
    });
  },

  triggerRun() {
    return request<RunAccepted>("/api/agent/run", { method: "POST" });
  },

  listRuns(limit = 30, signal?: AbortSignal) {
    return request<SearchRun[]>(`/api/agent/runs?limit=${limit}`, {}, signal);
  },

  getRun(id: string, signal?: AbortSignal) {
    return request<SearchRun>(`/api/agent/runs/${id}`, {}, signal);
  },

  sendDigest() {
    return request<DigestResult>("/api/email/digest", { method: "POST" });
  },

  getSettings(signal?: AbortSignal) {
    return request<SettingsView>("/api/settings", {}, signal);
  },

  updateSettings(settings: SettingsUpdate) {
    return request<SettingsView>("/api/settings", {
      method: "PUT",
      body: JSON.stringify(settings),
    });
  },

  testEmail(to?: string) {
    return request<TestEmailResult>("/api/email/test", {
      method: "POST",
      body: JSON.stringify({ to: to?.trim() || null }),
    });
  },

  health(signal?: AbortSignal) {
    return request<{ status: string; database: string; version: string }>(
      "/api/health",
      {},
      signal,
    );
  },
};

export { API_URL };
