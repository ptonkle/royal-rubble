#!/usr/bin/env python3
"""Write data/sounds.json: every sound recipe and both music tracks.

    python3 tools/sound/author.py && python3 tools/sound/compile.py

Recipes are written here as Python so repeated shapes (a trumpet note, a
struck partial) stay short; the JSON it writes is what the compile reads.
Edit here, not in the JSON. See tools/sound/compile.py for the recipe fields.
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
SILENCE = 0.001


# ---- shapes ----

def noise(gain, filters, **kw):
    return {"wave": "noise", "gain": gain, "filters": filters, **kw}


def bp(freq, q=1.0, **kw):
    return {"type": "bandpass", "freq": freq, "q": q, **kw}


def lp(freq, q=0.7071, **kw):
    return {"type": "lowpass", "freq": freq, "q": q, **kw}


def hp(freq, q=0.7071, **kw):
    return {"type": "highpass", "freq": freq, "q": q, **kw}


def tone(wave, freq, gain, release, to=None, glide=None, at=0, attack=2, hold=0, **kw):
    l = {"wave": wave, "freq": freq, "gain": gain, "at_ms": at, "attack_ms": attack, "hold_ms": hold, "release_ms": release, **kw}
    if to:
        l["to_freq"] = to
    if glide:
        l["glide_ms"] = glide
    return l


def struck(wave_or_noise, gain, strikes, ring, freq=None, filters=None, at=0, tail=None):
    l = {"wave": wave_or_noise, "gain": gain, "strikes": strikes, "ring_ms": ring, "at_ms": at, "filters": filters or []}
    if freq:
        l["freq"] = freq
    if tail:
        l["tail_ms"] = tail
    return l


def trumpet(freq, at, hold, gain=0.09, release=140):
    return {"wave": "sawtooth", "freq": freq, "gain": gain, "at_ms": at, "attack_ms": 22, "hold_ms": hold,
            "release_ms": release, "filters": [lp(2600, 1.2)], "wobble": [{"rate": 5.5, "cents": 7}]}


def horn(freq, at, hold, gain=0.11, release=380):
    return {"wave": "sawtooth", "freq": freq, "gain": gain, "at_ms": at, "attack_ms": 70, "hold_ms": hold,
            "release_ms": release, "filters": [lp(1100, 1.5)], "wobble": [{"rate": 4.8, "cents": 9}]}


def metal(base, gain, ring, at=0):
    """An inharmonic clang: partials of a struck bar."""
    parts = [(1.0, 1.0), (2.76, 0.6), (5.40, 0.4), (8.93, 0.22)]
    return [struck("sine", gain * g, [[0, 1]], ring / (1 + k * 0.6), freq=round(base * ratio, 1), at=at)
            for k, (ratio, g) in enumerate(parts)]


def scatter(n, every, start=0, fall=0.85, wobble=(1.0, 0.55, 0.8, 0.4, 0.7)):
    """n hits, spaced unevenly, getting quieter."""
    out, t, level = [], start, 1.0
    for k in range(n):
        out.append([round(t), round(level * wobble[k % len(wobble)], 3)])
        t += every * (0.6 + 0.8 * ((k * 7) % 5) / 4)
        level *= fall
    return out


C = []


def cue(id, label, *layers, **kw):
    C.append({"id": id, "label": label, "layers": [l for x in layers for l in (x if isinstance(x, list) else [x])], **kw})


# ---- trebuchet ----
cue("trebuchet_creak", "Trebuchet wind-up",
    tone("sawtooth", 82, 0.22, 220, to=64, attack=60, hold=700, filters=[bp(650, 4)], throb=[{"rate": 13, "depth": 0.9}]),
    struck("noise", 0.3, [[t, 1 if t % 160 == 0 else 0.6] for t in range(0, 900, 80)], 10, filters=[bp(2400, 3)]))
cue("trebuchet_release", "Trebuchet release",
    tone("sine", 115, 0.55, 260, to=42, hold=30),
    struck("noise", 0.35, [[0, 1], [22, 0.4]], 45, filters=[lp(700)]),
    tone("triangle", 330, 0.22, 120, to=190),
    tone("sawtooth", 176, 0.07, 420, to=171, at=10, filters=[lp(900)]))
cue("shot_whoosh", "Shot in flight", kind="loop", length_ms=2000,
    layers=[noise(0.07, [bp(520, 1.3)], steady=True, throb=[{"rate": 2.6, "depth": 0.5}]),
            noise(0.025, [bp(1500, 2)], steady=True, throb=[{"rate": 4.1, "depth": 0.6}])])
C[-1]["layers"] = C[-1].pop("layers")  # keep kind/length beside the layers

# ---- impacts ----
for i, (band, thud, crack) in enumerate([(1800, 140, 900), (2300, 120, 1100), (1500, 160, 760)], 1):
    cue(f"impact_wood_{i}", f"Wood hit {i}",
        struck("noise", 0.5, scatter(5, 22), 24, filters=[bp(band, 1.6)]),
        tone("triangle", thud, 0.32, 130, to=thud // 2),
        tone("square", crack, 0.045, 35, to=crack // 2, filters=[lp(3000)]))
for i, (low, thud) in enumerate([(1100, 80), (900, 70), (1300, 92)], 1):
    cue(f"impact_stone_{i}", f"Stone hit {i}",
        struck("noise", 0.55, scatter(8, 20, fall=0.8), 28, filters=[lp(low)]),
        struck("noise", 0.18, scatter(6, 31, start=8), 9, filters=[bp(3200, 1.5)]),
        tone("sine", thud, 0.5, 230, to=thud // 2))
for i, base in enumerate([420, 510, 365], 1):
    cue(f"impact_metal_{i}", f"Metal clang {i}",
        metal(base, 0.2, 380),
        struck("noise", 0.2, [[0, 1]], 10, filters=[hp(3000)]))
cue("segment_snap", "Segment snaps",
    tone("sawtooth", 140, 0.05, 60, to=120, attack=40, hold=60, filters=[bp(800, 5)]),
    struck("noise", 0.55, [[100, 1], [108, 0.5]], 8, filters=[hp(2500)]),
    tone("square", 1400, 0.06, 45, to=300, at=100))
for name, label, hold, rel, gain, n in [("small", "Collapse (small)", 250, 600, 0.3, 6),
                                         ("mid", "Collapse (medium)", 650, 1000, 0.42, 12),
                                         ("big", "Collapse (big)", 1300, 1800, 0.52, 22)]:
    layers = [noise(gain, [lp(260, sweep_ms=hold + rel, to_freq=110)], attack_ms=25, hold_ms=hold, release_ms=rel,
                    throb=[{"rate": 8.5, "depth": 0.4}]),
              tone("sine", 58, gain * 0.9, rel // 2, to=34, hold=60),
              struck("noise", gain * 0.4, scatter(n, (hold + rel) / n / 1.4, start=40, fall=0.92), 18, filters=[bp(1500, 1.2)])]
    if name == "big":
        layers.append(tone("sine", 38, 0.35, 1500, hold=400, attack=80, to=30))
    cue(f"collapse_{name}", label, *layers)
for i, every in enumerate([28, 36], 1):
    cue(f"debris_{i}", f"Debris clatter {i}",
        struck("noise", 0.6, scatter(9, every, fall=0.86), 14, filters=[bp(2200 + 400 * i, 2)]),
        struck("noise", 0.35, scatter(5, every * 1.7, start=15), 22, filters=[bp(900, 2)]))

# ---- shots and air effects ----
cue("splash", "Splash",
    noise(0.42, [lp(3200, sweep_ms=650, to_freq=450)], attack_ms=6, hold_ms=60, release_ms=600),
    tone("sine", 280, 0.13, 70, to=900, glide=80, glide_kind=None),
    struck("noise", 0.12, scatter(7, 55, start=160), 9, filters=[bp(3600, 3)]),
    tone("sine", 70, 0.3, 240, to=40))
cue("explosion", "Explosion",
    noise(0.48, [lp(2600, sweep_ms=1400, to_freq=140)], attack_ms=3, hold_ms=80, release_ms=1400),
    tone("sine", 72, 0.42, 700, to=30, hold=40),
    struck("noise", 0.22, scatter(10, 70, start=60, fall=0.84), 12, filters=[hp(2000)]))
cue("fire_ignite", "Fire catches",
    noise(0.32, [bp(380, 1.2, to_freq=1500, sweep_ms=380)], attack_ms=90, hold_ms=100, release_ms=520),
    tone("sine", 95, 0.18, 300, to=150, attack=60))
cue("fire_crackle", "Fire crackle (loop)", kind="loop", length_ms=3000,
    layers=[noise(0.05, [lp(650)], steady=True, throb=[{"rate": 1.3, "depth": 0.6}]),
            struck("noise", 0.3, [[40, 1], [260, 0.5], [330, 0.8], [910, 0.6], [1180, 1], [1240, 0.4],
                                  [1700, 0.7], [2150, 0.9], [2480, 0.5], [2810, 0.7]], 6, filters=[bp(2800, 1)]),
            struck("noise", 0.12, [[150, 1], [700, 0.7], [1450, 0.9], [2000, 0.6], [2600, 0.8]], 14, filters=[bp(1200, 1.5)])])
cue("phase_shimmer", "Phase",
    [dict(tone("sine", f, 0.055, 520, to=f * 1.5, attack=150, hold=150), wobble=[{"rate": 7, "cents": 30}])
     for f in (880, 1320, 1760)])
cue("split_pop", "Split",
    tone("sine", 600, 0.25, 90, to=1400, glide=40),
    struck("noise", 0.2, [[0, 1]], 18, filters=[hp(1800)]),
    tone("sine", 900, 0.1, 90, to=700, at=30))
cue("dive_whistle", "Dive",
    {**tone("sine", 2300, 0.11, 80, to=520, attack=20, hold=650, glide=730), "glide": "linear",
     "wobble": [{"rate": 9, "cents": 14}]},
    noise(0.05, [bp(1400, 2, to_freq=500, sweep_ms=750)], attack_ms=40, hold_ms=600, release_ms=120))
cue("air_drop", "Drop",
    tone("sawtooth", 420, 0.08, 120, to=70, attack=10, hold=320, glide=430, filters=[lp(900)]),
    noise(0.12, [bp(900, 1.5, to_freq=200, sweep_ms=450)], attack_ms=10, hold_ms=320, release_ms=120))
cue("air_reverse", "Reverse",
    {**tone("sine", 320, 0.12, 100, attack=15, hold=420), "bends": [[220, 900], [440, 300]]},
    noise(0.1, [bp(600, 1.5, to_freq=2000, sweep_ms=220)], attack_ms=15, hold_ms=420, release_ms=100))

# ---- royals ----
def yelp(f0, up, down, dur, formant, gain=0.16, at=0):
    return {"wave": "sawtooth", "freq": f0, "gain": gain, "at_ms": at, "attack_ms": 12, "hold_ms": dur, "release_ms": 70,
            "bends": [[dur * 0.3, up], [dur, down]], "filters": [bp(formant, 3.5), bp(formant * 2.6, 4)],
            "wobble": [{"rate": 11, "cents": 25}]}


def thud(at, gain=0.3):
    return [tone("sine", 95, gain, 140, to=50, at=at), struck("noise", gain * 0.6, [[0, 1]], 30, filters=[lp(500)], at=at)]


cue("royal_squire", "Squire falls", yelp(520, 700, 380, 180, 1100, 0.13), thud(200, 0.22))
cue("royal_knight", "Knight falls", yelp(260, 330, 190, 240, 700), thud(260), metal(620, 0.08, 160, at=260))
cue("royal_princess", "Princess falls", yelp(780, 1100, 640, 300, 1400, 0.12), thud(330, 0.22),
    [struck("sine", 0.05, [[0, 1]], 300, freq=f, at=340 + k * 70) for k, f in enumerate((1568, 2093))])
cue("royal_prince", "Prince falls", yelp(330, 450, 240, 320, 850), thud(350),
    [struck("sine", 0.05, [[0, 1]], 300, freq=f, at=360 + k * 70) for k, f in enumerate((1175, 1568))])
cue("royal_queen", "Queen falls", yelp(440, 620, 330, 420, 1000), thud(450),
    [struck("sine", 0.06, [[0, 1]], 500, freq=f, at=470 + k * 90) for k, f in enumerate((1047, 1319, 1568))])
cue("royal_king", "King falls", yelp(190, 260, 140, 420, 620, 0.18), thud(450, 0.35),
    trumpet(523, 520, 70), trumpet(523, 640, 70), trumpet(392, 760, 420, release=380),
    trumpet(659, 520, 70, gain=0.05), trumpet(659, 640, 70, gain=0.05), trumpet(494, 760, 420, gain=0.05, release=380))

# ---- building and gold ----
cue("place_segment", "Place segment",
    tone("triangle", 620, 0.2, 70, to=430),
    struck("noise", 0.14, [[0, 1]], 8, filters=[bp(2100, 2)]))
cue("sell_segment", "Sell segment",
    noise(0.06, [bp(1800, 1.5, to_freq=600, sweep_ms=140)], attack_ms=80, release_ms=60),
    struck("sine", 0.07, [[150, 1]], 110, freq=1976), struck("sine", 0.06, [[210, 1]], 160, freq=2637))
cue("coin", "Gold",
    tone("square", 988, 0.05, 30, hold=60, filters=[lp(4500)]),
    tone("square", 1319, 0.05, 320, at=75, filters=[lp(4500)]),
    struck("sine", 0.04, [[75, 1]], 220, freq=2637))
cue("buy_xp", "Buy XP",
    [tone("sine", f, 0.08, 220, at=k * 55) for k, f in enumerate((660, 880, 1100, 1320))],
    noise(0.03, [bp(4000, 2)], attack_ms=100, release_ms=200))
cue("level_up", "Level up",
    [tone("triangle", f, 0.13, 160 if k < 3 else 650, at=k * 95, hold=60 if k < 3 else 300) for k, f in enumerate((523, 659, 784, 1047))],
    [struck("sine", 0.035, [[0, 1]], 260, freq=f, at=300 + k * 60) for k, f in enumerate((2093, 2637, 3136))])
cue("cards_reveal", "Upgrade cards appear",
    [noise(0.1, [bp(1200, 1.5, to_freq=3200, sweep_ms=140)], at_ms=k * 130, attack_ms=40, release_ms=110) for k in range(3)],
    [struck("sine", 0.05, [[0, 1]], 260, freq=f, at=60 + k * 130) for k, f in enumerate((1568, 1760, 2093))])
cue("card_pick", "Upgrade picked",
    tone("sine", 140, 0.25, 120, to=80),
    [tone("triangle", f, 0.07, 600, at=20) for f in (523, 659, 784)])
cue("reroll", "Reroll",
    struck("noise", 0.14, [[t, 1] for t in range(0, 320, 32)], 10, filters=[bp(2500, 1.5)]),
    {**tone("sine", 400, 0.06, 80, to=820, hold=280, glide=320), "glide": "linear"})

# ---- phases and timers ----
cue("horn_build", "Build begins",
    horn(293.7, 0, 230), horn(440, 330, 230), horn(587.3, 660, 650, release=500),
    horn(146.8, 0, 1300, gain=0.05, release=500))
cue("horn_siege", "Siege begins",
    horn(220, 0, 130, gain=0.13), horn(220, 230, 130, gain=0.13), horn(329.6, 460, 900, gain=0.13, release=450),
    horn(110, 0, 1500, gain=0.07, release=450),
    tone("sine", 70, 0.4, 500, to=40), struck("noise", 0.25, [[0, 1]], 60, filters=[lp(300)]))
cue("tick", "Timer tick",
    tone("sine", 1250, 0.11, 45, to=1000),
    struck("noise", 0.06, [[0, 1]], 5, filters=[bp(2600, 3)]))
cue("tick_last", "Last tick",
    tone("sine", 1650, 0.14, 60, to=1350),
    struck("noise", 0.08, [[0, 1]], 6, filters=[bp(3200, 3)]))
cue("elimination", "Player out",
    horn(392, 0, 250, gain=0.1), horn(370, 330, 250, gain=0.1), horn(330, 660, 250, gain=0.1),
    {**horn(262, 990, 700, gain=0.1, release=700), "wobble": [{"rate": 5, "cents": 25}]},
    metal(110, 0.18, 1600, at=990))
cue("victory", "Victory",
    trumpet(523.3, 0, 90), trumpet(523.3, 150, 90), trumpet(523.3, 300, 90), trumpet(784, 450, 330),
    trumpet(659.3, 900, 110), trumpet(784, 1080, 110), trumpet(1046.5, 1260, 1000, release=650),
    trumpet(659.3, 1260, 1000, gain=0.06, release=650), trumpet(784, 1260, 1000, gain=0.06, release=650),
    trumpet(261.6, 1260, 1000, gain=0.06, release=650),
    struck("sine", 0.25, scatter(12, 50, start=900, fall=1.0, wobble=(0.6, 0.8, 0.7, 0.9)), 60, freq=98),
    struck("sine", 0.45, [[1260, 1]], 450, freq=82),
    struck("noise", 0.2, [[1260, 1]], 120, filters=[lp(400)]))
cue("ui_click", "Click", tone("sine", 1000, 0.08, 35, to=700), struck("noise", 0.03, [[0, 1]], 4, filters=[bp(3000, 2)]))
cue("ui_hover", "Hover", tone("sine", 1800, 0.025, 28))

# tidy: drop helper keys that are not recipe fields
for c in C:
    for l in c["layers"]:
        l.pop("glide_kind", None)


# ---- music ----

INSTRUMENTS = {
    "lute": {"layers": [
        {"wave": "periodic", "harmonics": [1, 0.62, 0.46, 0.32, 0.22, 0.16, 0.1, 0.07, 0.05, 0.03], "ratio": 1, "gain": 0.075,
         "attack_ms": 3, "release_ms": 950, "filters": [{"type": "lowpass", "ratio": 9, "to_ratio": 2.2, "sweep_ms": 260, "q": 1}]},
        {"wave": "periodic", "harmonics": [1, 0.5, 0.3, 0.2, 0.12, 0.08], "ratio": 1.0035, "gain": 0.03,
         "attack_ms": 4, "release_ms": 800, "filters": [{"type": "lowpass", "ratio": 6, "to_ratio": 2, "sweep_ms": 220, "q": 1}]}]},
    "recorder": {"layers": [
        {"wave": "sine", "ratio": 1, "gain": 0.06, "attack_ms": 45, "hold_ms": "note", "gap_ms": 70, "release_ms": 90,
         "wobble": [{"rate": 5.2, "cents": 9}]},
        {"wave": "triangle", "ratio": 2, "gain": 0.008, "attack_ms": 60, "hold_ms": "note", "gap_ms": 70, "release_ms": 80},
        {"wave": "noise", "gain": 0.006, "attack_ms": 30, "hold_ms": "note", "gap_ms": 90, "release_ms": 60,
         "filters": [{"type": "bandpass", "ratio": 2, "q": 5}]}]},
    "drone": {"layers": [
        {"wave": "sawtooth", "ratio": 1, "gain": 0.016, "filters": [lp(420)], "wobble": [{"rate": 0.23, "cents": 4}]},
        {"wave": "sine", "ratio": 2, "gain": 0.008, "wobble": [{"rate": 0.31, "cents": 5}]}]},
    "tabor": {"layers": [
        struck("noise", 0.11, [[0, 1]], 60, filters=[bp(190, 1.5)]),
        tone("sine", 125, 0.09, 120, to=72, glide=90),
        struck("noise", 0.03, [[0, 1]], 28, filters=[bp(1900, 1)])]},
    "timpani": {"layers": [
        tone("sine", 84, 0.2, 650, to=72, glide=300),
        tone("sine", 136, 0.04, 260),
        struck("noise", 0.09, [[0, 1]], 40, filters=[lp(320)])]},
    "war_drum": {"layers": [
        tone("sine", 62, 0.17, 280, to=40),
        struck("noise", 0.08, [[0, 1]], 55, filters=[lp(160)])]},
    "snare": {"layers": [
        struck("noise", 0.06, [[0, 1]], 34, filters=[bp(2100, 0.8)]),
        tone("triangle", 200, 0.03, 60, to=170)]},
    "woodblock": {"layers": [
        tone("sine", 1400, 0.1, 40, to=1150),
        struck("noise", 0.025, [[0, 1]], 5, filters=[bp(2800, 3)])]},
    "strings": {"layers": [
        {"wave": "sawtooth", "ratio": 1, "gain": 0.04, "attack_ms": 8, "release_ms": 200,
         "filters": [{"type": "lowpass", "ratio": 4, "q": 2}]},
        {"wave": "sawtooth", "ratio": 0.5, "gain": 0.025, "attack_ms": 8, "release_ms": 200,
         "filters": [{"type": "lowpass", "ratio": 3, "q": 1}]}]},
    "shawm": {"layers": [
        {"wave": "sawtooth", "ratio": 1, "gain": 0.035, "attack_ms": 40, "hold_ms": "note", "gap_ms": 50, "release_ms": 110,
         "filters": [{"type": "lowpass", "ratio": 5, "q": 3}], "wobble": [{"rate": 5.5, "cents": 12}]},
        {"wave": "square", "ratio": 1, "gain": 0.01, "attack_ms": 40, "hold_ms": "note", "gap_ms": 50, "release_ms": 110,
         "filters": [{"type": "lowpass", "ratio": 3, "q": 1}]}]},
    "low_drone": {"layers": [
        {"wave": "sawtooth", "ratio": 1, "gain": 0.022, "filters": [lp(300)], "wobble": [{"rate": 0.19, "cents": 5}]},
        {"wave": "square", "ratio": 0.5, "gain": 0.012, "filters": [lp(200)]}]},
}

BUILD = {
    "id": "music_build", "label": "Build and upgrade music",
    "bpm": 96, "beats_per_bar": 4, "section_bars": 8, "root_hz": 146.83, "mode": [0, 2, 3, 5, 7, 9, 10],
    "seed": 11, "humanize_ms": 12,
    "form": ["A", "B", "C", "A2"],
    "drones": [{"instrument": "drone", "degree": -7}, {"instrument": "drone", "degree": -3, "gain": 0.7}],
    "parts": [
        {"type": "notes", "instrument": "recorder", "octave": 1, "sections": {
            "A": {"notes": "4:1 3:.5 2:.5 1:1 0:1  2:1.5 3:.5 4:2  5:1 4:.5 3:.5 4:1 7:1  6:1 5:1 4:2 "
                           "4:1 3:.5 2:.5 1:1 0:.5 1:.5  2:1 4:1 3:1 2:1  1:1 2:.5 1:.5 -1:1 1:1  0:3 r:1"},
            "B": {"notes": "7:1.5 8:.5 9:1 7:1  6:1 5:1 4:2  5:1 6:1 7:1 5:1  4:3 r:1 "
                           "7:1.5 8:.5 9:1 10:1  9:.5 8:.5 7:1 6:2  5:1 4:.5 3:.5 2:1 1:1  0:2 4:1 r:1"},
            "C": {"notes": "r:8 4:4 3:2 2:2 1:4 0:4 r:8", "gain": 0.8},
            "A2": {"notes": "4:1 3:.5 2:.5 1:1 0:1  2:1.5 3:.5 4:2  5:1 4:.5 3:.5 4:1 7:1  6:1 5:.5 6:.5 4:2 "
                            "4:1 3:.5 2:.5 1:1 0:.5 1:.5  2:1 4:1 3:1 2:1  1:1 2:.5 1:.5 -1:1 1:1  0:3 r:1"}}},
        {"type": "arp", "instrument": "lute", "step_beats": 0.5, "pattern": [-7, 0, 2, 4, 7, 4, 2, 0],
         "soft": 0.65, "ring": 2, "sections": {
            "A": {"chords": [0, 2, 3, 4, 0, 2, 6, 0]},
            "B": {"chords": [0, 6, 3, 4, 0, 2, 3, 0]},
            "C": {"chords": [0, 0, 6, 6, 3, 3, 4, 4], "pattern": [-7, None, 4, 2, 7, None, 4, 2]},
            "A2": {"chords": [0, 2, 3, 4, 0, 2, 6, 0], "pattern": [-7, 4, 2, 4, 7, 4, 9, 4]}}},
        {"type": "drum", "instrument": "tabor", "step_beats": 0.5, "soft": 0.5, "sections": {
            "A": {"bars": ["X..x..x.", "X..x.xx."]},
            "B": {"bars": ["X..x..x.", "X.xx..x."]},
            "C": {"bars": ["X.......", "X...x..."], "gain": 0.7},
            "A2": {"bars": ["X..x..x.", "X..x.xxx"]}}},
    ],
}

SIEGE = {
    "id": "music_siege", "label": "Siege music",
    "bpm": 120, "beats_per_bar": 4, "section_bars": 8, "root_hz": 110, "mode": [0, 2, 3, 5, 7, 8, 10],
    "seed": 23, "humanize_ms": 6,
    "form": ["S1", "S2", "S3", "S4", "S3", "S4"],
    "drones": [{"instrument": "low_drone", "degree": -7}, {"instrument": "low_drone", "degree": -3, "gain": 0.6}],
    "parts": [
        {"type": "drum", "instrument": "timpani", "step_beats": 0.5, "soft": 0.6, "sections": {
            "S1": {"bars": ["X...x...", "X...x.x."]},
            "S2": {"bars": ["X...x...", "X...x.x."]},
            "S3": {"bars": ["X.x.x...", "X.x.x.xx"]},
            "S4": {"bars": ["X.xxX.x.", "X.xxX.xx"]}}},
        {"type": "drum", "instrument": "war_drum", "step_beats": 0.25, "soft": 0.6, "sections": {
            "S2": {"bars": ["X.......x......."]},
            "S3": {"bars": ["X...x...X...x..."]},
            "S4": {"bars": ["X..xX...X..xX.x."]}}},
        {"type": "drum", "instrument": "snare", "step_beats": 0.25, "soft": 0.5, "sections": {
            "S3": {"bars": ["....X.......X..."]},
            "S4": {"bars": ["....X.......X.xx", "....X.......xxxx"]}}},
        {"type": "arp", "instrument": "strings", "step_beats": 0.5, "pattern": [0, 0, 7, 0, 5, 0, 3, 4],
         "soft": 0.75, "ring": 1, "sections": {
            "S2": {"chords": [0, 0, 0, 0, 5, 5, 6, 4]},
            "S3": {"chords": [0, 0, 0, 0, 5, 5, 6, 4]},
            "S4": {"chords": [0, 0, 5, 5, 3, 3, 6, 4], "pattern": [0, 7, 0, 7, 5, 3, 4, 2]}}},
        {"type": "notes", "instrument": "shawm", "octave": 1, "sections": {
            "S3": {"notes": "7:2 6:1 4:1 5:3 4:1 3:2 2:1 1:1 0:4  4:2 5:1 6:1 7:3 8:1 7:2 6:1 5:1 4:4"},
            "S4": {"notes": "9:2 8:1 7:1 8:3 7:1 6:2 5:1 6:1 4:4  7:1 7:1 8:1 9:1 10:3 9:1 8:1 7:1 6:1 5:1 4:4"}}},
    ],
}

URGENT = {
    "id": "music_siege_urgent", "label": "Siege last seconds (layer)",
    "bpm": 120, "beats_per_bar": 4, "section_bars": 2, "root_hz": 220, "mode": [0, 2, 3, 5, 7, 8, 10],
    "seed": 5, "humanize_ms": 0, "form": ["U"], "drones": [],
    "parts": [
        {"type": "drum", "instrument": "woodblock", "step_beats": 0.25, "soft": 0.5, "sections": {
            "U": {"bars": ["X.x.x.x.X.x.x.x."]}}},
        {"type": "arp", "instrument": "strings", "step_beats": 0.25, "pattern": [7, 8, 7, 6], "soft": 0.8, "ring": 1,
         "gain": 0.6, "sections": {"U": {"chords": [0, 4]}}},
    ],
}

DATA = {
    "_note": "Every sound in Royal Rubble. There are no audio files: tools/sound/compile.py compiles these recipes into data/sound-programs.json, which web/sound/synth.js plays and tools/sound/render.py renders. Written by tools/sound/author.py; edit there. Frequencies in Hz, times in ms, gains 0 to 1.",
    "silence": SILENCE,
    "noise": {"samples": 44100, "seed": 7, "rate": 44100},
    "setup": {
        "_about": "master_scale: a full slider plays 60% of the level the test page plays (the low-volume rule, after Fleeced's T23). music_gain sits the music well under the effects. Times in seconds except tick_ms.",
        "master_scale": 0.6, "sfx_gain": 1.0, "music_gain": 0.32, "urgent_gain": 0.8,
        "smooth": 0.06, "crossfade": 0.33, "fade_out": 1.4, "lookahead": 0.5, "tick_ms": 120, "repeat_gap": 0.04, "urgent_track": "music_siege_urgent",
        "limiter": {"threshold": -6, "knee": 6, "ratio": 4, "attack": 0.003, "release": 0.25},
    },
    "groups": {
        "impact_wood": ["impact_wood_1", "impact_wood_2", "impact_wood_3"],
        "impact_stone": ["impact_stone_1", "impact_stone_2", "impact_stone_3"],
        "impact_metal": ["impact_metal_1", "impact_metal_2", "impact_metal_3"],
        "debris": ["debris_1", "debris_2"],
    },
    "tiers": {"collapse": [[1, "collapse_small"], [5, "collapse_mid"], [12, "collapse_big"]]},
    "sections": [
        {"label": "Trebuchet", "ids": ["trebuchet_creak", "trebuchet_release", "shot_whoosh"]},
        {"label": "Impacts", "ids": ["impact_wood_1", "impact_wood_2", "impact_wood_3", "impact_stone_1", "impact_stone_2",
                                     "impact_stone_3", "impact_metal_1", "impact_metal_2", "impact_metal_3", "segment_snap",
                                     "collapse_small", "collapse_mid", "collapse_big", "debris_1", "debris_2"]},
        {"label": "Shots and air effects", "ids": ["splash", "explosion", "fire_ignite", "fire_crackle", "phase_shimmer",
                                                   "split_pop", "dive_whistle", "air_drop", "air_reverse"]},
        {"label": "Royals", "ids": ["royal_squire", "royal_knight", "royal_princess", "royal_prince", "royal_queen", "royal_king"]},
        {"label": "Building and gold", "ids": ["place_segment", "sell_segment", "coin", "buy_xp", "level_up",
                                               "cards_reveal", "card_pick", "reroll"]},
        {"label": "Phases and menus", "ids": ["horn_build", "horn_siege", "tick", "tick_last", "elimination", "victory",
                                              "ui_click", "ui_hover"]},
        {"label": "Music", "ids": ["music_build", "music_siege", "music_siege_urgent"]},
    ],
    "cues": C,
    "instruments": INSTRUMENTS,
    "music": [BUILD, SIEGE, URGENT],
}

if __name__ == "__main__":
    ids = {c["id"] for c in C} | {m["id"] for m in DATA["music"]}
    listed = [i for s in DATA["sections"] for i in s["ids"]]
    assert set(listed) == ids, set(listed) ^ ids
    (ROOT / "data" / "sounds.json").write_text(json.dumps(DATA, indent=1) + "\n")
    print(f"data/sounds.json: {len(C)} cues, {len(DATA['music'])} music tracks")
