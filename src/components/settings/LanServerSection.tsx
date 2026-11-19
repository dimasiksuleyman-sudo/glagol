import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Copy, Eye, EyeOff, KeyRound } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  getLanServer,
  lanServerConnectionCode,
  regenerateLanServerKey,
  setLanServerEnabled,
  setLanServerKeepAwake,
  setLanServerPort,
  type LanServerInfo,
} from "@/lib/tauri";

/** How often the counters refresh while the server runs. */
const REFRESH_MS = 5000;

function firewallCommand(port: number): string {
  return `New-NetFirewallRule -DisplayName "Glagol server" -Direction Inbound -Protocol TCP -LocalPort ${port} -Profile Private -Action Allow`;
}

/**
 * Settings → Dictation → «Server for other computers» (GL-010): this computer
 * serves Russian GigaAM recognition to Glagol on office computers.
 */
export function LanServerSection() {
  useI18n();
  const [info, setInfo] = useState<LanServerInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [code, setCode] = useState<string | null>(null);
  const [port, setPort] = useState("");
  const [confirmNewKey, setConfirmNewKey] = useState(false);

  useEffect(() => {
    getLanServer().then((next) => { setInfo(next); setPort(String(next.port)); }).catch((e) => toast.error(String(e)));
  }, []);

  const running = info?.running ?? false;
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => { getLanServer().then(setInfo).catch(() => undefined); }, REFRESH_MS);
    return () => clearInterval(timer);
  }, [running]);

  async function run(action: () => Promise<LanServerInfo>) {
    setBusy(true);
    try {
      const next = await action();
      setInfo(next);
      setPort(String(next.port));
    } catch (e) {
      toast.error(String(e));
      getLanServer().then(setInfo).catch(() => undefined);
    } finally {
      setBusy(false);
    }
  }

  async function copyCode() {
    try {
      const value = code ?? await lanServerConnectionCode();
      await navigator.clipboard.writeText(value);
      toast.success(t("Connection code copied."));
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function toggleCode() {
    if (code) { setCode(null); return; }
    try { setCode(await lanServerConnectionCode()); } catch (e) { toast.error(String(e)); }
  }

  async function copyFirewall() {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(firewallCommand(info.port));
      toast.success(t("Command copied."));
    } catch {
      toast.error(t("Could not copy to clipboard."));
    }
  }

  if (!info || !info.supported) return null;
  const status = info.status;
  const canEnable = info.model_ready || info.enabled;

  return (
    <Card data-tour="lan-server">
      <CardHeader>
        <CardTitle>{t("Server for other computers")}</CardTitle>
        <CardDescription>
          {t("Let Glagol on office computers dictate in Russian through this computer. Speech stays in your local network; an access key is always required.")}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-0.5">
            <label htmlFor="lan-server-toggle" className="text-sm font-medium">{t("Serve recognition over the network")}</label>
            <p className="text-muted-foreground text-xs">
              {info.model_name
                ? t("Uses this computer's Russian model {p0} on the device chosen above.", { p0: info.model_name })
                : t("Uses this computer's Russian model on the device chosen above.")}
            </p>
          </div>
          <Switch
            id="lan-server-toggle"
            checked={info.enabled}
            onCheckedChange={(enabled) => void run(() => setLanServerEnabled(enabled))}
            disabled={busy || !canEnable}
          />
        </div>

        {!info.model_ready && (
          <p className="text-sm text-amber-600 dark:text-amber-400">
            {t("Install a Russian GigaAM model first (Manage models above).")}
          </p>
        )}
        {info.error && <p role="alert" className="text-sm text-destructive">{info.error}</p>}

        {info.running && status && (
          <div className="space-y-3 rounded-lg border p-3">
            <p className="text-sm">
              <span className="font-medium">{t("Running")}</span>
              {" · "}
              {info.address ? `${info.address}:${status.port}` : t("no local network address")}
              {info.model_name ? ` · ${info.model_name}` : ""}
            </p>
            <p className="text-muted-foreground text-xs">
              {t("Computers in the last 10 minutes: {p0} · requests: {p1} · errors: {p2} · in queue: {p3}", {
                p0: String(status.active_clients),
                p1: String(status.requests),
                p2: String(status.failures),
                p3: String(status.pending),
              })}
            </p>
            {info.address && (
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("Connection code")}</p>
                <p className="text-muted-foreground text-xs">
                  {t("On the other computer: Settings → Dictation → Organization server → Paste connection code. The code contains the access key — share it only with your colleagues.")}
                </p>
                {code && <Input readOnly value={code} className="font-mono text-xs" onFocus={(e) => e.currentTarget.select()} />}
                <div className="flex flex-wrap gap-2">
                  <Button size="sm" onClick={() => void copyCode()}><Copy className="mr-1 h-4 w-4" />{t("Copy connection code")}</Button>
                  <Button size="sm" variant="outline" onClick={() => void toggleCode()}>
                    {code ? <EyeOff className="mr-1 h-4 w-4" /> : <Eye className="mr-1 h-4 w-4" />}
                    {code ? t("Hide") : t("Show")}
                  </Button>
                  {!confirmNewKey && (
                    <Button size="sm" variant="outline" disabled={busy} onClick={() => setConfirmNewKey(true)}>
                      <KeyRound className="mr-1 h-4 w-4" />{t("New key")}
                    </Button>
                  )}
                </div>
                {confirmNewKey && (
                  <div className="space-y-2 rounded-md border border-amber-500/50 p-2 text-sm">
                    <p>{t("Computers using the old code will stop dictating until they get the new code. Create a new key?")}</p>
                    <div className="flex gap-2">
                      <Button size="sm" variant="destructive" disabled={busy}
                        onClick={() => { setConfirmNewKey(false); setCode(null); void run(regenerateLanServerKey); }}>
                        {t("Create new key")}
                      </Button>
                      <Button size="sm" variant="outline" onClick={() => setConfirmNewKey(false)}>{t("Cancel")}</Button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {info.enabled && (
          <details className="text-sm">
            <summary className="cursor-pointer text-muted-foreground">{t("Computers cannot connect?")}</summary>
            <div className="mt-2 space-y-2 text-xs text-muted-foreground">
              <p>{t("When the server first starts, Windows asks whether Glagol may accept connections: allow private networks. If you missed it, run this command in PowerShell as administrator:")}</p>
              <code className="block break-all rounded bg-muted p-2 font-mono">{firewallCommand(info.port)}</code>
              <Button size="sm" variant="outline" onClick={() => void copyFirewall()}><Copy className="mr-1 h-4 w-4" />{t("Copy command")}</Button>
              <p>{t("Both computers must be in the same local network, and this computer must not be asleep.")}</p>
            </div>
          </details>
        )}

        <details className="text-sm">
          <summary className="cursor-pointer text-muted-foreground">{t("Advanced")}</summary>
          <div className="mt-3 space-y-3">
            <div className="flex items-end gap-2">
              <div className="space-y-1">
                <label htmlFor="lan-server-port" className="text-xs">{t("Port")}</label>
                <Input id="lan-server-port" inputMode="numeric" className="w-28" value={port}
                  onChange={(e) => setPort(e.target.value.replace(/\D/g, "").slice(0, 5))} />
              </div>
              <Button size="sm" variant="outline" disabled={busy || port === String(info.port) || !port}
                onClick={() => void run(() => setLanServerPort(Number(port)))}>{t("Save")}</Button>
            </div>
            <div className="flex items-start justify-between gap-4">
              <div className="space-y-0.5">
                <label htmlFor="lan-server-awake" className="text-sm">{t("Keep this computer awake while the server is on")}</label>
                <p className="text-muted-foreground text-xs">{t("The screen may still turn off; only sleep is prevented.")}</p>
              </div>
              <Switch id="lan-server-awake" checked={info.keep_awake} disabled={busy}
                onCheckedChange={(enabled) => void run(() => setLanServerKeepAwake(enabled))} />
            </div>
          </div>
        </details>
      </CardContent>
    </Card>
  );
}
