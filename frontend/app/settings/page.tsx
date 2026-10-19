import type { Metadata } from "next";

import { SettingsForm } from "@/components/settings-form";

export const metadata: Metadata = {
  title: "Settings",
  description: "Configure Hackathon Hunter connections, AI extraction, email and schedule.",
};

export default function SettingsPage() {
  return <SettingsForm />;
}
