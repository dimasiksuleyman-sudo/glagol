import { t } from "@/i18n";
import { Button } from "@/components/ui/button";

interface UpdateReminderToastProps {
  onCheck: () => void;
  onEnable: () => void;
  onLater: () => void;
}

/**
 * Monthly update reminder rendered with `toast.custom`: the text spans the
 * full toast width and the buttons sit on their own row, because Sonner's
 * built-in action layout squeezes the text into a narrow column.
 */
export function UpdateReminderToast({ onCheck, onEnable, onLater }: UpdateReminderToastProps) {
  return (
    <div className="bg-popover text-popover-foreground w-[356px] max-w-[calc(100vw-2rem)] rounded-lg border p-4 shadow-lg">
      <p className="text-sm font-semibold">{t("Keep Glagol up to date")}</p>
      <p className="text-muted-foreground mt-1 text-sm">
        {t("Check for a new version now, or let Glagol check automatically at startup.")}
      </p>
      {/* Left-aligned, primary first — the same order as buttons in the app. */}
      <div className="mt-3 flex flex-wrap gap-2">
        <Button size="sm" onClick={onCheck}>
          {t("Check now")}
        </Button>
        <Button variant="outline" size="sm" onClick={onEnable}>
          {t("Enable auto-check")}
        </Button>
        <Button variant="ghost" size="sm" onClick={onLater}>
          {t("Later")}
        </Button>
      </div>
    </div>
  );
}
