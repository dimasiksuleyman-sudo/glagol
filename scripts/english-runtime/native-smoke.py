import ctypes as c
import hashlib
import json
import pathlib
import struct
import time
import wave
import sys
import zipfile

# Development ABI probe only; Python is NOT an English runtime dependency.
# Inputs and generated WAVs are synthetic. Never pass user documents/audio here.
if sys.platform != 'win32':
    raise RuntimeError('This probe requires Windows x64')
ROOT = pathlib.Path(sys.argv[1] if len(sys.argv)>1 else '.scratch/english-first').resolve()
manifest = json.loads(pathlib.Path(__file__).with_name('artifacts.json').read_text(encoding='utf-8'))
def verify(artifact, data):
    if len(data) != artifact['bytes'] or hashlib.sha256(data).hexdigest() != artifact['sha256']:
        raise RuntimeError('Integrity mismatch: '+artifact['file'])
archive = manifest['runtime']
verify(archive, (ROOT/archive['file']).read_bytes())
# Extract only the two pinned entries; no wheel code, paths or metadata execute.
with zipfile.ZipFile(ROOT/archive['file']) as z:
    for member in archive['members']:
        info=z.getinfo(member['entry'])
        if info.file_size != member['bytes']:
            raise RuntimeError('Unexpected archive member size')
        data=z.read(info);verify(member,data)
        target=ROOT/member['file'];target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(data)
for artifact in manifest['artifacts']:
    verify(artifact,(ROOT/'assets'/artifact['file']).read_bytes())
lib = c.CDLL(str(ROOT/'native/moonshine.dll'), winmode=0x100 | 0x1000)
if lib.moonshine_get_version() != manifest['abi_version']:
    raise RuntimeError('ABI version mismatch')
class Option(c.Structure):
    _fields_ = [('name', c.c_char_p), ('value', c.c_char_p)]
class Line(c.Structure):
    _fields_ = [('text', c.c_char_p), ('audio', c.c_void_p), ('count', c.c_size_t), ('start', c.c_float), ('duration', c.c_float), ('id', c.c_uint64), ('complete', c.c_int8), ('updated', c.c_int8), ('new', c.c_int8), ('changed', c.c_int8), ('speakers_changed', c.c_int8), ('speakers', c.c_void_p), ('speaker_count', c.c_uint64), ('latency', c.c_uint32), ('words', c.c_void_p), ('word_count', c.c_uint64)]
class Transcript(c.Structure):
    _fields_ = [('lines', c.POINTER(Line)), ('count', c.c_uint64)]
def bind(name, args, result=c.c_int32):
    f=getattr(lib,'moonshine_'+name); f.argtypes=args; f.restype=result; return f
def options(values):
    return (Option*len(values))(*[Option(k.encode(),str(v).encode()) for k,v in values.items()])
def check(code):
    if code<0: raise RuntimeError(code)
    return code
u64=c.c_uint64; i32=c.c_int32; u32=c.c_uint32; char=c.c_char_p; op=c.POINTER(Option); fp=c.POINTER(c.c_float); tp=c.POINTER(Transcript)
create_tts=bind('create_tts_synthesizer_from_files',[char,c.POINTER(char),u64,op,u64,i32])
synthesize=bind('text_to_speech',[i32,char,op,u64,c.POINTER(fp),c.POINTER(u64),c.POINTER(i32)])
free_tts=bind('free_tts_synthesizer',[i32],None)
free_buffer=bind('free_buffer',[c.c_void_p],None)
load_stt=bind('load_transcriber_from_files',[char,u32,op,u64,i32])
free_stt=bind('free_transcriber',[i32],None)
create_stream=bind('create_stream',[i32,u32]); start_stream=bind('start_stream',[i32,i32]); stop_stream=bind('stop_stream',[i32,i32]); free_stream=bind('free_stream',[i32,i32]); add=bind('transcribe_add_audio_to_stream',[i32,i32,fp,u64,i32,u32]); transcribe=bind('transcribe_stream',[i32,i32,u32,c.POINTER(tp)])
sample_text=b'Hello from Glagol. This is a local speech test. The library keeps your documents on this computer.'
results=[]
audio_for_stt=None
for voice in ['af_heart','am_michael','bf_emma','bm_george']:
    o=options({'voice':'kokoro_'+voice, 'g2p_root':str(ROOT/'assets/tts')})
    before=time.perf_counter(); h=check(create_tts(b'en-gb' if voice.startswith('b') else b'en-us',None,0,o,len(o),30000)); load_ms=(time.perf_counter()-before)*1000
    for iteration in range(2):
        audio=fp(); count=u64(); rate=i32(); before=time.perf_counter()
        check(synthesize(h,sample_text,None,0,c.byref(audio),c.byref(count),c.byref(rate)))
        elapsed=(time.perf_counter()-before)*1000
        assert rate.value==24000 and 24000<count.value<24000*120
        samples=list(audio[:count.value]); free_buffer(audio)
        assert max(abs(x) for x in samples)>0.01
        if voice=='af_heart': audio_for_stt=samples
        with wave.open(str(ROOT/f'{voice}-{iteration}.wav'),'wb') as wav:
            wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(rate.value)
            wav.writeframes(struct.pack('<'+'h'*len(samples),*[int(max(-1,min(1,x))*32767) for x in samples]))
        record={'kind':'tts','voice':voice,'iteration':iteration,'load_ms':round(load_ms),'synthesis_ms':round(elapsed),'audio_seconds':round(count.value/rate.value,3)}
        results.append(record); print(json.dumps(record),flush=True)
    free_tts(h)
