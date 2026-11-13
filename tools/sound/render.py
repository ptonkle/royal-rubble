#!/usr/bin/env python3
"""Render the game's synthesized sounds to WAV files, offline, so a person can
listen without running the game.

    python3 tools/sound/render.py                 every cue, loop and track, to notes/sound/wav/
    python3 tools/sound/render.py impact          the sounds whose ids hold this word
    python3 tools/sound/render.py --seconds 10 music_build    the first ten seconds

Ported from Fleeced. This is the page's synth (web/sound/synth.js) over again,
in plain Python with no installs. It decides nothing: the schedules come from
data/sound-programs.json (tools/sound/compile.py), and each stage follows the
Web Audio definition: AudioParam ramps, oscillators with periodic waves, the
biquad formulas of the specification, and the same chain order (source,
filters, throb, gain). A loop or music track is rendered once through, with
the tails that run past its end wrapped round to its start, as it sounds when
it repeats.

The output is mono, 16-bit, 44.1 kHz, at the level the test page plays (the
main volume at full, before the game's master_scale).
"""
import argparse
import json
import math
import multiprocessing
import pathlib
import wave

ROOT = pathlib.Path(__file__).resolve().parents[2]
RATE = 44100
TABLE = 2048
BLOCK = 64


def load():
    return json.loads((ROOT / "data" / "sound-programs.json").read_text())


def noise_samples():
    with wave.open(str(ROOT / "data" / "sound-noise.wav")) as w:
        raw = w.readframes(w.getnframes())
    return [int.from_bytes(raw[i:i + 2], "little", signed=True) / 32768 for i in range(0, len(raw), 2)]


def param(schedule, n, default=0.0):
    """An AudioParam's value at each of n samples, from its schedule."""
    out = [default] * n
    value = default
    i = 0
    settling = None

    def settle(until):
        nonlocal value, i, settling
        goal, keep = settling
        gap = value - goal
        out[i:until] = [goal + gap * keep**j for j in range(until - i)]
        value = goal + gap * keep ** (until - i)
        i = until
        settling = None

    for p in schedule:
        at = min(n, round(p["t"] * RATE))
        kind = p["m"]
        if settling:
            settle(at)
        if kind == "setTargetAtTime":
            out[i:at] = [value] * (at - i)
            i = at
            settling = (p["v"], math.exp(-1 / (p["tc"] * RATE)))
            continue
        if kind == "setValueAtTime":
            out[i:at] = [value] * (at - i)
            value = p["v"]
        else:
            span = max(1, at - i)
            start = value
            if kind == "linearRampToValueAtTime":
                step = (p["v"] - start) / span
                out[i:at] = [start + step * j for j in range(at - i)]
            else:
                ratio = (p["v"] / start) ** (1 / span) if start > 0 else 1.0
                out[i:at] = [start * ratio**j for j in range(at - i)]
            value = p["v"]
        i = at
    if settling:
        settle(n)
    out[i:n] = [value] * (n - i)
    return out


def steps_of(schedule):
    """A frequency schedule made only of setValueAtTime, as (sample, value)."""
    return [(round(p["t"] * RATE), p["v"]) for p in schedule]


def table(periodic, hz):
    """One cycle of a periodic wave, keeping the partials below Nyquist as the
    browser does."""
    amps = periodic["imag"][1:]
    top = min(len(amps), int(RATE / 2 / max(hz, 1)))
    out = [0.0] * (TABLE + 1)
    for n in range(1, top + 1):
        a = amps[n - 1]
        if a == 0:
            continue
        w = 2 * math.pi * n / TABLE
        s1 = math.sin(w)
        c1 = math.cos(w)
        s, c = 0.0, 1.0
        for i in range(TABLE):
            out[i] += a * s
            s, c = s * c1 + c * s1, c * c1 - s * s1
    out[TABLE] = out[0]
    return out


def fourier(kind):
    """The sine series the Web Audio specification gives a basic wave, each
    starting at phase zero. Browsers scale a full-band saw or square to a peak
    of 1, and its full-band peak, with the Gibbs overshoot, is about 1.179."""
    count = TABLE // 2 - 1
    if kind == "sine":
        return [0.0, 1.0]
    if kind == "sawtooth":
        return [0.0] + [(-1) ** (k + 1) * 2 / (k * math.pi) / 1.179 for k in range(1, count + 1)]
    if kind == "square":
        return [0.0] + [(4 / (k * math.pi) if k % 2 else 0.0) / 1.179 for k in range(1, count + 1)]
    return [0.0] + [8 * math.sin(k * math.pi / 2) / (k * math.pi) ** 2 for k in range(1, count + 1)]


BASIC = {}


