import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useI18n } from "@/contexts/PreferencesContext";
import { t } from "@/i18n";
import {
  getAiNormalizationSettings,
  listAiModels,
  saveAiNormalizationSettings,
  testAiNormalization,
  type AiNormalizationSettings,
  type AiNormalizationSettingsView,
} from "@/lib/tauri";

/** Quick-fill addresses; any OpenAI-compatible service works. */
const QUICK_FILL = [
  { label: "RouterAI", url: "https://routerai.ru/api/v1" },
  { label: "AITunnel", url: "https://api.aitunnel.ru/v1" },
];
const trimUrl = (url: string) => url.trim().replace(/\/+$/, "");

/** Optional sending of short excerpts for document narration: address + key, model picked from the service's list. */
export function AiNormalizationSection() {
  useI18n();
  const [saved, setSaved] = useState<AiNormalizationSettingsView | null>(null);
  const [draft, setDraft] = useState<AiNormalizationSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState<null | "models" | "save" | "test">(null);
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

  async function run(kind: "models" | "save" | "test", action: () => Promise<void>) {
    setBusy(kind);
    try {
      await action();
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy(null);
    }
  }

  const endpointChanged = !!draft && trimUrl(draft.base_url) !== saved?.settings.base_url;
  const keyAvailable = apiKey.trim() !== "" || (!!saved?.has_key && !endpointChanged);
  const dirty = !!draft && (JSON.stringify({ ...draft, base_url: trimUrl(draft.base_url) }) !== JSON.stringify(saved?.settings) || apiKey.trim() !== "");
  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const list = q ? models.filter(m => m.toLowerCase().includes(q)) : models;
    return draft?.model && !list.includes(draft.model) ? [draft.model, ...list] : list;
  }, [models, filter, draft?.model]);

  async function loadModels() {
    if (!draft) return;
    const result = await listAiModels(draft.base_url, apiKey.trim() || undefined);
    setModels(result.models);
    setFilter("");
    const model = result.models.includes(draft.model) ? draft.model : result.suggested ?? "";
    setDraft({ ...draft, model });
    toast.success(t("aiModelsLoaded", { count: result.models.length, model: model || "—" }));
  }

  async function save(settings: AiNormalizationSettings, key?: string) {
    const view = await saveAiNormalizationSettings(settings, key);
    setSaved(view);
    setDraft(view.settings);
    setApiKey("");
    toast.success(settings.enabled ? t("aiCheckedAndSaved") : t("AI settings saved"));
  }

  const disabled = busy !== null;
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
            void run("save", () => save(draft, apiKey.trim() || undefined));
          }}>
            <div className="flex items-center gap-2">
              <Switch id="ai-enabled" checked={draft.enabled} disabled={disabled} aria-describedby="ai-disclosure"
                onCheckedChange={enabled => setDraft({ ...draft, enabled })} />
              <Label htmlFor="ai-enabled">{t("Use AI for document narration")}</Label>
            </div>

            <div className="space-y-2">
              <Label htmlFor="ai-endpoint">{t("AI service address")}</Label>
              <Input id="ai-endpoint" type="url" maxLength={256} value={draft.base_url}
                placeholder="https://…/v1" disabled={disabled}
                onChange={event => { setDraft({ ...draft, base_url: event.target.value }); setModels([]); }} />
              <p className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                {t("aiAddressHint")}
                {QUICK_FILL.map(item => (
                  <button key={item.url} type="button" disabled={disabled}
                    className="rounded border px-2 py-0.5 text-foreground hover:bg-muted disabled:opacity-50"
                    onClick={() => { setDraft({ ...draft, base_url: item.url }); setModels([]); }}>{item.label}</button>
                ))}
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="ai-key">{t("AI API key")}</Label>
              <Input id="ai-key" type="password" autoComplete="new-password" maxLength={512} value={apiKey}
                placeholder={saved?.has_key && !endpointChanged ? t("Saved in your system credential store") : undefined}
                disabled={disabled} onChange={event => setApiKey(event.target.value)} />
            </div>

            <div className="space-y-2">
              <div className="flex flex-wrap items-end gap-2">
                <div className="min-w-0 flex-1 space-y-2">
                  <Label htmlFor="ai-model">{t("AI model")}</Label>
                  {models.length > 0 ? (
                    <>
                      {models.length > 15 && <Input value={filter} placeholder={t("Filter models")} disabled={disabled}
                        onChange={event => setFilter(event.target.value)} />}
                      <select id="ai-model" className="w-full rounded border bg-background p-2 text-sm" value={draft.model} disabled={disabled}
                        onChange={event => setDraft({ ...draft, model: event.target.value })}>
                        {shown.map(model => <option key={model} value={model}>{model}</option>)}
                      </select>
                    </>
                  ) : (
                    <Input id="ai-model" maxLength={128} value={draft.model} disabled={disabled}
                      placeholder="google/gemini-3.5-flash-lite"
                      onChange={event => setDraft({ ...draft, model: event.target.value })} />
                  )}
                </div>
                <Button type="button" variant="outline" disabled={disabled || !draft.base_url.trim() || !keyAvailable}
                  onClick={() => void run("models", loadModels)}>
                  {busy === "models" ? t("Checking the connection…") : t("Load models")}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">{t("aiModelHint")}</p>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button type="submit" disabled={disabled || !dirty || !draft.base_url.trim() || !draft.model.trim()}>
                {busy === "save" ? t("Checking the connection…") : t("Save AI settings")}
              </Button>
              <Button type="button" variant="outline" disabled={disabled || dirty || !saved?.has_key}
                onClick={() => void run("test", async () => {
                  await testAiNormalization();
                  toast.success(t("AI connection works"));
                })}>{busy === "test" ? t("Checking the connection…") : t("Test AI connection")}</Button>
              {saved?.has_key && (
                <Button type="button" variant="ghost" disabled={disabled || dirty}
                  onClick={() => void run("save", () => save({ ...draft, enabled: false }, ""))}>
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
