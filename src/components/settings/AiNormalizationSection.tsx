import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useI18n } from "@/contexts/PreferencesContext";
import { t } from "@/i18n";
import {
  getAiNormalizationSettings,
  saveAiNormalizationSettings,
  testAiNormalization,
  type AiNormalizationProvider,
  type AiNormalizationSettings,
  type AiNormalizationSettingsView,
} from "@/lib/tauri";

const PRESETS: Record<AiNormalizationProvider, Pick<AiNormalizationSettings, "base_url" | "model">> = {
  router_ai: { base_url: "https://routerai.ru/api/v1", model: "google/gemini-3.5-flash-lite" },
  ai_tunnel: { base_url: "https://api.aitunnel.ru/v1", model: "gemini-3.5-flash-lite" },
  custom: { base_url: "", model: "" },
};

/** Optional sending of short excerpts for document narration. */
export function AiNormalizationSection() {
  useI18n();
  const [saved, setSaved] = useState<AiNormalizationSettingsView | null>(null);
  const [draft, setDraft] = useState<AiNormalizationSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let ignore = false;
    void getAiNormalizationSettings().then(
      view => {
        if (ignore) return;
        setSaved(view);
        setDraft(view.settings);
      },
      error => { if (!ignore) setLoadError(String(error)); },
    );
    return () => { ignore = true; };
  }, []);

  async function run(action: () => Promise<void>) {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function save(settings: AiNormalizationSettings, key?: string) {
    const view = await saveAiNormalizationSettings(settings, key);
    setSaved(view);
    setDraft(view.settings);
    setApiKey("");
    toast.success(t("AI settings saved"));
  }

  const endpointChanged = draft?.base_url.trim().replace(/\/+$/, "") !== saved?.settings.base_url;
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved?.settings) || apiKey.trim() !== "";

  return (
    <Card id="ai-normalization">
      <CardHeader>
        <CardTitle>{t("Improve number reading with AI")}</CardTitle>
        <CardDescription id="ai-disclosure">{t("aiExcerptDisclosure")}</CardDescription>
      </CardHeader>
      <CardContent>
        {loadError && <p role="alert" className="text-sm text-destructive">{loadError}</p>}
        {draft && (
          <form className="space-y-4" onSubmit={event => {
            event.preventDefault();
            void run(() => save(draft, apiKey.trim() || undefined));
          }}>
            <div className="flex items-center gap-2">
              <Switch
                id="ai-enabled"
                checked={draft.enabled}
                disabled={busy}
                aria-describedby="ai-disclosure"
                onCheckedChange={enabled => setDraft({ ...draft, enabled })}
              />
              <Label htmlFor="ai-enabled">{t("Use AI for document narration")}</Label>
            </div>

            <div className="space-y-2">
              <Label htmlFor="ai-provider">{t("AI provider")}</Label>
              <Select value={draft.provider} disabled={busy} onValueChange={(provider: AiNormalizationProvider) => {
                setDraft({ ...draft, ...PRESETS[provider], provider });
                setApiKey("");
              }}>
                <SelectTrigger id="ai-provider" className="w-full"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="router_ai">RouterAI</SelectItem>
                  <SelectItem value="ai_tunnel">AITunnel</SelectItem>
                  <SelectItem value="custom">{t("Custom endpoint")}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {draft.provider === "custom" && (
              <div className="space-y-2">
                <Label htmlFor="ai-endpoint">{t("HTTPS endpoint")}</Label>
                <Input id="ai-endpoint" type="url" required maxLength={256} value={draft.base_url}
                  placeholder="https://example.com/v1" disabled={busy}
                  onChange={event => setDraft({ ...draft, base_url: event.target.value })} />
              </div>
            )}

            <div className="space-y-2">
              <Label htmlFor="ai-model">{t("AI model")}</Label>
              <Input id="ai-model" required maxLength={128} value={draft.model} disabled={busy}
                onChange={event => setDraft({ ...draft, model: event.target.value })} />
            </div>

            <div className="space-y-2">
              <Label htmlFor="ai-key">{t("AI API key")}</Label>
              <Input id="ai-key" type="password" autoComplete="new-password" maxLength={512} value={apiKey}
                placeholder={saved?.has_key && !endpointChanged ? t("Saved in your system credential store") : undefined}
                disabled={busy} onChange={event => setApiKey(event.target.value)} />
            </div>

            <div className="flex flex-wrap gap-2">
              <Button type="submit" disabled={busy || !dirty}>{t("Save AI settings")}</Button>
              <Button type="button" variant="outline" disabled={busy || dirty || !saved?.has_key}
                onClick={() => void run(async () => {
                  await testAiNormalization();
                  toast.success(t("AI connection works"));
                })}>{t("Test AI connection")}</Button>
              {saved?.has_key && (
                <Button type="button" variant="ghost" disabled={busy || dirty}
                  onClick={() => void run(() => save({ ...draft, enabled: false }, ""))}>
                  {t("Forget AI key")}
                </Button>
              )}
            </div>
          </form>
        )}
      </CardContent>
    </Card>
  );
}
