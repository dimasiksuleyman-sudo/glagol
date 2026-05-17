import { useState } from "react";
import { Link } from "react-router-dom";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

import { useCredentials } from "@/contexts/CredentialsContext";
import { synthesizeDocument, writeWavFile, type ProgressEvent } from "@/lib/tauri";
import { DEFAULT_VOICE_ID, VOICES } from "@/lib/voices";

/**
 * Synthesize page — paste text, pick a voice, hit "Озвучить и
 * сохранить". The single button runs the full pipeline (chunker →
 * synthesize → wav_join) and immediately opens a native Save As
 * dialog so the resulting WAV lands wherever the user picks.
 *
 * Gated by the credentials context: while the mount-time probe is in
 * flight (`"unknown"`) we render nothing distracting; on `"invalid"`
 * we point the user at Settings.
 */
export function Synthesize() {
  const { state } = useCredentials();
  const [text, setText] = useState<string>("");
  const [voice, setVoice] = useState<string>(DEFAULT_VOICE_ID);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);

  async function handleSynthesize() {
    setIsLoading(true);
    setProgress(null);
    try {
      const bytes = await synthesizeDocument(text, voice, setProgress);

      const path = await save({
        title: "Сохранить WAV",
        defaultPath: "glagol.wav",
        filters: [{ name: "WAV audio", extensions: ["wav"] }],
      });

      if (path === null) {
        // User cancelled the dialog. WAV bytes are discarded; the
        // SaluteSpeech character cost has already been paid.
        return;
      }

      await writeWavFile(path, bytes);

      const filename = path.split(/[\\/]/).pop() ?? path;
      toast.success(`Сохранено: ${filename}`);
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsLoading(false);
      setProgress(null);
    }
  }

  if (state === "unknown") {
    return (
      <div className="space-y-6">
        <Header />
        <p className="text-muted-foreground text-sm">Загружаем…</p>
      </div>
    );
  }

  if (state === "invalid") {
    return (
      <div className="space-y-6">
        <Header />
        <Card>
          <CardHeader>
            <CardTitle>Ключ не настроен</CardTitle>
            <CardDescription>
              Чтобы озвучить текст, сначала добавьте Authorization Key SaluteSpeech.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button asChild>
              <Link to="/settings">Перейти в Настройки →</Link>
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  const trimmedTextLength = text.trim().length;
  const canSynthesize = !isLoading && trimmedTextLength > 0;

  return (
    <div className="space-y-6">
      <Header />

      <Card>
        <CardContent className="space-y-4 pt-6">
          <div className="space-y-2">
            <Label htmlFor="text">Текст</Label>
            <Textarea
              id="text"
              rows={12}
              placeholder="Вставьте сюда русский текст для озвучивания."
              value={text}
              onChange={(event) => setText(event.target.value)}
              disabled={isLoading}
            />
            <p className="text-muted-foreground text-xs">
              {trimmedTextLength.toLocaleString("ru-RU")} символов
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="voice">Голос</Label>
            <Select value={voice} onValueChange={setVoice} disabled={isLoading}>
              <SelectTrigger id="voice" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {VOICES.map(({ id, label }) => (
                  <SelectItem key={id} value={id}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <Button onClick={handleSynthesize} disabled={!canSynthesize} className="w-full">
            {isLoading ? "Озвучиваем…" : "Озвучить и сохранить"}
          </Button>

          {progress !== null && <ProgressIndicator event={progress} />}
        </CardContent>
      </Card>
    </div>
  );
}

function Header() {
  return (
    <div>
      <h2 className="text-2xl font-semibold tracking-tight">Озвучить</h2>
      <p className="text-muted-foreground mt-1 text-sm">
        Текст превратится в WAV-файл с выбранным голосом.
      </p>
    </div>
  );
}

interface ProgressIndicatorProps {
  event: ProgressEvent;
}

function ProgressIndicator({ event }: ProgressIndicatorProps) {
  // Translate a discriminated ProgressEvent into a [0..100] percentage
  // plus a Russian status line. Reserve 5% for chunking, 90% for the
  // per-chunk loop, and 5% for the final join.
  let percent = 0;
  let label = "";
  switch (event.kind) {
    case "chunked":
      percent = 5;
      label = `Текст разбит на ${event.total.toLocaleString("ru-RU")} фрагментов`;
      break;
    case "synthesizingChunk":
      percent = 5 + Math.round((event.current / event.total) * 90);
      label = `Озвучиваем фрагмент ${event.current} из ${event.total}`;
      break;
    case "joining":
      percent = 95;
      label = "Склеиваем WAV…";
      break;
  }
  return (
    <div className="space-y-2">
      <Progress value={percent} />
      <p className="text-muted-foreground text-xs">{label}</p>
    </div>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
