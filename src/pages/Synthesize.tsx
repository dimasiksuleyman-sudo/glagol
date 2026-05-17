import { useState } from "react";
import { Link } from "react-router-dom";

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
import { DEFAULT_VOICE_ID, VOICES } from "@/lib/voices";
import type { ProgressEvent } from "@/lib/tauri";

/**
 * Synthesize page — paste text, pick a voice, hit "Озвучить и
 * сохранить". The single button runs the full pipeline (chunker →
 * synthesize → wav_join) and immediately opens a native Save As dialog
 * so the resulting WAV lands wherever the user picks.
 *
 * Phase 2 skeleton: UI only, no Tauri wiring. Phase 3 connects the
 * button to {@link synthesizeDocument} + {@link writeWavFile}.
 */
export function Synthesize() {
  const { hasCredentials } = useCredentials();
  const [text, setText] = useState<string>("");
  const [voice, setVoice] = useState<string>(DEFAULT_VOICE_ID);
  const [isLoading, _setIsLoading] = useState<boolean>(false);
  const [progress, _setProgress] = useState<ProgressEvent | null>(null);

  if (!hasCredentials) {
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

  function handleSynthesize() {
    // Phase 3: synthesizeDocument(text, voice, setProgress) →
    // dialog.save() → writeWavFile(path, bytes) → toast.
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
  // plus a Russian status line. Phase 3 will keep this logic identical
  // — only the call site that supplies `event` becomes real.
  let percent = 0;
  let label = "";
  switch (event.kind) {
    case "chunked":
      percent = 5;
      label = `Текст разбит на ${event.total.toLocaleString("ru-RU")} фрагментов`;
      break;
    case "synthesizingChunk":
      // Reserve 5% for chunking, 5% for joining, 90% for synthesis.
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
