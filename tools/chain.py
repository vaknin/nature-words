# Python because Chatterbox (PyTorch), kokoro-onnx and faster-whisper have no Rust binding (as daily-chapter found).
# Every lesson clip through daily-chapter's voice chain: a Kokoro voice read slowly as the reference, cloned by
# Chatterbox 0.5B (as daily-chapter's tools/tts-bench/cast_cb9.py), each clip Whisper-checked word for word.
# Three venvs already on the server, one per model (none of them holds the other two):
#   ~/tts-test/venv     Kokoro (kokoro-onnx)     python3 tools/chain.py refs
#   ~/tts-test/venv-cb  Chatterbox               python3 tools/chain.py gen    (spawns the Whisper side itself)
#   ~/tts-test/venv-fw  faster-whisper           (hear: reads wav paths on stdin, prints what it heard)
#   python3 tools/chain.py all      refs then gen, each in its venv; resumes from what tools/clips/chain.json has.
# The lines and their keys are tools/clips/clips.json's (the site's CLIPS keys). A clip that fails Whisper after
# TRIES seeds keeps the old voice: chain.json marks it ok false and embed leaves its old clip in place.
import base64, json, os, re, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLIPS = ROOT / "tools" / "clips"
OUT = CLIPS / "chain"
MAN = CLIPS / "chain.json"
TTS = Path.home() / "tts-test"
PY = {"kokoro": TTS / "venv/bin/python", "cb": TTS / "venv-cb/bin/python", "fw": TTS / "venv-fw/bin/python"}
SR = 24_000
# Kokoro voices: the girl is af_heart, the voice daily-chapter measured and cloned (its narrator); the boy am_puck,
# the lightest am_ voice (am_adam, onyx, fenrir, michael, santa are grown men); English only.
VOICES = {"girl": "af_heart", "boy": "am_puck"}
# daily-chapter round 3: Chatterbox follows the reference's pace loosely; from a 0.75 reference the clones came out
# at 0.72-0.81 with nothing stretched, so the references are read at 0.75 for slow, clear beginner speech.
REF_SPEED = 0.75
REF_TEXT = ("Hi! I'm so happy to see you. Look, a big red ball and a yellow banana. "
            "Let's play together! Listen to the word, and then say it with me.")
# cast_cb9.py's settings, the ones Aviv heard and kept for daily-chapter.
EXAGGERATION, CFG = 0.8, 0.3
# His brief: retry with another seed up to 3 times, so the first seed and three more.
TRIES = 4
HOMOPHONES = {"see": "sea"}
# Repeat fallback: a lone word said three times as sentences, split at pauses of 120 ms or more (12 frames of 10 ms).
# Measured on the dry clips: in 2 of 3 takes the first piece cut at 120 ms was one clean word; in the third two words
# ran together ('Dry, dry.'), which Whisper rejects, so a gap too long only costs a retake, never a wrong clip.
REPEAT, GAP = 3, 12


def norm(text):
    return [HOMOPHONES.get(w, w) for w in re.sub(r"[^a-z0-9 ]+", "", text.lower().replace("-", " ")).split()]


def slug(text):
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")[:60]


def lines():
    return json.loads((CLIPS / "clips.json").read_text())


def cmd_refs():
    import soundfile as sf
    from kokoro_onnx import Kokoro
    k = Kokoro(str(TTS / "models/kokoro-v1.0.onnx"), str(TTS / "models/voices-v1.0.bin"))
    OUT.mkdir(parents=True, exist_ok=True)
    for who, v in VOICES.items():
        f = OUT / f"ref-{who}.wav"
        if f.exists():
            continue
        a, sr = k.create(REF_TEXT, voice=v, speed=REF_SPEED, lang="en-us")
        sf.write(f, a, sr)
        print(f"{f.name}: {v} at {REF_SPEED}, {len(a) / sr:.1f} s", flush=True)


def cmd_hear():
    import numpy as np
    from faster_whisper import WhisperModel
    w = WhisperModel("small.en", device="cpu", compute_type="int8", cpu_threads=4)
    for path in sys.stdin:
        pcm = subprocess.run(["ffmpeg", "-v", "error", "-i", path.strip(), "-f", "f32le", "-ac", "1", "-ar", "16000", "-"],
                             capture_output=True, check=True).stdout
        segs, _ = w.transcribe(np.frombuffer(pcm, np.float32), language="en", beam_size=5)
        print(json.dumps(" ".join(s.text for s in segs).strip()), flush=True)


def trim(a):
    """cast_cb9.py trim(): cut to the speech, 40 ms kept at both ends."""
    import numpy as np
    fr = SR // 100
    e = np.array([np.sqrt(np.mean(a[i:i + fr] ** 2)) for i in range(0, len(a) - fr, fr)])
    on = np.where(e > e.max() * 10 ** (-40 / 20))[0]
    m = SR * 40 // 1000
    return a[max(on[0] * fr - m, 0):min((on[-1] + 1) * fr + m, len(a))]


def islands(a):
    """Speech runs split at pauses of GAP or more (energy 40 dB under the peak, cast_cb9.py trim()'s threshold)."""
    import numpy as np
    fr = SR // 100
    e = np.array([np.sqrt(np.mean(a[i:i + fr] ** 2)) for i in range(0, len(a) - fr, fr)])
    on = np.where(e > e.max() * 10 ** (-40 / 20))[0]
    runs, s0, prev = [], on[0], on[0]
    for i in on[1:]:
        if i - prev > GAP:
            runs.append((s0, prev)); s0 = i
        prev = i
    runs.append((s0, prev))
    m = SR * 40 // 1000
    return [a[max(s * fr - m, 0):min((e_ + 1) * fr + m, len(a))] for s, e_ in runs if (e_ - s + 1) * 10 >= 150]