o=options({'decode_incomplete_lines':'false'})
before=time.perf_counter(); h=check(load_stt(str(ROOT/'assets/stt').encode(),4,o,len(o),30000)); load_ms=round((time.perf_counter()-before)*1000); print('STT load_ms',load_ms,flush=True)
for iteration in range(2):
    stream=check(create_stream(h,0)); check(start_stream(h,stream)); t=tp()
    # Uneven second-session blocks exercise tail delivery across boundaries.
    block=4800 if iteration==0 else 997
    feed_start=time.perf_counter()
    for offset in range(0,len(audio_for_stt),block):
        part=audio_for_stt[offset:offset+block]; buf=(c.c_float*len(part))(*part)
        check(add(h,stream,buf,len(part),24000,0)); check(transcribe(h,stream,0,c.byref(t)))
    feed_ms=round((time.perf_counter()-feed_start)*1000)
    before=time.perf_counter(); check(stop_stream(h,stream)); check(transcribe(h,stream,0,c.byref(t))); elapsed=(time.perf_counter()-before)*1000
    assert t and t.contents.count<100
    text=' '.join((t.contents.lines[i].text or b'').decode() for i in range(t.contents.count))
    assert 'local speech test' in text.lower(), 'Public sample did not match'
    assert 'hello' in text.lower() and 'computer' in text.lower(), 'Missing beginning/end of public sample'
    record={'kind':'stt','iteration':iteration,'load_ms':load_ms,'feed_ms':feed_ms,'block_samples':block,'final_ms':round(elapsed),'public_sample_match':True,'lines':t.contents.count}
    results.append(record);print(json.dumps(record),flush=True);check(free_stream(h,stream))
free_stt(h)
# Process peak includes sequentially tested TTS and STT, not simultaneous workers.
class Memory(c.Structure):
    _fields_=[('cb',c.c_ulong),('faults',c.c_ulong),('peak_working_set',c.c_size_t),('working_set',c.c_size_t),('peak_paged',c.c_size_t),('paged',c.c_size_t),('peak_nonpaged',c.c_size_t),('nonpaged',c.c_size_t),('pagefile',c.c_size_t),('peak_pagefile',c.c_size_t)]
kernel=c.WinDLL('kernel32',use_last_error=True)
kernel.GetCurrentProcess.restype=c.c_void_p
psapi=c.WinDLL('psapi',use_last_error=True)
psapi.GetProcessMemoryInfo.argtypes=[c.c_void_p,c.POINTER(Memory),c.c_ulong]
memory=Memory();memory.cb=c.sizeof(memory)
if not psapi.GetProcessMemoryInfo(kernel.GetCurrentProcess(),c.byref(memory),memory.cb):
    raise c.WinError(c.get_last_error())
record={'kind':'memory','peak_working_set_bytes':memory.peak_working_set,'scope':'whole Python probe process; sequential voices and STT'}
results.append(record);print(json.dumps(record),flush=True)
(ROOT/'smoke-results.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
print('NATIVE SMOKE PASS',flush=True)
