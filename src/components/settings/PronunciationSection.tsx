import { t } from "@/i18n";
import { activeVoice } from "@/lib/voices";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useEffect, useMemo, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Pencil, Play, Trash2 } from "lucide-react";

import { AudioPlayer } from "@/components/AudioPlayer";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useTts } from "@/contexts/TtsContext";
import { previewTts } from "@/lib/tts";
import {
  deletePronunciation,
  exportPronunciations,
  importPronunciations,
  listPronunciations,
  previewNormalized,
  savePronunciation,
  type PronunciationEntry,
} from "@/lib/tauri";

const EMPTY: PronunciationEntry = { id: null, pattern: "", replacement: "", language: "any", whole_word: true, case_sensitive: false, enabled: true };

/** Settings → Text to speech → Pronunciation dictionary (tts-normalization N2). */
export function PronunciationSection() {
  useI18n();
  const { preferences } = usePreferences();
  const { status, provider } = useTts();
  const speechLanguage = preferences?.tts_language ?? "ru";
  const voice = activeVoice(preferences, speechLanguage);
  const ready = !!status?.installed && !!status?.accepted;
  const [entries, setEntries] = useState<PronunciationEntry[]>([]);
  const [draft, setDraft] = useState<PronunciationEntry>(EMPTY);
  const [filter, setFilter] = useState("");
  const [phrase, setPhrase] = useState("");
  const [prepared, setPrepared] = useState<string | null>(null);
  const [audio, setAudio] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = () => listPronunciations().then(setEntries).catch(e => toast.error(String(e)));
  useEffect(() => { void reload(); }, []);
  useEffect(() => { setPrepared(null); setAudio(null); }, [phrase, speechLanguage]);

  async function run(fn: () => Promise<unknown>) {
    setBusy(true);
    try { await fn(); } catch (e) { toast.error(String(e)); } finally { setBusy(false); }
  }
  const shown = useMemo(() => {
    const q = filter.trim().toLocaleLowerCase();
    return q ? entries.filter(e => e.pattern.toLocaleLowerCase().includes(q) || e.replacement.toLocaleLowerCase().includes(q)) : entries;
  }, [entries, filter]);
  const listen = (text: string) => run(async () => { setAudio(null); const path = await previewTts(voice, provider, text); setAudio(convertFileSrc(path) + "?v=" + Date.now()); });
  const languageLabel = (l: PronunciationEntry["language"]) => l === "ru" ? t("Russian") : l === "en" ? t("English") : t("Any language");

  return <Card id="pronunciation" data-tour="pronunciation">
    <CardHeader>
      <CardTitle>{t("Pronunciation dictionary")}</CardTitle>
      <CardDescription>{t("pronDescription")}</CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      <div className="space-y-2">
        <label htmlFor="pron-phrase" className="text-sm font-medium">{t("Check a phrase")}</label>
        <div className="flex flex-wrap gap-2">
          <Input id="pron-phrase" className="min-w-0 flex-1" maxLength={200} value={phrase} placeholder={t("pronPhrasePlaceholder")} onChange={e => setPhrase(e.target.value)} />
          <Button variant="outline" disabled={busy || !phrase.trim()} onClick={() => void run(async () => setPrepared(await previewNormalized(phrase, speechLanguage)))}>{t("Show how it is read")}</Button>
          <Button variant="outline" disabled={busy || !phrase.trim() || !ready} title={ready ? undefined : t("pronListenNeedsVoice")} onClick={() => void listen(phrase)}>{t("Listen")}</Button>
        </div>
        {prepared !== null && <p className="rounded border bg-muted/40 p-2 text-sm" aria-live="polite">{prepared}</p>}
        {audio && <AudioPlayer key={audio} autoPlay src={audio} />}
      </div>

      <form className="space-y-2 rounded-lg border p-3" onSubmit={e => { e.preventDefault(); void run(async () => { await savePronunciation(draft); setDraft(EMPTY); await reload(); }); }}>
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="space-y-1 text-sm"><span>{t("Written as")}</span><Input maxLength={100} value={draft.pattern} placeholder="IPO" onChange={e => setDraft({ ...draft, pattern: e.target.value })} /></label>
          <label className="space-y-1 text-sm"><span>{t("Read as")}</span><Input maxLength={300} value={draft.replacement} placeholder={t("pronReplacementPlaceholder")} onChange={e => setDraft({ ...draft, replacement: e.target.value })} /></label>
        </div>
        <p className="text-xs text-muted-foreground">{t("pronStressHint")}</p>
        <div className="flex flex-wrap items-center gap-4 text-sm">
          <label className="flex items-center gap-2">{t("Language")}<select className="rounded border bg-background p-1.5" value={draft.language} onChange={e => setDraft({ ...draft, language: e.target.value as PronunciationEntry["language"] })}>
            <option value="any">{t("Any language")}</option><option value="ru">{t("Russian")}</option><option value="en">{t("English")}</option>
          </select></label>
          <label className="flex items-center gap-2"><input type="checkbox" checked={draft.whole_word} onChange={e => setDraft({ ...draft, whole_word: e.target.checked })} />{t("Whole word")}</label>
          <label className="flex items-center gap-2"><input type="checkbox" checked={draft.case_sensitive} onChange={e => setDraft({ ...draft, case_sensitive: e.target.checked })} />{t("Match case")}</label>
        </div>
        <div className="flex gap-2">
          <Button type="submit" disabled={busy || !draft.pattern.trim() || !draft.replacement.trim()}>{draft.id === null ? t("Add") : t("Save")}</Button>
          {draft.id !== null && <Button type="button" variant="outline" onClick={() => setDraft(EMPTY)}>{t("Cancel")}</Button>}
        </div>
      </form>

      <div className="flex flex-wrap items-center gap-2">
        <Input className="min-w-0 flex-1" value={filter} placeholder={t("Search the dictionary")} onChange={e => setFilter(e.target.value)} />
        <Button variant="outline" disabled={busy} onClick={() => void run(async () => {
          const path = await open({ multiple: false, filters: [{ name: "CSV", extensions: ["csv", "txt"] }] });
          if (typeof path !== "string") return;
          const [imported, skipped] = await importPronunciations(path);
          toast.success(t("pronImported", { imported, skipped }));
          await reload();
        })}>{t("Import")}</Button>
        <Button variant="outline" disabled={busy || entries.length === 0} onClick={() => void run(async () => {
          const path = await save({ defaultPath: "glagol-pronunciations.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
          if (!path) return;
          const count = await exportPronunciations(path);
          toast.success(t("pronExported", { count }));
        })}>{t("Export")}</Button>
      </div>

      {entries.length === 0 ? <p className="text-sm text-muted-foreground">{t("pronEmpty")}</p> :
        <ul className="max-h-80 divide-y overflow-auto rounded-lg border">
          {shown.map(entry => <li key={entry.id ?? entry.pattern} className="flex items-center gap-3 p-2 text-sm">
            <Switch aria-label={t("Use during synthesis")} checked={entry.enabled} disabled={busy} onCheckedChange={enabled => void run(async () => { await savePronunciation({ ...entry, enabled }); await reload(); })} />
            <div className="min-w-0 flex-1">
              <p className="truncate"><span className="font-medium">{entry.pattern}</span> → {entry.replacement}</p>
              <p className="text-xs text-muted-foreground">{languageLabel(entry.language)}{entry.whole_word ? "" : ` · ${t("Part of a word")}`}{entry.case_sensitive ? ` · ${t("Match case")}` : ""}</p>
            </div>
            <Button variant="ghost" size="icon" aria-label={t("Listen")} title={ready ? t("Listen") : t("pronListenNeedsVoice")} disabled={busy || !ready || !entry.enabled} onClick={() => void listen(entry.pattern)}><Play className="h-4 w-4" /></Button>
            <Button variant="ghost" size="icon" aria-label={t("Edit")} disabled={busy} onClick={() => setDraft(entry)}><Pencil className="h-4 w-4" /></Button>
            <Button variant="ghost" size="icon" aria-label={t("Delete")} disabled={busy || entry.id === null} onClick={() => void run(async () => { await deletePronunciation(entry.id as number); if (draft.id === entry.id) setDraft(EMPTY); await reload(); })}><Trash2 className="h-4 w-4" /></Button>
          </li>)}
        </ul>}
    </CardContent>
  </Card>;
}
