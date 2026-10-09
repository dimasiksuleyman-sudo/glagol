import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { NavLink, Outlet, useNavigate } from "react-router-dom";
import { AudioLines, ExternalLink, Library, Mic, Settings } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { cn } from "@/lib/utils";
import { Separator } from "@/components/ui/separator";
import { Toaster } from "@/components/ui/sonner";
import { LanguageSwitch } from "@/components/LanguageSwitch";
import { UpdateReminderToast } from "@/components/UpdateReminderToast";
import { usePreferences } from "@/contexts/PreferencesContext";
import { getUpdateSettings } from "@/lib/tauri";
import { changeAutoCheck, runStartupUpdateCheck, runUpdateCheck } from "@/lib/updates";

/** The author's GitHub profile, opened in the default browser. */
const AUTHOR_GITHUB_URL = "https://github.com/dimasiksuleyman-sudo";

interface NavItem {
  to: string;
  label: string;
  Icon: typeof Settings;
}

const navItems = (): readonly NavItem[] => [
  { to: "/synthesize", label: t("Synthesize"), Icon: AudioLines },
  { to: "/library", label: t("Library"), Icon: Library },
  { to: "/dictation", label: t("Dictation"), Icon: Mic },
  { to: "/settings", label: t("Settings"), Icon: Settings },
];

/**
 * App-wide layout: a fixed sidebar with three navigation entries and
 * a scrollable main area where the active route renders via `<Outlet />`.
 *
 * `<Toaster />` is mounted here so toasts triggered from any page land
 * in a single, app-wide stack.
 */
export function AppShell() {
  useI18n();
  const { error } = usePreferences();
  const navigate = useNavigate();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getUpdateSettings().then((settings) => setVersion(settings.currentVersion)).catch(() => {});
    const showUpdate = (version: string) =>
      toast(t("Version {version} is available.", { version }), {
        action: { label: t("Show"), onClick: () => navigate("/settings") },
        duration: 15000,
      });
    void runStartupUpdateCheck().then((result) => {
      if (result?.kind === "update") {
        showUpdate(result.update.version);
      } else if (result?.kind === "reminder") {
        toast.custom(
          (id) => (
            <UpdateReminderToast
              onLater={() => toast.dismiss(id)}
              onEnable={() => {
                toast.dismiss(id);
                changeAutoCheck(true)
                  .then(() => toast.success(t("Automatic update check is on.")))
                  .catch((e) => toast.error(String(e)));
              }}
              onCheck={() => {
                toast.dismiss(id);
                runUpdateCheck()
                  .then((update) =>
                    update ? showUpdate(update.version) : toast.success(t("You have the latest version.")),
                  )
                  .catch((e) => toast.error(String(e)));
              }}
            />
          ),
          // Stays until the user answers: it appears at most once a month.
          { duration: Infinity },
        );
      }
    });
  }, [navigate]);
  return (
    <div className="bg-background text-foreground flex min-h-screen">
      <aside className="bg-sidebar text-sidebar-foreground border-sidebar-border flex w-60 shrink-0 flex-col border-r">
        <div className="px-6 py-5">
          <h1 className="text-xl font-semibold tracking-tight">
            Glagol
            {version && (
              <span className="text-muted-foreground ml-2 text-xs font-normal">
                v{version}
              </span>
            )}
          </h1>
          <p className="text-muted-foreground mt-1 text-xs">
            {t("Text to speech and dictation")}{" "}</p>
        </div>
        <Separator />
        <nav className="flex flex-1 flex-col gap-1 p-3">
          {navItems().map(({ to, label, Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                  isActive
                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                    : "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground",
                )
              }
            >
              <Icon className="h-4 w-4" aria-hidden />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="px-3">
          <button
            type="button"
            title={t("Author's GitHub profile")}
            onClick={() => {
              openUrl(AUTHOR_GITHUB_URL).catch(() => toast.error(t("Could not open the link")));
            }}
            className="text-muted-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors"
          >
            <ExternalLink className="h-4 w-4" aria-hidden />
            <span>GitHub</span>
          </button>
        </div>
        <div className="p-3"><LanguageSwitch /></div>
      </aside>

      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-2xl px-8 py-10">
          {error && <p role="alert" className="mb-4 text-destructive">{error}</p>}
          <Outlet />
        </div>
      </main>

      <Toaster richColors position="top-right" />
    </div>
  );
}
