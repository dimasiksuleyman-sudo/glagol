"""Opt-in CPU smoke test of the exact Silero v5.5 model; no downloads.

Run in a child process with a timeout. Writes public test sentences as WAV and
timing metadata in a fresh output directory. Never touches Glagol user data.
"""
import argparse
import ctypes
import hashlib
import json
from pathlib import Path
import time
import wave

MODEL_SHA256 = "50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437"
VOICES = ("aidar", "baya", "kseniya", "xenia", "eugene")
CASES = {
    "plain": "Привет! Это Глагол. Проверяем локальную озвучку на вашем компьютере.",
    "questions": "Ты уже готов? Когда мы отправимся домой? Неужели это работает без интернета?",
    "stress": "На двери висит зам+ок. На горе стоит з+амок. Я уже готов открыть все ваши замки.",
    "numbers": "Сегодня 12 сентября 2026 года. Встреча в 14:30. Стоимость — 1234,56 рубля.",
    "words": "Сегодня двенадцатое сентября две тысячи двадцать шестого года. Встреча в четырнадцать тридцать.",
    "latin": "Windows, Python, USB и PDF. Электронная почта и интернет.",
    "paragraph": "Утром город просыпался медленно. Сначала за окнами послышались шаги, потом загудел первый трамвай. Анна открыла книгу и вспомнила вчерашний разговор. Почему этот вопрос не давал ей покоя? Она решила дочитать главу до конца, а после отправиться на прогулку. На улице было прохладно, но солнце уже освещало верхушки деревьев. День обещал быть долгим и интересным.",
}


def peak_working_set():
    class Counters(ctypes.Structure):
        _fields_ = [("cb", ctypes.c_ulong), ("PageFaultCount", ctypes.c_ulong)] + [
            (name, ctypes.c_size_t) for name in ("PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = ctypes.c_void_p
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = (ctypes.c_void_p, ctypes.POINTER(Counters), ctypes.c_ulong)
    counters = Counters()
    counters.cb = ctypes.sizeof(counters)
    if not psapi.GetProcessMemoryInfo(kernel.GetCurrentProcess(), ctypes.byref(counters), counters.cb):
        raise ctypes.WinError(ctypes.get_last_error())
    return counters.PeakWorkingSetSize


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--threads", type=int, choices=(2, 4), required=True)
    args = parser.parse_args()
    with args.model.open("rb") as source:
        if hashlib.file_digest(source, "sha256").hexdigest() != MODEL_SHA256:
            raise ValueError("Model SHA-256 mismatch; nothing was loaded")
    args.output.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter()
    import torch
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    model = torch.package.PackageImporter(str(args.model.resolve())).load_pickle("tts_models", "model")
    model.to(torch.device("cpu"))
    report = {"torch": torch.__version__, "threads": args.threads, "model_bytes": args.model.stat().st_size,
              "model_sha256": MODEL_SHA256, "import_and_load_seconds": time.perf_counter() - started,
              "voices": list(model.speakers), "samples": []}
    assert all(voice in model.speakers for voice in VOICES)
    # Every voice twice; the remaining corpus exercises the default voice.
    samples = [(voice, "plain", repeat) for voice in VOICES for repeat in range(2)]
    samples += [("xenia", case, 0) for case in CASES if case != "plain"]
    for voice, case, repeat in samples:
        start = time.perf_counter()
        try:
            with torch.inference_mode():
                audio = model.apply_tts(text=CASES[case], speaker=voice, sample_rate=24000,
                                        put_accent=True, put_yo=True)
            seconds = time.perf_counter() - start
            assert audio.ndim == 1 and audio.numel() > 2400 and torch.isfinite(audio).all()
            assert float(audio.abs().max()) > 0.001
            pcm = (audio.clamp(-1, 1) * 32767).round().to(torch.int16).cpu().numpy().astype("<i2")
            with wave.open(str(args.output / f"{voice}-{case}-{repeat}.wav"), "wb") as writer:
                writer.setnchannels(1)
                writer.setsampwidth(2)
                writer.setframerate(24000)
                writer.writeframes(pcm.tobytes())
            duration = audio.numel() / 24000
            entry = {"voice": voice, "case": case, "repeat": repeat, "synthesis_seconds": seconds,
                     "audio_seconds": duration, "real_time_factor": seconds / duration, "ok": True}
        except Exception as error:
            entry = {"voice": voice, "case": case, "repeat": repeat, "ok": False, "error_type": type(error).__name__}
        report["samples"].append(entry)
        report["peak_working_set_bytes"] = peak_working_set()
        (args.output / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
        print(json.dumps(entry), flush=True)
    if not all(sample["ok"] for sample in report["samples"]):
        raise SystemExit("Some corpus cases failed; inspect report before integrating")


if __name__ == "__main__":
    main()
