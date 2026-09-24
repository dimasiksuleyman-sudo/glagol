"""Owned release Moonshine child plus Silero EN worker on public synthetic audio."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import wave


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'stt_root', 'tts_root', 'wav', 'output'):
        p.add_argument('--' + name.replace('_', '-'), type=Path, required=True)
    a = p.parse_args()
    import numpy as np
    with wave.open(str(a.wav), 'rb') as wav:
        assert wav.getnchannels() == 1 and wav.getsampwidth() == 2
        rate = wav.getframerate()
        source = np.frombuffer(wav.readframes(wav.getnframes()), dtype='<i2').astype(np.float32) / 32768
    # This harness prepares a 16 kHz fixture only. Glagol's real streaming
    # resampler is independently exercised by Rust boundary/native tests.
    samples = np.interp(np.arange(int(len(source) * 16000 / rate)) * rate / 16000, np.arange(len(source)), source)
    output = a.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    worker = Path(__file__).resolve().parents[2] / 'src-tauri/src/tts/silero/worker.py'
    commands = [
        [str(a.exe.resolve()), '--moonshine-worker', str(a.stt_root.resolve()), str(os.getpid())],
        [str(a.tts_root.resolve() / 'runtime/python.exe'), '-I', '-B', '-u', str(worker), str(a.tts_root.resolve() / 'v3_en.pt'), str(output), str(os.getpid()), 'en'],
    ]
    children = []
    def stop():
        for child in children:
            if child.poll() is None:
                child.kill()
    timer = threading.Timer(120, stop)
    timer.start()
    try:
        for cmd in commands:
            children.append(subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, creationflags=0x08000000))
        stt, tts = children
        assert json.loads(stt.stdout.readline(512001))['ok']
        assert json.loads(tts.stdout.readline(1025))['ready']
        def request(child, value, limit):
            child.stdin.write((json.dumps(value) + '\n').encode())
            child.stdin.flush()
            return json.loads(child.stdout.readline(limit))
        tts_started = threading.Event()
        def synthesis():
            start = time.perf_counter()
            text = 'The town woke slowly. Someone opened a window, and a bird started singing in the garden. Alice picked up her book and remembered a conversation from the day before. She decided to finish the chapter before going for a walk.'
            tts.stdin.write((json.dumps({'text': text, 'voice': 'en_0'}) + '\n').encode())
            tts.stdin.flush()
            tts_started.set()
            assert json.loads(tts.stdout.readline(1025))['ok']
            return round((time.perf_counter() - start) * 1000)
        with ThreadPoolExecutor(max_workers=1) as pool:
            task = pool.submit(synthesis)
            assert tts_started.wait(5)
            assert not task.done(), 'TTS must still be running when recognition starts'
            assert request(stt, {'op': 'begin'}, 512001)['ok']
            start = time.perf_counter()
            for offset in range(0, len(samples), 4096):
                assert request(stt, {'op': 'audio', 'samples': samples[offset:offset + 4096].tolist()}, 512001)['ok']
            final = request(stt, {'op': 'finish'}, 512001)
            assert final['ok']
            assert 'hello' in final['text'].lower() and 'listen' in final['text'].lower()
            recognition_ms = round((time.perf_counter() - start) * 1000)
            synthesis_ms = task.result(timeout=90)
        with wave.open(str(output / 'chunk.wav'), 'rb') as wav:
            assert wav.getframerate() == 24000 and wav.getnframes() > 24000
        report = {'recognition_fixture_ms': recognition_ms, 'synthesis_ms': synthesis_ms, 'overlap': True, 'live_microphone': False, 'ui_tested': False}
        (output / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        print('CONCURRENT EN WORKERS PASS', json.dumps(report))
    finally:
        timer.cancel()
        stop()
        for child in children:
            child.wait(timeout=5)


if __name__ == '__main__':
    main()
