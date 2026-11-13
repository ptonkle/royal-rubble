#!/usr/bin/env python3
"""Compile the sound recipes in data/sounds.json into the schedules the page's
synth plays (web/synth.js) and tools/sound/render.py renders.

    python3 tools/sound/compile.py

Writes data/sound-programs.json (every cue, loop and music track as layers of
AudioParam schedules, times in seconds, every value final) and
data/sound-noise.wav (one second of seeded white noise, the only buffer the
synth uses). Nothing is recorded: every sound is built from these recipes.

Ported from Fleeced, where the core compiles the recipes (crates/core/src/
sound.rs). Here the compile is a build step, so the sound work stays out of the
core while the physics is being written; the page still holds no numbers.

A layer recipe (times in ms, frequencies in Hz, gains 0 to 1):
  wave            sine | square | sawtooth | triangle | noise | periodic
  harmonics       for periodic: the sine amplitudes of partials 1, 2, 3 ...
  at_ms           when it starts
  freq, to_freq   pitch, and where it glides to (glide_ms, glide: linear)
  bends           [[ms, hz], ...] linear pitch moves after the start
  attack_ms, hold_ms, release_ms, gain    the level's shape
  strikes         [[ms, level], ...] repeated hits that each ring down over
                  ring_ms, then tail_ms of silence (replaces the shape above)
  steady          a loop's bed: plays at a constant level for as long as the
                  loop plays, built once (no seam)
  filters         [{type, freq, to_freq, q, gain}]
  wobble          [{rate, cents}] slow pitch wobble; throb [{rate, depth}]
In music, an instrument layer takes `ratio` (and `to_ratio`) times the note's
pitch instead of freq, a filter may take `ratio` too, and hold_ms "note"
holds for the note's length.
"""
import json
import math
import pathlib
import struct
import wave

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "data" / "sounds.json"
OUT = ROOT / "data" / "sound-programs.json"
NOISE = ROOT / "data" / "sound-noise.wav"


def r(x):
    """Seconds or values, rounded so the JSON stays small and stable."""
    return round(x, 5)


class Rng:
    """A small seeded generator, so a compile is the same every time."""

    def __init__(self, seed):
        self.s = seed & 0xFFFFFFFF or 1

    def next(self):
        x = self.s
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        self.s = x
        return x / 0x100000000


def curve_point(m, t_ms, v, tc_ms=None):
    p = {"m": m, "t": r(t_ms / 1000), "v": r(v)}
    if tc_ms is not None:
        p["tc"] = r(tc_ms / 1000)
    return p


def compile_layer(l, silence, note_hz=None, note_ms=None, vel=1.0, offset_ms=0.0):
    """One layer recipe as a program layer. Returns (layer, end_ms)."""
    start = offset_ms + l.get("at_ms", 0)
    peak = l["gain"] * vel
    gain = []
    if l.get("steady"):
        gain.append(curve_point("setValueAtTime", start, peak))
        end = start
    elif "strikes" in l:
        last = start
        for ms, level in l["strikes"]:
            last = start + ms
            gain.append(curve_point("setValueAtTime", last, peak * level))
            gain.append(curve_point("setTargetAtTime", last, 0, l["ring_ms"]))
        end = last + l.get("tail_ms", l["ring_ms"] * 4)
    else:
        attack = l.get("attack_ms", 2)
        hold = l.get("hold_ms", 0)
        if hold == "note":
            hold = max(0, note_ms - attack - l.get("gap_ms", 0))
        release = l.get("release_ms", 40)
        end = start + attack + hold + release
        gain.append(curve_point("setValueAtTime", start, 0))
        gain.append(curve_point("linearRampToValueAtTime", start + attack, peak))
        gain.append(curve_point("setValueAtTime", start + attack + hold, peak))
        gain.append(curve_point("exponentialRampToValueAtTime", end, min(silence, max(peak, silence / 10))))
    out = {"wave": l["wave"], "start": r(start / 1000), "stop": r(end / 1000), "gain": gain}
    if l.get("steady"):
        out["steady"] = True
    if l["wave"] == "periodic":
        h = l["harmonics"]
        out["periodic"] = {"real": [0.0] * (len(h) + 1), "imag": [0.0] + [r(a) for a in h]}
    if l["wave"] != "noise":
        hz = l["ratio"] * note_hz if "ratio" in l else l["freq"]
        freq = [curve_point("setValueAtTime", start, hz)]
        to = l.get("to_ratio", 0) * note_hz if "to_ratio" in l else l.get("to_freq")
        if to:
            bend = start + l.get("glide_ms", end - start)
            m = "linearRampToValueAtTime" if l.get("glide") == "linear" else "exponentialRampToValueAtTime"
            freq.append(curve_point(m, bend, to))
        for ms, bhz in l.get("bends", []):
            freq.append(curve_point("linearRampToValueAtTime", start + ms, bhz))
        out["freq"] = freq
    filters = []
    for f in l.get("filters", []):
        base = f["ratio"] * note_hz if "ratio" in f else f["freq"]
        fc = [curve_point("setValueAtTime", start, base)]
        if "to_freq" in f or "to_ratio" in f:
            to = f["to_ratio"] * note_hz if "to_ratio" in f else f["to_freq"]
            fc.append(curve_point("exponentialRampToValueAtTime", start + f.get("sweep_ms", end - start), to))
        filters.append({"type": f["type"], "freq": fc, "q": f.get("q", 0.7071), "gain": f.get("gain", 0)})
    out["filters"] = filters
    out["wobble"] = l.get("wobble", [])
    out["throb"] = l.get("throb", [])
    return out, end


