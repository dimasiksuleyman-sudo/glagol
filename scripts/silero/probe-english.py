"""Opt-in v3_en CPU compatibility probe; no downloads, public synthetic text only."""
import argparse
import hashlib
import json
from pathlib import Path
import time
import wave

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    manifest=json.loads(Path(__file__).with_name('english-model.json').read_text(encoding='utf-8'))
    with args.model.open('rb') as source:
        if args.model.stat().st_size!=manifest['bytes'] or hashlib.file_digest(source,'sha256').hexdigest()!=manifest['sha256']:
            raise ValueError('Model integrity mismatch; nothing was loaded')
    args.output.mkdir(parents=True,exist_ok=False)
    start=time.perf_counter()
    import torch
    torch.set_num_threads(2);torch.set_num_interop_threads(1)
    model=torch.package.PackageImporter(str(args.model.resolve())).load_pickle('tts_models','model')
    model.to(torch.device('cpu'))
    report={'torch':torch.__version__,'load_seconds':time.perf_counter()-start,'voices':list(model.speakers),'samples':[]}
    print('Loaded',report['torch'],len(model.speakers),'voices',round(report['load_seconds'],3),'seconds',flush=True)
    # No assumptions about gender/accent based on numeric voice IDs.
    voices=['en_0','en_1','en_2','en_3','en_117']
    cases={
      'plain':'Hello! This is Glagol. Your documents stay on this computer. Are you ready to listen?',
      'numbers':'Today is September twenty fourth, two thousand and twenty six. The meeting starts at two thirty. The price is twelve dollars and fifty cents.',
      'paragraph':'The town woke slowly. Someone opened a window, and a bird started singing in the garden. Alice picked up her book and remembered a conversation from the day before. Why had that question stayed in her mind? She decided to finish the chapter before going for a walk.',
    }
    for voice,case,repeat in [(v,'plain',r) for v in voices for r in range(2)]+[('en_0',k,0) for k in cases if k!='plain']:
        start=time.perf_counter()
        with torch.inference_mode():
            audio=model.apply_tts(text=cases[case],speaker=voice,sample_rate=24000)
        elapsed=time.perf_counter()-start
        assert audio.ndim==1 and 2400<audio.numel()<24000*120 and torch.isfinite(audio).all()
        assert float(audio.abs().max())>0.001
        pcm=(audio.clamp(-1,1)*32767).round().to(torch.int16).cpu().numpy().astype('<i2')
        with wave.open(str(args.output/f'{voice}-{case}-{repeat}.wav'),'wb') as wav:
            wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(24000);wav.writeframes(pcm.tobytes())
        row={'voice':voice,'case':case,'repeat':repeat,'synthesis_seconds':round(elapsed,3),'audio_seconds':audio.numel()/24000}
        report['samples'].append(row);print(json.dumps(row),flush=True)
    (args.output/'report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print('SILERO EN SHARED RUNTIME PASS',flush=True)

if __name__=='__main__': main()
