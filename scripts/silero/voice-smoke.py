"""Opt-in current worker protocol test for all nine offered voices, isolated paths."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import time
import wave


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    script = Path(__file__).resolve().parents[2] / 'src-tauri/src/tts/silero/worker.py'
    report = []
    cases = [
        ('ru', 'v5_5_ru.pt', ['aidar', 'baya', 'kseniya', 'xenia', 'eugene'], 'Сегодня 24 сентября 2026 года. Цена 12,50. Проверяем USB. Ты готов?'),
        ('en', 'v3_en.pt', ['en_0', 'en_1', 'en_2', 'en_3'], 'Today is September 24, 2026. The price is $12.50. Check the USB. Are you ready?'),
    ]
    for language, model, voices, text in cases:
        started = time.perf_counter()
        child = subprocess.Popen([str(root / 'runtime/python.exe'), '-I', '-B', '-u', str(script), str(root / model), str(output), str(os.getpid()), language], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, creationflags=0x08000000)
        timer = threading.Timer(120, child.kill)
        timer.start()
        try:
            assert json.loads(child.stdout.readline(1025))['ready']
            report.append({'language': language, 'ready_ms': round((time.perf_counter() - started) * 1000)})
            for voice in voices:
                started = time.perf_counter()
                child.stdin.write((json.dumps({'text': text, 'voice': voice}) + '\n').encode())
                child.stdin.flush()
                assert json.loads(child.stdout.readline(1025))['ok']
                source = output / 'chunk.wav'
                with wave.open(str(source), 'rb') as wav:
                    assert (wav.getnchannels(), wav.getsampwidth(), wav.getframerate()) == (1, 2, 24000)
                    assert 2400 < wav.getnframes() < 24000 * 120
                    assert any(wav.readframes(wav.getnframes()))
                shutil.copyfile(source, output / f'{voice}.wav')
                report.append({'voice': voice, 'elapsed_ms': round((time.perf_counter() - started) * 1000)})
        finally:
            timer.cancel()
            child.kill()
            child.wait(timeout=5)
    (output / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print('SILERO CURRENT WORKER VOICES PASS: 5 RU + 4 EN; numbers, dates, abbreviations; valid mono PCM WAVs (listening separate)')


if __name__ == '__main__':
    main()