def compile_cue(c, silence, rng):
    layers, length = [], 0
    for l in c["layers"]:
        layer, end = compile_layer(l, silence)
        layers.append(layer)
        length = max(length, end)
    p = {"id": c["id"], "kind": c.get("kind", "cue"), "length": r(length / 1000),
         "noise_at": r(rng.next() * 0.9), "layers": layers}
    if c.get("kind") == "loop":
        p["length"] = r(c["length_ms"] / 1000)
    return p


# ---- music ----

def note_hz(score, degree, octave_shift=0):
    mode = score["mode"]
    octave, step = divmod(degree, len(mode))
    semis = mode[step] + 12 * (octave + octave_shift)
    return score["root_hz"] * 2 ** (semis / 12)


def parse_notes(text):
    """'4:1 3:.5 r:2' as [(degree or None, beats)]."""
    out = []
    for tok in text.split():
        d, b = tok.split(":")
        out.append((None if d == "r" else int(d), float(b)))
    return out


def compile_music(m, instruments, silence):
    beat_ms = 60000 / m["bpm"]
    bar = m["beats_per_bar"]
    rng = Rng(m.get("seed", 1))
    human = m.get("humanize_ms", 0)
    sections = m["form"]
    length_ms = 0
    events = []  # (instrument, start_ms, hz, note_ms, vel)

    def jitter():
        return rng.next() * human

    t0 = 0.0
    for name in sections:
        sec_beats = m["section_bars"] * bar
        for part in m["parts"]:
            body = part["sections"].get(name)
            if body is None:
                continue
            inst = part["instrument"]
            octv = part.get("octave", 0)
            vel = part.get("gain", 1.0) * (body.get("gain", 1.0) if isinstance(body, dict) else 1.0)
            if part["type"] == "notes":
                t = 0.0
                for d, beats in parse_notes(body["notes"]):
                    if d is not None:
                        events.append((inst, t0 + t * beat_ms + jitter(), note_hz(m, d, octv), beats * beat_ms, vel))
                    t += beats
                assert abs(t - sec_beats) < 1e-6, f"{m['id']} {name} {inst}: {t} beats, want {sec_beats}"
            elif part["type"] == "arp":
                step = part["step_beats"]
                per_bar = round(bar / step)
                for b, chord in enumerate(body["chords"]):
                    for k in range(per_bar):
                        pat = body.get("pattern", part["pattern"])
                        off = pat[k % len(pat)]
                        if off is None:
                            continue
                        t = (b * bar + k * step) * beat_ms
                        accent = 1.0 if k == 0 else part.get("soft", 0.7)
                        events.append((inst, t0 + t + jitter(), note_hz(m, chord + off, octv), step * beat_ms * part.get("ring", 2), vel * accent))
            elif part["type"] == "drum":
                step = part["step_beats"]
                pats = body["bars"]
                for b in range(m["section_bars"]):
                    pat = pats[b % len(pats)]
                    for k, ch in enumerate(pat):
                        if ch == ".":
                            continue
                        level = 1.0 if ch == "X" else part.get("soft", 0.55)
                        t = (b * bar + k * step) * beat_ms
                        events.append((inst, t0 + t + jitter(), part.get("hz", 0), step * beat_ms, vel * level))
            elif part["type"] == "held":
                for b, chord in enumerate(body["chords"]):
                    span = body.get("bars_each", 1)
                    for d in body.get("voicing", [0, 2, 4]):
                        events.append((inst, t0 + b * span * bar * beat_ms, note_hz(m, chord + d, octv), span * bar * beat_ms, vel))
        t0 += sec_beats * beat_ms
    length_ms = t0

    layers = []
    for d in m.get("drones", []):
        inst = instruments[d["instrument"]]
        hz = note_hz(m, d["degree"], d.get("octave", 0))
        for l in inst["layers"]:
            layer, _ = compile_layer({**l, "steady": True}, silence, note_hz=hz, vel=d.get("gain", 1.0))
            layer["stop"] = r(length_ms / 1000)
            layers.append(layer)
    events.sort(key=lambda e: e[1])
    for inst, start, hz, note_ms, vel in events:
        for l in instruments[inst]["layers"]:
            layer, _ = compile_layer(l, silence, note_hz=hz, note_ms=note_ms, vel=vel, offset_ms=start)
            layers.append(layer)
    layers.sort(key=lambda la: la["start"])
    return {"id": m["id"], "kind": "music", "length": r(length_ms / 1000), "noise_at": 0.0,
            "layers": layers}


def write_noise(spec):
    rng = Rng(spec["seed"])
    frames = b"".join(struct.pack("<h", int((rng.next() * 2 - 1) * 32000)) for _ in range(spec["samples"]))
    with wave.open(str(NOISE), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(spec["rate"])
        w.writeframes(frames)
    return [s[0] / 32768 for s in struct.iter_unpack("<h", frames)]


def compile_all():
    data = json.loads(SRC.read_text())
    silence = data["silence"]
    rng = Rng(data["noise"]["seed"] + 1)
    cues = [compile_cue(c, silence, rng) for c in data["cues"]]
    music = [compile_music(m, data["instruments"], silence) for m in data["music"]]
    return data, {
        "_note": "Generated by tools/sound/compile.py from data/sounds.json. Do not edit.",
        "setup": data["setup"],
        "groups": data["groups"],
        "tiers": data["tiers"],
        "labels": {c["id"]: c["label"] for c in data["cues"] + data["music"]},
        "sections": data["sections"],
        "programs": cues + music,
    }


def main():
    data, programs = compile_all()
    write_noise(data["noise"])
    OUT.write_text(json.dumps(programs, separators=(",", ":")) + "\n")
    n = sum(len(p["layers"]) for p in programs["programs"])
    print(f"{OUT.relative_to(ROOT)}: {len(programs['programs'])} programs, {n} layers, {OUT.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