def cmd_gen():
    import torch, soundfile as sf
    import perth; perth.PerthImplicitWatermarker = perth.DummyWatermarker
    from chatterbox.tts import ChatterboxTTS
    torch.set_num_threads(int(os.environ.get("THREADS", "6")))
    man = json.loads(MAN.read_text()) if MAN.exists() else {}
    m = ChatterboxTTS.from_pretrained(device="cpu")
    ear = subprocess.Popen([str(PY["fw"]), "-I", __file__, "hear"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

    def hear(path):
        ear.stdin.write(str(path) + "\n"); ear.stdin.flush()
        return json.loads(ear.stdout.readline())

    def say(line, voice, seed):
        torch.manual_seed(seed)
        with torch.inference_mode():
            return m.generate(line, audio_prompt_path=str(OUT / f"ref-{voice}.wav"), exaggeration=EXAGGERATION, cfg_weight=CFG)[0].numpy()

    t_all = time.time()
    for key, old in lines().items():
        cur = man.get(key, {})
        if cur.get("ok") or cur.get("rep") == TRIES or (cur.get("tries") == TRIES and len(old["text"].split()) > 1):
            continue
        text, voice = old["text"], old["voice"]
        out = OUT / f"{voice}-{slug(text)}.wav"
        line = text if text[-1:] in ".!?" else text + "."
        ok, got, seed, rep = False, cur.get("heard", ""), cur.get("seed", 0), 0
        if cur.get("tries") != TRIES:
            for seed in range(1, TRIES + 1):
                t = time.time()
                a = say(line, voice, seed)
                sf.write(out, trim(a), SR)
                got = hear(out)
                ok = norm(got) == norm(text)
                print(f"{key} seed {seed}: heard '{got}' {'ok' if ok else 'RETAKE'} ({time.time() - t:.0f} s, {len(a) / SR:.1f} s audio)", flush=True)
                if ok:
                    break
        # A lone word is the hardest case for Chatterbox (the first run heard 'dry' as 'Durot', 'Grigh', 'Try!' on all
        # four seeds each voice): it gets the word said REPEAT times as sentences, each said word is cut out at its
        # pause and heard alone, and the first one Whisper hears right is the clip.
        if not ok and len(text.split()) == 1:
            for rep in range(1, TRIES + 1):
                t = time.time()
                a = say(" ".join([line] * REPEAT), voice, 100 + rep)
                heard = []
                for piece in islands(a):
                    sf.write(out, piece, SR)
                    got = hear(out)
                    heard.append(got)
                    if norm(got) == norm(text):
                        ok = True
                        break
                print(f"{key} repeat seed {100 + rep}: heard {heard} {'ok' if ok else 'RETAKE'} ({time.time() - t:.0f} s)", flush=True)
                if ok:
                    seed = 100 + rep
                    break
        man[key] = {"text": text, "voice": voice, "file": out.name, "heard": got, "ok": ok, "seed": seed, "tries": TRIES if rep else seed}
        if rep:
            man[key]["rep"] = rep
        MAN.write_text(json.dumps(man, indent=1) + "\n")
    ear.stdin.close()
    bad = [k for k, v in man.items() if not v["ok"]]
    print(f"all {time.time() - t_all:.0f} s; {len(man)} clips; still the old voice: {bad or 'none'}", flush=True)


def cmd_all():
    subprocess.run([str(PY["kokoro"]), "-I", __file__, "refs"], check=True)
    subprocess.run([str(PY["cb"]), "-I", __file__, "gen"], check=True)


def to_mp3(wav, out):
    """As tools/voices.py: silence cut at both ends, loudness -16 LUFS, mono 24 kHz mp3 64 kbps (no stretch: the
    slow reference already makes the pace)."""
    tr = "silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.05"
    af = f"{tr},areverse,{tr},areverse,loudnorm=I=-16:TP=-1.5:LRA=11,adelay=40"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(wav), "-af", af, "-ac", "1", "-ar", "24000", "-b:a", "64k", str(out)], check=True)


def cmd_embed():
    """Swaps each passing chain clip into index.html's CLIPS block, same key; failing ones keep their old clip."""
    man = json.loads(MAN.read_text())
    src = (ROOT / "index.html").read_text()
    a, b = src.index("/* CLIPS:BEGIN */"), src.index("/* CLIPS:END */")
    block = src[a:b]
    clips = json.loads(block[block.index("{"):block.rindex("}") + 1])
    done, kept = [], []
    with tempfile.TemporaryDirectory() as d:
        for k in clips:
            if man.get(k, {}).get("ok"):
                mp3 = Path(d) / "c.mp3"
                to_mp3(OUT / man[k]["file"], mp3)
                clips[k] = "data:audio/mpeg;base64," + base64.b64encode(mp3.read_bytes()).decode()
                done.append(k)
            else:
                kept.append(k)
    head = "/* CLIPS:BEGIN */\n/* voice chain: kokoro+chatterbox */\n"
    src = src[:a] + head + "const CLIPS = " + json.dumps(clips, indent=0) + ";\n" + src[b:]
    (ROOT / "index.html").write_text(src)
    print(f"index.html: {len(done)} chain clips, old voice kept: {kept or 'none'}")


if __name__ == "__main__":
    {"refs": cmd_refs, "hear": cmd_hear, "gen": cmd_gen, "all": cmd_all, "embed": cmd_embed}[sys.argv[1]]()
