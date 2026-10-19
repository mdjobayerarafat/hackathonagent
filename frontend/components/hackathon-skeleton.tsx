import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export function HackathonSkeletonGrid({ count = 6 }: { count?: number }) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {Array.from({ length: count }).map((_, index) => (
        <Card key={index} className="h-full border-border/60">
          <CardHeader className="p-5 pb-3">
            <div className="flex items-center justify-between gap-2">
              <Skeleton className="h-5 w-24 rounded-full" />
              <Skeleton className="h-5 w-12 rounded-full" />
            </div>
            <Skeleton className="mt-4 h-5 w-4/5" />
            <Skeleton className="mt-2 h-4 w-1/3" />
          </CardHeader>
          <CardContent className="space-y-3 p-5 pt-0">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-11/12" />
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-8 w-32 rounded-lg" />
            <div className="flex gap-2 pt-2">
              <Skeleton className="h-10 flex-1 rounded-lg" />
              <Skeleton className="h-10 w-10 rounded-lg" />
              <Skeleton className="h-10 w-10 rounded-lg" />
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
