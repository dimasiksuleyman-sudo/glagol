import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { NavLink, Outlet } from "react-router-dom";
import { AudioLines, Library, Mic, Settings } from "lucide-react";

import { cn } from "@/lib/utils";
import { Separator } from "@/components/ui/separator";
import { Toaster } from "@/components/ui/sonner";
import { LanguageSwitch } from "@/components/LanguageSwitch";
import { usePreferences } from "@/contexts/PreferencesContext";

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
  return (
    <div className="bg-background text-foreground flex min-h-screen">
      <aside className="bg-sidebar text-sidebar-foreground border-sidebar-border flex w-60 shrink-0 flex-col border-r">
        <div className="px-6 py-5">
          <h1 className="text-xl font-semibold tracking-tight">Glagol</h1>
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
