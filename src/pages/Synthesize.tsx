import { currentLocale } from "@/i18n";
import { t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { FileUp, Loader2Icon } from "lucide-react";

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

import { ScannedPdfDialog } from "@/components/ScannedPdfDialog";
import { useTts } from "@/contexts/TtsContext";
import { cancelTts, prepareTts } from "@/lib/tts";
import {
  exportAudio,
  readAndParseFile,
  synthesizeDocument,
  type ProgressEvent,
} from "@/lib/tauri";
import { voicesFor } from "@/lib/voices";
import { TtsLanguageSwitch } from "@/components/TtsLanguageSwitch";

/**
 * Synthesize page — paste text, pick a voice, hit "Озвучить и
 * сохранить в библиотеку". The backend now persists the result
 * automatically (DB row + audio file in the local cache), so the
 * primary action no longer prompts for a save location. A secondary
 * "Сохранить на диск" button appears beneath the textarea after a
 * successful synthesis so the user can still export the WAV to a
 * path of their choosing.
 *
 * Readiness depends on optional local TTS, independently of dictation.
 */
export function Synthesize() {
  useI18n();
  const { status, error, provider } = useTts();
  const state = !status && !error ? "unknown" : status?.installed && status.accepted ? "valid" : "invalid";
  const navigate = useNavigate();
  const [text, setText] = useState<string>("");
  const { preferences, setTtsVoice } = usePreferences();
  const speechLanguage = preferences?.tts_language ?? "ru";
  const voice = (speechLanguage === "en" ? preferences?.tts_voice_en : preferences?.tts_voice_ru) ?? (speechLanguage === "en" ? "en_0" : "xenia");
  const VOICES = voicesFor(speechLanguage);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  // The document_id of the most recent successful synthesis — drives
  // the secondary "Сохранить на диск" button. Cleared on the next
  // synthesis kick-off so the button never points at a stale id.
  const [lastDocumentId, setLastDocumentId] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState<boolean>(false);
  // File picker state. `isLoadingFile` disables the picker button while
  // the parse IPC is in flight; `scannedPdfOpen` drives the OCR-disclaimer
  // modal when the PDF parser returns is_scanned_pdf = true.
  const [isLoadingFile, setIsLoadingFile] = useState<boolean>(false);
  const [scannedPdfOpen, setScannedPdfOpen] = useState<boolean>(false);
  const [isWarmingUp, setIsWarmingUp] = useState<boolean>(true);
  const warmupStarted = useRef<string | null>(null);

  useEffect(() => {
    if (!status?.installed || !status.accepted || warmupStarted.current === provider) return;
    warmupStarted.current = provider;
    setIsWarmingUp(true);
    // The backend reports preparation failures through TTS status; the warm-up
    // must not produce a duplicate toast before the user starts an operation.
    void prepareTts(provider)
      .catch(() => undefined)
      .finally(() => { if (warmupStarted.current === provider) setIsWarmingUp(false); });
  }, [provider, status?.accepted, status?.installed]);

  async function handleSynthesize() {
    setIsLoading(true);
    setProgress(null);
    setLastDocumentId(null);
    try {
      const documentId = await synthesizeDocument(text, voice, setProgress, provider);
      setLastDocumentId(documentId);
      toast.success(t("Saved to library"), {
        action: {
          label: t("Open library"),
          onClick: () => navigate("/library"),
        },
        duration: 8000,
      });
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsLoading(false);
      setProgress(null);
    }
  }

  async function handlePickFile() {
    setIsLoadingFile(true);
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        title: t("Choose a file"),
        filters: [
          { name: t("Supported files"), extensions: ["txt", "md", "docx", "pdf"] },
          { name: t("All files"), extensions: ["*"] },
        ],
      });
      if (picked === null) return; // user cancelled — preserve current textarea

      // With `multiple: false` the plugin returns a single string; the
      // array branch is here only to satisfy strict typing for older
      // Tauri shapes and is unreachable at runtime.
      const filePath = typeof picked === "string" ? picked : picked[0];

      const parsed = await readAndParseFile(filePath);

      if (parsed.is_scanned_pdf) {
        setScannedPdfOpen(true);
        return; // leave the textarea untouched
      }

      setText(parsed.text);
      const chars = parsed.text.length;
      toast.success(t("File loaded ({p0} chars)", { p0: chars.toLocaleString(currentLocale()) }));
    } catch (err) {
      // Backend errors come back as Russian-language strings already
      // (file too big, content too long, parse error). Surface them
      // directly; the textarea is preserved either way.
      toast.error(stringifyError(err));
    } finally {
      setIsLoadingFile(false);
    }
  }

  async function handleExportToDisk() {
    if (lastDocumentId === null) return;
    setIsExporting(true);
    try {
      const dest = await save({
        title: t("Save WAV"),
        defaultPath: `glagol-${lastDocumentId.slice(0, 8)}.wav`,
        filters: [{ name: "WAV audio", extensions: ["wav"] }],
      });
      if (dest === null) return; // user cancelled

      await exportAudio(lastDocumentId, dest);
      const filename = dest.split(/[\\/]/).pop() ?? dest;
      toast.success(t("Saved: {p0}", { p0: filename }));
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsExporting(false);
    }
  }

  if (state === "unknown") {
    return (
      <div className="space-y-6">
        <Header />
        <p className="text-muted-foreground text-sm">{t("Loading…")}</p>
      </div>
    );
  }

  if (state === "invalid") {
    return (
      <div className="space-y-6">
        <Header />
        <Card>
          <CardHeader>
            <CardTitle>{t("Install local text to speech")}</CardTitle>
            <CardDescription>
              {t("Silero is downloaded separately for noncommercial use. Dictation is available without it.")}{" "}{error}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button asChild>
              <Link to="/settings">{t("Go to Settings →")}</Link>
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  const trimmedTextLength = text.trim().length;
  const preparationActive = isWarmingUp || status?.progress?.stage === "preparing";
  const canSynthesize = !isLoading && !preparationActive && !status?.progress && trimmedTextLength > 0;
  const canExport = !isLoading && !isExporting && lastDocumentId !== null;

  return (
    <div className="space-y-6">
      <Header disabled={isLoading || preparationActive} />

      <Card>
        <CardContent className="space-y-4 pt-6">
          {preparationActive && (
            <div className="space-y-2 rounded-lg border bg-muted/30 p-3" role="status" aria-live="polite">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Loader2Icon className="h-4 w-4 animate-spin" aria-hidden="true" />
                {t("Preparing local speech…")}{" "}</div>
              <Progress aria-label={t("Preparing local speech")} />
              <p className="text-muted-foreground text-xs">
                {t("Verifying files and loading the model. A full integrity check runs every 30 days and may take longer.")}{" "}</p>
            </div>
          )}
          {status?.error && <p role="alert" className="text-destructive text-sm">{status.error}</p>}
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <Label htmlFor="text">{t("Text")}</Label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={handlePickFile}
                disabled={isLoading || isLoadingFile}
              >
                <FileUp className="mr-1 h-4 w-4" />
                {isLoadingFile ? t("Opening…") : t("Choose a file")}
              </Button>
            </div>
            <Textarea
              id="text"
              rows={12}
              placeholder={t("Paste the text you want to read aloud.")}
              value={text}
              onChange={(event) => setText(event.target.value)}
              disabled={isLoading}
            />
            <p className="text-muted-foreground text-xs">
              {trimmedTextLength.toLocaleString(currentLocale())} {t("characters")}{" "}</p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="voice">{t("Voice")}</Label>
            <Select value={voice} onValueChange={value => { void setTtsVoice(value).catch(e => toast.error(String(e))); }} disabled={isLoading}>
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
            {isLoading
              ? t("Synthesizing…")
              : preparationActive
                ? t("Preparing speech…")
                : t("Synthesize and save to library")}
          </Button>

          {lastDocumentId !== null && (
            <Button
              variant="outline"
              onClick={handleExportToDisk}
              disabled={!canExport}
              className="w-full"
            >
              {isExporting ? t("Saving…") : t("Save to disk")}
            </Button>
          )}

          {isLoading && <Button variant="outline" onClick={() => void cancelTts().catch(e => toast.error(String(e)))}>{t("Cancel synthesis")}</Button>}
          {progress !== null && <ProgressIndicator event={progress} />}
        </CardContent>
      </Card>

      <ScannedPdfDialog open={scannedPdfOpen} onOpenChange={setScannedPdfOpen} />
    </div>
  );
}

function Header({ disabled = false }: { disabled?: boolean }) {
  useI18n();
  return (
    <div>
      <TtsLanguageSwitch disabled={disabled} />
      <h2 className="text-2xl font-semibold tracking-tight">{t("Synthesize")}</h2>
      <p className="text-muted-foreground mt-1 text-sm">
        {t("Your text will become a WAV file in your library.")}{" "}</p>
    </div>
  );
}

interface ProgressIndicatorProps {
  event: ProgressEvent;
}

function ProgressIndicator({ event }: ProgressIndicatorProps) {
  useI18n();
  // Translate a discriminated ProgressEvent into a [0..100] percentage
  // plus a Russian status line. Reserve 5% for chunking, 90% for the
  // per-chunk loop, and 5% for the final join.
  let percent = 0;
  let label = "";
  switch (event.kind) {
    case "preparing":
      label = t("Preparing local speech…");
      break;
    case "chunked":
      percent = 5;
      label = t("Text split into {p0} chunks", { p0: event.total.toLocaleString(currentLocale()) });
      break;
    case "synthesizingChunk":
      percent = 5 + Math.round((event.current / event.total) * 90);
      label = t("Synthesizing chunk {p0} of {p1}", { p0: event.current, p1: event.total });
      break;
    case "joining":
      percent = 95;
      label = t("Joining WAV…");
      break;
  }
  return (
    <div className="space-y-2">
      <Progress value={event.kind === "preparing" ? undefined : percent} />
      <p className="text-muted-foreground text-xs">{label}</p>
    </div>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
