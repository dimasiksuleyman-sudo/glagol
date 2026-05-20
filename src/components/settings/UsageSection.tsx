import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TriangleAlert } from "lucide-react";

import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getCurrentMonthUsage,
  SYNTHESIS_COMPLETED_EVENT,
  type SynthesisCompletedEvent,
  type UsageInfo,
} from "@/lib/tauri";

/**
 * Russian month name in the form used after the preposition «в»
 * ("в мае", "в июне"). Mirrors `commands::usage::russian_month_genitive`
 * on the backend; kept in sync by hand so the frontend doesn't have
 * to round-trip an extra IPC call just to render the section title.
 *
 * Falls back to «этом месяце» ("this month") for an unparseable input,
 * which lets the section render usefully even if the backend ever
 * starts emitting a malformed `YYYY-MM` string.
 */
function monthNameAfterV(month: string): string {
  const part = month.split("-")[1];
  const num = part ? Number.parseInt(part, 10) : Number.NaN;
  const names = [
    "январе",
    "феврале",
    "марте",
    "апреле",
    "мае",
    "июне",
    "июле",
    "августе",
    "сентябре",
    "октябре",
    "ноябре",
    "декабре",
  ];
  if (Number.isFinite(num) && num >= 1 && num <= 12) {
    return names[num - 1];
  }
  return "этом месяце";
}

const numberFormatter = new Intl.NumberFormat("ru-RU");

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; usage: UsageInfo }
  | { kind: "error"; message: string };

export function UsageSection() {
  const [state, setState] = useState<LoadState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;

    async function refresh() {
      try {
        const info = await getCurrentMonthUsage();
        if (!cancelled) {
          setState({ kind: "ready", usage: info });
        }
      } catch (err) {
        if (!cancelled) {
          setState({ kind: "error", message: stringifyError(err) });
        }
      }
    }

    refresh();

    // Re-fetch authoritative usage on every successful synthesis. The
    // payload includes `charsAdded` but we deliberately ignore it and
    // re-query — the backend has the canonical value, and we'd rather
    // pay 5ms for a fresh read than risk a drift bug if the UPSERT
    // ever changes semantics.
    const unlisten = listen<SynthesisCompletedEvent>(SYNTHESIS_COMPLETED_EVENT, () => {
      void refresh();
    });

    return () => {
      cancelled = true;
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Использование SaluteSpeech</CardTitle>
        <CardDescription>
          Бесплатный тариф — 200 000 символов синтеза в месяц.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">{renderBody(state)}</CardContent>
      <CardFooter>
        <p className="text-muted-foreground text-xs">
          Счётчик сбрасывается 1-го числа каждого месяца.
        </p>
      </CardFooter>
    </Card>
  );
}

function renderBody(state: LoadState) {
  switch (state.kind) {
    case "loading":
      return (
        <>
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-1 w-full" />
          <Skeleton className="h-3 w-12" />
        </>
      );
    case "error":
      return (
        <div className="text-muted-foreground flex items-start gap-2 text-sm">
          <TriangleAlert className="text-foreground mt-0.5 size-4 shrink-0" />
          <p>Не удалось загрузить счётчик использования: {state.message}</p>
        </div>
      );
    case "ready": {
      const { usage } = state;
      return (
        <>
          <p className="text-sm">
            Использовано в {monthNameAfterV(usage.month)}:{" "}
            <strong className="text-foreground">
              {numberFormatter.format(usage.chars_used)}
            </strong>{" "}
            / {numberFormatter.format(usage.chars_limit)} символов
          </p>
          <Progress value={usage.percent_used} />
          <p className="text-muted-foreground text-xs">
            {usage.percent_used.toFixed(1).replace(".", ",")}%
          </p>
        </>
      );
    }
  }
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
