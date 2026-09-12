"""Glagol's offline Silero adapter (MIT); model code remains in its NC package.

One bounded JSON request per line. Only metadata uses stdout; PCM stays on disk.
The parent selects all paths. No hub, HTTP service, pip or runtime downloads.
"""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import threading
import wave

PROTOCOL = sys.stdout
sys.stdout = sys.stderr
VOICES = ("aidar", "baya", "kseniya", "xenia", "eugene")
MODEL_HASH = "50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437"


def emit(payload):
    PROTOCOL.write(json.dumps(payload) + "\n")
    PROTOCOL.flush()


def watch_parent(pid):
    if sys.platform != "win32":
        return
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = (ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong)
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.WaitForSingleObject.argtypes = (ctypes.c_void_p, ctypes.c_ulong)
    kernel.CloseHandle.argtypes = (ctypes.c_void_p,)
    handle = kernel.OpenProcess(0x00100000, False, pid)  # SYNCHRONIZE only
    if not handle:
        os._exit(1)
    def wait():
        kernel.WaitForSingleObject(handle, 0xFFFFFFFF)
        kernel.CloseHandle(handle)
        os._exit(0)
    threading.Thread(target=wait, daemon=True).start()


def normalize(text):
    from num2words import num2words
    # Keep punctuation and explicit Silero stress/focus markers. Digits and
    # Latin letters are otherwise silently removed by this Russian model.
    text = re.sub(r"([аеёиоуыэюяАЕЁИОУЫЭЮЯ])\u0301", r"+\1", text)
    def number(match):
        value = match.group()
        if len(value) > 15 or (len(value) > 1 and value.startswith("0")):
            return " ".join(num2words(int(d), lang="ru") for d in value)
        return num2words(int(value), lang="ru")
    # Decimal/composite values retain every component. We don't infer units or
    # grammatical cases that cannot be determined reliably from plain text.
    text = re.sub(r"(?<=\d),(?=\d)", " запятая ", text)
    text = re.sub(r"(?<=\d):(?=\d)", " часов ", text)
    text = re.sub(r"(?<=\d)\.(?=\d)", " точка ", text)
    text = re.sub(r"[0-9]+", number, text)
    words = {"windows": "виндоус", "python": "пайтон", "linux": "линукс", "email": "электронная почта", "pdf": "пи ди эф", "usb": "ю эс би", "tts": "ти ти эс", "api": "эй пи ай"}
    letters = dict(zip("abcdefghijklmnopqrstuvwxyz", ("эй", "би", "си", "ди", "и", "эф", "джи", "эйч", "ай", "джей", "кей", "эл", "эм", "эн", "оу", "пи", "кью", "ар", "эс", "ти", "ю", "ви", "дабл ю", "экс", "уай", "зэд")))
    text = re.sub(r"[a-zA-Z]+", lambda m: words.get(m[0].lower()) or " ".join(letters[c] for c in m[0].lower()), text)
    text = text.replace("%", " процентов ").replace("№", " номер ")
    return re.sub(r"\s+", " ", text).strip()


def segments(text, limit=480):
    while len(text) > limit:
        end = max(text.rfind(mark, 0, limit) for mark in (". ", "! ", "? ", "; "))
        if end >= limit // 3:
            end += 1
        else:
            end = text.rfind(" ", 0, limit)
        if end < 1:
            end = limit
        yield text[:end].strip()
        text = text[end:].strip()
    if text:
        yield text


def main():
    model_path, output_dir, parent = sys.argv[1:]
    watch_parent(int(parent))
    with open(model_path, "rb") as source:
        if hashlib.file_digest(source, "sha256").hexdigest() != MODEL_HASH:
            raise ValueError("model_hash")
    import torch
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    model = torch.package.PackageImporter(model_path).load_pickle("tts_models", "model")
    model.to(torch.device("cpu"))
    emit({"ready": True})
    output = Path(output_dir) / "chunk.wav"
    while True:
        line = sys.stdin.buffer.readline(4097)
        if not line:
            return
        if len(line) > 4096 or not line.endswith(b"\n"):
            raise ValueError("protocol_limit")
        request = json.loads(line)
        text, voice = request["text"], request["voice"]
        if not isinstance(text, str) or not 1 <= len(text) <= 300 or voice not in VOICES:
            raise ValueError("invalid_request")
        try:
            normalized = normalize(text)
            if not re.search("[а-яёА-ЯЁ]", normalized):
                emit({"empty": True})
                continue
            with wave.open(str(output), "wb") as writer:
                writer.setnchannels(1)
                writer.setsampwidth(2)
                writer.setframerate(24000)
                for segment in segments(normalized):
                    with torch.inference_mode():
                        audio = model.apply_tts(text=segment, speaker=voice, sample_rate=24000, put_accent=True, put_yo=True)
                    if not torch.isfinite(audio).all() or audio.numel() > 24000 * 300:
                        raise ValueError("audio_limit")
                    pcm = (audio.clamp(-1, 1) * 32767).round().to(torch.int16).cpu().numpy().astype("<i2")
                    writer.writeframes(pcm.tobytes())
            emit({"ok": True})
        except Exception:
            output.unlink(missing_ok=True)
            emit({"error": "synthesis_failed"})


if __name__ == "__main__":
    try:
        main()
    except Exception:
        emit({"error": "worker_failed"})
        raise SystemExit(1)
