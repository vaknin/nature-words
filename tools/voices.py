# /// script
# requires-python = ">=3.10,<3.13"
# dependencies = ["omnivoice==0.2.1", "torch==2.8.0", "torchaudio==2.8.0", "soundfile", "faster-whisper==1.2.1", "numpy"]
# [tool.uv.sources]
# torch = { index = "pytorch-cpu" }
# torchaudio = { index = "pytorch-cpu" }
# [[tool.uv.index]]
# name = "pytorch-cpu"
# url = "https://download.pytorch.org/whl/cpu"
# explicit = true
# ///
# Records every word and spoken line of index.html in English and embeds them base64 in index.html.
# Python because OmniVoice (k2-fsa) only ships as a PyTorch model; built on letter-b's tools/takes.py.
#   uv run tools/voices.py ref-girl   design the girl once (tools/clips/girl-ref.wav); the boy is letter-b's,
#                                     the voice Aviv liked (tools/clips/boy-ref.wav)
#   uv run tools/voices.py record     record what is missing, check each with Whisper, retake when misheard
#   uv run tools/voices.py embed      slow every raw clip 15% (pitch kept) and write them into index.html
# The lines come from the site itself: LESSON and PHRASES(LESSON) in index.html, read through node, so a
# line added on the site is recorded on the next run and the smoke test fails until it is.
import base64, json, os, re, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLIPS = ROOT / "tools" / "clips"
RAW = CLIPS / "raw"
REF_TEXT = "Hi! I'm so happy to see you. Look, a big red ball and a yellow banana. Let's play together!"
VOICES = {
    "boy": CLIPS / "boy-ref.wav",
    "girl": CLIPS / "girl-ref.wav",
}
GIRL = os.environ.get("GIRL", "female, child, american accent")
# Of seeds 4, 7 and 11, all heard word for word by Whisper; 7 read the line calmly (4 shouted every sentence)
# and sits at a child girl's pitch (median F0 356 Hz against the boy's 262 Hz), so 7.
GIRL_SEED = int(os.environ.get("GIRL_SEED", "7"))
# His "start with 15%": 15% slower than the model's normal, by rubberband from the raw clip so retuning never stacks.
TEMPO = float(os.environ.get("TEMPO", "0.85"))
# A clip is retaken when Whisper's words differ from the line; seeds 1..TRIES are tried, then it stops and says so.
TRIES = 4


def lesson():
    src = (ROOT / "index.html").read_text()
    code = src[src.index("const LESSON = {"):src.index("/* CLIPS:BEGIN")]
    js = code + "\nconsole.log(JSON.stringify(PHRASES(LESSON)));"
    return json.loads(subprocess.run(["node", "-e", js], capture_output=True, text=True, check=True).stdout)


def slug(text):
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")[:60]


def norm(text):
    return re.sub(r"[^a-z0-9 ]+", "", text.lower().replace("-", " ")).split()


_m, _prompts, _w = None, {}, None


def model():
    global _m
    if _m is None:
        import torch
        from omnivoice import OmniVoice
        torch.set_num_threads(int(os.environ.get("THREADS", "10")))
        _m = OmniVoice.from_pretrained("k2-fsa/OmniVoice", device_map="cpu", dtype=torch.float32)
    return _m


def prompt(voice):
    if voice not in _prompts:
        _prompts[voice] = model().create_voice_clone_prompt(ref_audio=str(VOICES[voice]), ref_text=REF_TEXT)
    return _prompts[voice]


def heard(path):
    global _w
    if _w is None:
        from faster_whisper import WhisperModel
        _w = WhisperModel("small.en", device="cpu", compute_type="int8")
    # Decoded with ffmpeg to 16 kHz mono floats (Whisper's input rate): faster-whisper's own decoder calls
    # PyAV with an argument newer PyAV removed, so it crashed on every clip.
    import numpy as np
    pcm = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", "1", "-ar", "16000", "-"], capture_output=True, check=True).stdout
    segs, _ = _w.transcribe(np.frombuffer(pcm, np.float32), language="en", beam_size=5)
    return " ".join(s.text for s in segs).strip()


def to_mp3(wav, out, tempo=1.0):
    """Silence cut at both ends, loudness to -16 LUFS, slowed by TEMPO with pitch kept, mono 24 kHz mp3 64 kbps."""
    trim = "silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.05"
    slow = f"rubberband=tempo={tempo}," if tempo != 1.0 else ""
    af = f"{trim},areverse,{trim},areverse,{slow}loudnorm=I=-16:TP=-1.5:LRA=11,adelay=40"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(wav), "-af", af, "-ac", "1", "-ar", "24000", "-b:a", "64k", str(out)], check=True)


