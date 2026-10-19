import type { Metadata } from "next";

import { RunsView } from "@/components/runs-view";

export const metadata: Metadata = {
  title: "Agent runs",
  description: "Live progress, stats and activity logs for every Hackathon Hunter run.",
};

export default function RunsPage() {
  return <RunsView />;
}
