import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useTts } from "@/contexts/TtsContext";
import { cancelTts, installTts, previewTts, removeTts } from "@/lib/tts";
import { VOICES, DEFAULT_VOICE_ID } from "@/lib/voices";
const mb = (bytes: number) => (bytes / 1e6).toLocaleString("ru-RU", { maximumFractionDigits: 1 });
export function TtsSection() {
  const { status, error, refresh } = useTts();
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [voice, setVoice] = useState(localStorage.getItem("tts-voice") ?? DEFAULT_VOICE_ID);
  const [audio, setAudio] = useState<string | null>(null);
  async function action(fn: () => Promise<unknown>) {
    setBusy(true);
    try { await fn(); } catch (e) { toast.error(String(e)); }
    finally { setBusy(false); await refresh(); }
  }
  const active = busy || !!status?.progress;
  const accepted = checked || !!status?.accepted;
  const stage = status?.progress?.stage;
  return <Card id="local-tts">
    <CardHeader>
      <CardTitle>Локальная озвучка — Silero v5.5</CardTitle>
      <CardDescription>Дополнительная функция. После загрузки работает без интернета.</CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      <p className="text-sm">Silero Team · CC BY-NC-SA 4.0 · <strong>для некоммерческого использования</strong>. Модель и движок скачиваются отдельно по вашему выбору. Для диктовки, в том числе через офисный сервер, Silero не нужна.</p>
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
      {!status ? <p className="text-sm text-muted-foreground">Проверяем состояние…</p> : <>
        <details className="text-sm"><summary className="cursor-pointer">Полная лицензия Silero</summary><pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap rounded border p-3 text-xs">{status.license}</pre></details>
        <p className="text-sm text-muted-foreground">Модель: {mb(status.model_bytes)} МБ · движок: {mb(status.runtime_bytes)} МБ. Для установки освободите не менее {mb(status.disk_bytes)} МБ.</p>
        {!status.supported && <p className="text-sm">Локальная озвучка пока поддерживает Windows x64.</p>}
        {!status.accepted && <label className="flex items-start gap-2 text-sm"><input type="checkbox" checked={checked} disabled={active} onChange={e => setChecked(e.target.checked)} className="mt-1" />Я ознакомился с условиями Silero и буду использовать её некоммерчески.</label>}
        <p className="text-sm" aria-live="polite">{stage === "downloading" ? "Скачиваем компоненты…" : stage === "verifying" ? "Проверяем и устанавливаем компоненты…" : stage === "preparing" ? "Подготовка озвучки…" : status.installed && status.accepted ? "Готова к работе" : "Не установлена"}</p>
        {status.progress && status.progress.total > 0 && <div className="space-y-1"><Progress value={100 * status.progress.downloaded / status.progress.total} /><p className="text-xs">{mb(status.progress.downloaded)} / {mb(status.progress.total)} МБ</p></div>}
        {status.error && <p role="alert" className="text-sm text-destructive">{status.error}</p>}
        <div className="flex flex-wrap gap-2">
          <Button disabled={active || !accepted || !status.supported} onClick={() => void action(() => installTts(accepted, status.license_hash))}>{status.installed ? "Проверить и восстановить" : status.partial_bytes ? "Продолжить загрузку" : "Скачать и включить"}</Button>
          <Button variant="outline" disabled={active || !accepted || !status.supported} onClick={() => void action(async () => {
            const path = await open({ multiple: false, filters: [{ name: "Silero v5_5_ru.pt", extensions: ["pt"] }] });
            if (typeof path === "string") await installTts(accepted, status.license_hash, path);
          })}>Выбрать скачанную модель</Button>
          {active ? <Button variant="outline" onClick={() => void cancelTts().catch(e => toast.error(String(e)))}>Отменить</Button> : (status.installed || status.partial_bytes > 0 || status.accepted) && <Button variant="outline" onClick={() => void action(async () => { setAudio(null); await removeTts(); setChecked(false); })}>Удалить компоненты и загрузки</Button>}
        </div>
        {status.installed && status.accepted && <div className="space-y-2">
          <label className="flex flex-wrap items-center gap-2 text-sm">Голос<select className="rounded border bg-background p-2" value={voice} disabled={active} onChange={e => { setVoice(e.target.value); localStorage.setItem("tts-voice", e.target.value); }}>{VOICES.map(v => <option key={v.id} value={v.id}>{v.label}</option>)}</select></label>
          <Button variant="outline" disabled={active} onClick={() => void action(async () => { setAudio(null); const path = await previewTts(voice); setAudio(convertFileSrc(path) + "?v=" + Date.now()); })}>Послушать голос</Button>
          {audio && <audio key={audio} controls autoPlay src={audio} className="w-full" />}
        </div>}
      </>}
    </CardContent>
  </Card>;
}
