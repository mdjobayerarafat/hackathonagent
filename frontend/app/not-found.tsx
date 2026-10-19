import { Compass, Sparkles } from "lucide-react";
import Link from "next/link";

import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center text-center">
      <span className="flex h-14 w-14 items-center justify-center rounded-2xl bg-primary/10 text-primary">
        <Compass className="h-6 w-6" />
      </span>
      <p className="mt-5 text-xs font-extrabold uppercase tracking-[0.2em] text-primary">
        404
      </p>
      <h1 className="mt-2 font-display text-3xl font-extrabold tracking-tight">
        This page went exploring.
      </h1>
      <p className="mt-2 max-w-sm text-sm text-muted-foreground">
        It never came back. The hackathons, however, are right where you left them.
      </p>
      <Button asChild className="mt-6">
        <Link href="/">
          <Sparkles className="h-4 w-4" />
          Back to the dashboard
        </Link>
      </Button>
    </div>
  );
}
