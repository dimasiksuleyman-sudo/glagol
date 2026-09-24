import { useEffect, useRef, useState } from "react";
import { Play, Pause } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/contexts/PreferencesContext";

/** App-owned controls use the selected language, independent of Windows/WebView. */
export function AudioPlayer({ src, autoPlay = false }: { src: string; autoPlay?: boolean }) {
  const { t, locale } = useI18n();
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState(0);
  const [position, setPosition] = useState(0);
  const [volume, setVolume] = useState(1);
  const [rate, setRate] = useState(1);
  const [error, setError] = useState(false);
  useEffect(() => { setPlaying(false); setDuration(0); setPosition(0); setError(false); }, [src]);
  useEffect(() => { if (audio.current) { audio.current.volume = volume; audio.current.playbackRate = rate; } }, [volume, rate, src]);
  const clock = (value: number) => `${Math.floor(value / 60).toLocaleString(locale)}:${Math.floor(value % 60).toLocaleString(locale, { minimumIntegerDigits: 2, useGrouping: false })}`;
  async function toggle() {
    if (!audio.current) return;
    if (playing) audio.current.pause();
    else { try { await audio.current.play(); setError(false); } catch { setError(true); } }
  }
  return <div className="space-y-2 rounded-lg border p-3" role="group" aria-label={t("audioPlayer")}>
    <audio ref={audio} src={src} preload="metadata" autoPlay={autoPlay}
      onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => setPlaying(false)}
      onLoadedMetadata={e => setDuration(Number.isFinite(e.currentTarget.duration) ? e.currentTarget.duration : 0)}
      onTimeUpdate={e => setPosition(e.currentTarget.currentTime)} onError={() => setError(true)} />
    <div className="flex items-center gap-3">
      <Button type="button" variant="outline" size="icon" onClick={() => void toggle()} aria-label={playing ? t("pauseAudio") : t("playAudio")}>{playing ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}</Button>
      <input type="range" className="min-w-0 flex-1 accent-primary" min={0} max={duration || 1} step={0.1} value={Math.min(position, duration || 1)} disabled={!duration}
        aria-label={t("audioPosition")} aria-valuetext={`${clock(position)} / ${clock(duration)}`}
        onChange={e => { if (audio.current) { const value = Number(e.target.value); audio.current.currentTime = value; setPosition(value); } }} />
      <span className="whitespace-nowrap text-xs tabular-nums">{clock(position)} / {clock(duration)}</span>
    </div>
    <div className="flex flex-wrap items-center gap-4 text-xs">
      <label className="flex items-center gap-2">{t("volume")}<input type="range" className="w-24 accent-primary" min={0} max={1} step={0.05} value={volume} onChange={e => setVolume(Number(e.target.value))} /></label>
      <label className="flex items-center gap-2">{t("playbackSpeed")}<select className="rounded border bg-background px-1" value={rate} onChange={e => setRate(Number(e.target.value))}>{[0.5, 0.75, 1, 1.25, 1.5, 2].map(value => <option key={value} value={value}>{value.toLocaleString(locale)}×</option>)}</select></label>
    </div>
    {error && <p role="alert" className="text-sm text-destructive">{t("audioPlaybackError")}</p>}
  </div>;
}