def say(text, voice, out, seed):
    import soundfile as sf, torch
    m = model()
    torch.manual_seed(seed)
    kw = {"voice_clone_prompt": prompt(voice)}
    line = text if text[-1:] in ".!?" else text + "."
    # Short lines: OmniVoice guesses too short a length and clips the word to 0.2 s, so it gets one from its letters.
    if len(line) < 25:
        kw["duration"] = 0.6 + 0.09 * len(line)
    m_out = m.generate(text=line, language="en", **kw)[0]
    sf.write(out, m_out, 24000)


def cmd_ref_girl():
    import soundfile as sf, torch
    VOICES["girl"].parent.mkdir(parents=True, exist_ok=True)
    torch.manual_seed(GIRL_SEED)
    a = model().generate(text=REF_TEXT, instruct=GIRL, language="en", speed=0.9)[0]
    sf.write(VOICES["girl"], a, 24000)
    print(f"girl-ref.wav: {len(a) / 24000:.1f} s, seed {GIRL_SEED}, Whisper heard: {heard(VOICES['girl'])}")


def plan():
    """(text, voice) as PHRASES(LESSON) gives them: the words in both voices, the other lines boy and girl in turn.
    `record words` limits a run to the words alone (one word per line of the list, no spaces)."""
    want = [tuple(p) for p in lesson()]
    if sys.argv[2:3] == ["words"]:
        want = [p for p in want if " " not in p[0] and p[0].islower()]
    return want


def key(text, voice):
    """The clip's name on the site: CLIPS["boy:air"]."""
    return f"{voice}:{text}"


def cmd_record():
    RAW.mkdir(parents=True, exist_ok=True)
    man_f = CLIPS / "clips.json"
    man = json.loads(man_f.read_text()) if man_f.exists() else {}
    bad = []
    for text, voice in plan():
        if key(text, voice) in man and (RAW / man[key(text, voice)]["file"]).exists():
            continue
        out = RAW / f"{voice}-{slug(text)}.wav"
        for seed in range(1, TRIES + 1):
            t = time.time()
            say(text, voice, out, seed)
            got = heard(out)
            ok = norm(got) == norm(text)
            print(f"{voice} '{text}' seed {seed}: heard '{got}' {'ok' if ok else 'RETAKE'} ({time.time() - t:.0f} s)", flush=True)
            if ok:
                break
        man[key(text, voice)] = {"text": text, "voice": voice, "file": out.name, "seed": seed, "heard": got, "ok": ok}
        man_f.write_text(json.dumps(man, indent=1) + "\n")
        if not ok:
            bad.append(text)
    print(f"{len(man)} clips; misheard after {TRIES} tries: {bad or 'none'}")


def cmd_embed():
    man = json.loads((CLIPS / "clips.json").read_text())
    want = [key(t, v) for t, v in plan()]
    missing = [k for k in want if k not in man]
    if missing:
        sys.exit(f"not recorded yet: {missing}")
    clips = {}
    with tempfile.TemporaryDirectory() as d:
        for k in want:
            mp3 = Path(d) / "c.mp3"
            to_mp3(RAW / man[k]["file"], mp3, TEMPO)
            clips[k] = "data:audio/mpeg;base64," + base64.b64encode(mp3.read_bytes()).decode()
    src = (ROOT / "index.html").read_text()
    a, b = src.index("/* CLIPS:BEGIN */"), src.index("/* CLIPS:END */")
    src = src[:a] + "/* CLIPS:BEGIN */\nconst CLIPS = " + json.dumps(clips, indent=0) + ";\n" + src[b:]
    (ROOT / "index.html").write_text(src)
    print(f"index.html: {len(clips)} clips embedded, {len(src) / 1024:.0f} KB")


def cmd_hear():
    """Prints what Whisper hears in each file given (to pick among reference tries)."""
    for f in sys.argv[2:]:
        print(f"{f}: {heard(f)}")


if __name__ == "__main__":
    {"hear": cmd_hear, "ref-girl": cmd_ref_girl, "record": cmd_record, "embed": cmd_embed}[sys.argv[1] if len(sys.argv) > 1 else "record"]()