def basic(kind, hz):
    """One cycle of a basic wave at this pitch: band-limited, in three ranges
    to the octave, as browsers build them."""
    top = max(1, int(RATE / 2 / max(hz, 1)))
    rung = int(2 ** (math.floor(3 * math.log2(top)) / 3))
    key = (kind, rung)
    if key not in BASIC:
        BASIC[key] = table({"imag": fourier(kind)}, RATE / 2 / rung)
    return BASIC[key]


def detune_blocks(layer, n, schedule):
    """Pitch ratio per block: the detune schedule and the wobble."""
    base = param(schedule, n, 0.0)[::BLOCK]
    wob = layer.get("wobble") or []
    out = []
    for k, cents in enumerate(base):
        t = k * BLOCK / RATE - layer["start"]
        c = cents
        for w in wob:
            c += w["cents"] * math.sin(2 * math.pi * w["rate"] * t)
        out.append(2 ** (c / 1200))
    return out + [out[-1] if out else 1.0]


def oscillator(layer, n):
    freq = layer["freq"]
    ratio = detune_blocks(layer, n, layer.get("detune") or [])
    begin = round(layer["start"] * RATE)
    end = min(n, round(layer["stop"] * RATE))
    out = [0.0] * n
    tables = {}
    stepped = all(p["m"] == "setValueAtTime" for p in freq)
    if stepped:
        pieces = steps_of(freq)
        pieces.append((end, pieces[-1][1]))
    else:
        curve = param(freq, n)
        pieces = [(i, curve[i]) for i in range(begin, end, BLOCK)] + [(end, curve[max(0, end - 1)])]
    phase = 0.0
    scale = TABLE / RATE
    for (a, hz), (b, _) in zip(pieces, pieces[1:]):
        b = min(b, end)
        if b <= a:
            continue
        if layer["wave"] == "periodic":
            key = round(hz)
            if key not in tables:
                tables[key] = table(layer["periodic"], hz)
            wave_table = tables[key]
        else:
            wave_table = basic(layer["wave"], hz)
        i = a
        while i < b:
            stop = min(b, (i // BLOCK + 1) * BLOCK)
            step = hz * ratio[i // BLOCK] * scale
            for j in range(i, stop):
                k = int(phase)
                f = phase - k
                x = wave_table[k]
                out[j] = x + (wave_table[k + 1] - x) * f
                phase += step
                if phase >= TABLE:
                    phase -= TABLE
            i = stop
    return out


def noise(layer, n, samples):
    begin = round(layer["start"] * RATE)
    end = min(n, round(layer["stop"] * RATE))
    size = len(samples)
    out = [0.0] * n
    for i in range(begin, end):
        out[i] = samples[(i - begin) % size]
    return out


def coefficients(kind, f0, q, gain):
    """The Web Audio biquad formulas."""
    w = 2 * math.pi * min(f0, RATE / 2 - 1) / RATE
    cs = math.cos(w)
    sn = math.sin(w)
    a_db = 10 ** (gain / 40)
    a_q = sn / (2 * max(q, 1e-4))
    a_q_db = sn / (2 * 10 ** (q / 20))
    if kind == "lowpass":
        b0, b1, b2 = (1 - cs) / 2, 1 - cs, (1 - cs) / 2
        a0, a1, a2 = 1 + a_q_db, -2 * cs, 1 - a_q_db
    elif kind == "highpass":
        b0, b1, b2 = (1 + cs) / 2, -(1 + cs), (1 + cs) / 2
        a0, a1, a2 = 1 + a_q_db, -2 * cs, 1 - a_q_db
    elif kind == "bandpass":
        b0, b1, b2 = a_q, 0.0, -a_q
        a0, a1, a2 = 1 + a_q, -2 * cs, 1 - a_q
    elif kind == "peaking":
        b0, b1, b2 = 1 + a_q * a_db, -2 * cs, 1 - a_q * a_db
        a0, a1, a2 = 1 + a_q / a_db, -2 * cs, 1 - a_q / a_db
    elif kind == "notch":
        b0, b1, b2 = 1.0, -2 * cs, 1.0
        a0, a1, a2 = 1 + a_q, -2 * cs, 1 - a_q
    elif kind == "highshelf":
        a = a_db
        root = 2 * math.sqrt(a) * sn / math.sqrt(2)
        b0 = a * ((a + 1) + (a - 1) * cs + root)
        b1 = -2 * a * ((a - 1) + (a + 1) * cs)
        b2 = a * ((a + 1) + (a - 1) * cs - root)
        a0 = (a + 1) - (a - 1) * cs + root
        a1 = 2 * ((a - 1) - (a + 1) * cs)
        a2 = (a + 1) - (a - 1) * cs - root
    elif kind == "lowshelf":
        a = a_db
        root = 2 * math.sqrt(a) * sn / math.sqrt(2)
        b0 = a * ((a + 1) - (a - 1) * cs + root)
        b1 = 2 * a * ((a - 1) - (a + 1) * cs)
        b2 = a * ((a + 1) - (a - 1) * cs - root)
        a0 = (a + 1) + (a - 1) * cs + root
        a1 = -2 * ((a - 1) + (a + 1) * cs)
        a2 = (a + 1) + (a - 1) * cs - root
    else:
        raise ValueError(f"filter {kind}")
    return b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0


def biquad(signal, f):
    """One filter over the signal, in place; its frequency may sweep."""
    n = len(signal)
    kind, q, gain = f["type"], f["q"], f["gain"]
    swept = len(f["freq"]) > 1
    curve = param(f["freq"], n, f["freq"][0]["v"]) if swept else None
    x1 = x2 = y1 = y2 = 0.0
    b0, b1, b2, a1, a2 = coefficients(kind, f["freq"][0]["v"], q, gain)
    for i in range(0, n, BLOCK):
        if swept:
            b0, b1, b2, a1, a2 = coefficients(kind, curve[i], q, gain)
        for j in range(i, min(n, i + BLOCK)):
            x = signal[j]
            y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
            x2, x1, y2, y1 = x1, x, y1, y
            signal[j] = y
    return signal


def render_layer(layer, n, samples):
    if layer["wave"] == "noise":
        out = noise(layer, n, samples)
    else:
        out = oscillator(layer, n)
    for f in layer.get("filters") or []:
        out = biquad(out, f)
    for t in layer.get("throb") or []:
        w = 2 * math.pi * t["rate"] / RATE
        d = t["depth"]
        for i in range(n):
            out[i] *= 1 + d * math.sin(w * i)
    g = param(layer["gain"], n, 0.0)
    return [a * b for a, b in zip(out, g)]


def shifted(layer, by, stop):
    """The layer with every time moved earlier by `by` seconds."""

    def move(points):
        return [{**p, "t": p["t"] - by} for p in points]

    out = dict(layer)
    out["start"] = 0.0
    out["stop"] = stop - by
    out["gain"] = move(layer["gain"])
    if "freq" in layer:
        out["freq"] = move(layer["freq"])
    out["detune"] = move(layer.get("detune") or [])
    out["filters"] = [{**f, "freq": move(f["freq"])} for f in layer.get("filters") or []]
    return out


def render(program, samples, seconds=None):
    looped = program["kind"] in ("loop", "music")
    length = program["length"] if seconds is None else min(seconds, program["length"])
    n = round(length * RATE)
    mix = [0.0] * n
    for layer in program["layers"]:
        if layer["start"] >= length:
            continue
        stop = length if layer.get("steady") else layer["stop"]
        if not looped:
            stop = min(stop, length)
        local = shifted(layer, layer["start"], stop)
        begin = round(layer["start"] * RATE)
        m = max(1, round((stop - layer["start"]) * RATE))
        out = render_layer(local, m, samples)
        for j, x in enumerate(out):
            k = begin + j
            if k >= n:
                if not looped or seconds is not None:
                    break
                k %= n
            mix[k] += x
    return mix


def write(path, mix):
    peak = max((abs(x) for x in mix), default=0.0)
    rms = math.sqrt(sum(x * x for x in mix) / max(1, len(mix)))
    frames = bytearray(len(mix) * 2)
    for i, x in enumerate(mix):
        s = max(-32767, min(32767, round(x * 32767)))
        frames[2 * i] = s & 255
        frames[2 * i + 1] = (s >> 8) & 255
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(bytes(frames))
    clip = "  CLIPS" if peak > 1 else ""
    return f"{path.relative_to(ROOT)}: {len(mix) / RATE:.1f} s, peak {peak:.2f}, rms {rms:.3f}{clip}"


def job(args):
    program, out, seconds = args
    return write(out / f"{program['id']}.wav", render(program, noise_samples(), seconds))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("words", nargs="*")
    ap.add_argument("--seconds", type=float)
    ap.add_argument("--out", default="notes/sound/wav")
    args = ap.parse_args()
    out = ROOT / args.out
    out.mkdir(parents=True, exist_ok=True)
    picked = [p for p in load()["programs"] if not args.words or any(w in p["id"] for w in args.words)]
    picked.sort(key=lambda p: -len(p["layers"]))
    with multiprocessing.Pool() as pool:
        for line in pool.imap_unordered(job, [(p, out, args.seconds) for p in picked]):
            print(line, flush=True)


if __name__ == "__main__":
    main()
