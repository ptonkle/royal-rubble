"""The building songs that rotate round by round, the menu theme, and the
instruments only they use (R1.3802). Imported by tools/sound/author.py.

Folk tunes for a castle village: lute, mandolin and recorder over a frame
drum. Every melody is original. Notes are "degree:beats" in the song's mode
(0 is the root, 7 an octave up, r a rest); each section holds section_bars
bars exactly, which compile.py checks.
"""


def lp(freq, q=0.7071):
    return {"type": "lowpass", "freq": freq, "q": q}


INSTRUMENTS = {
    # a plucked double course: two strings a hair apart, the second struck
    # just after the first, bright at the pick and dulling fast
    "mandolin": {"layers": [
        {"wave": "periodic", "harmonics": [1, 0.72, 0.56, 0.46, 0.36, 0.28, 0.21, 0.15, 0.1, 0.07, 0.05], "ratio": 1,
         "gain": 0.042, "attack_ms": 2, "release_ms": 420,
         "filters": [{"type": "lowpass", "ratio": 12, "to_ratio": 3, "sweep_ms": 170, "q": 1.4}]},
        {"wave": "periodic", "harmonics": [1, 0.6, 0.45, 0.33, 0.24, 0.16, 0.1, 0.06], "ratio": 1.0045, "at_ms": 7,
         "gain": 0.03, "attack_ms": 2, "release_ms": 360,
         "filters": [{"type": "lowpass", "ratio": 9, "to_ratio": 2.6, "sweep_ms": 150, "q": 1.2}]}]},
    # the same strings picked fast on one long note
    "mandolin_trem": {"layers": [
        {"wave": "periodic", "harmonics": [1, 0.62, 0.46, 0.32, 0.22, 0.14, 0.08], "ratio": 1, "gain": 0.03,
         "attack_ms": 12, "hold_ms": "note", "gap_ms": 50, "release_ms": 140,
         "filters": [{"type": "lowpass", "ratio": 6, "q": 1}], "throb": [{"rate": 12.5, "depth": 0.8}]},
        {"wave": "periodic", "harmonics": [1, 0.5, 0.3, 0.2, 0.1], "ratio": 1.004, "gain": 0.018,
         "attack_ms": 14, "hold_ms": "note", "gap_ms": 50, "release_ms": 140,
         "filters": [{"type": "lowpass", "ratio": 5, "q": 1}], "throb": [{"rate": 13.1, "depth": 0.75}]}]},
    "bodhran": {"layers": [
        {"wave": "noise", "gain": 0.1, "strikes": [[0, 1]], "ring_ms": 70, "filters": [{"type": "bandpass", "freq": 140, "q": 1.2}]},
        {"wave": "sine", "freq": 108, "to_freq": 64, "glide_ms": 120, "gain": 0.08, "attack_ms": 2, "release_ms": 150},
        {"wave": "noise", "gain": 0.02, "strikes": [[0, 1]], "ring_ms": 20, "filters": [{"type": "bandpass", "freq": 1300, "q": 1}]}]},
    "tambourine": {"layers": [
        {"wave": "noise", "gain": 0.03, "strikes": [[0, 1], [22, 0.5]], "ring_ms": 45, "filters": [{"type": "highpass", "freq": 5500, "q": 0.8}]}]},
    "horn_note": {"layers": [
        {"wave": "sawtooth", "ratio": 1, "gain": 0.04, "attack_ms": 55, "hold_ms": "note", "gap_ms": 45, "release_ms": 170,
         "filters": [{"type": "lowpass", "ratio": 4, "q": 1.2}], "wobble": [{"rate": 4.8, "cents": 8}]},
        {"wave": "sawtooth", "ratio": 0.5, "gain": 0.016, "attack_ms": 70, "hold_ms": "note", "gap_ms": 45, "release_ms": 170,
         "filters": [{"type": "lowpass", "ratio": 3, "q": 1}]}]},
    "soft_drone": {"layers": [
        {"wave": "sawtooth", "ratio": 1, "gain": 0.009, "filters": [lp(360)], "wobble": [{"rate": 0.21, "cents": 4}]},
        {"wave": "sine", "ratio": 2, "gain": 0.006, "wobble": [{"rate": 0.29, "cents": 5}]}]},
}

MAJOR = [0, 2, 4, 5, 7, 9, 11]
MINOR = [0, 2, 3, 5, 7, 8, 10]


def shifted(notes, by):
    """The same tune `by` degrees away, rests kept: a second voice in thirds."""
    out = []
    for tok in notes.split():
        d, b = tok.split(":")
        out.append(tok if d == "r" else f"{int(d) + by}:{b}")
    return " ".join(out)


# ---- A village jig: G major, 6/8, an eighth to the beat ----
JIG_A = ("4:1 2:1 4:1 7:2 4:1  5:1 4:1 5:1 6:1 5:1 4:1  4:1 2:1 4:1 7:2 9:1  8:1 7:1 6:1 5:3 "
         "4:1 2:1 4:1 7:2 4:1  5:1 6:1 7:1 8:2 7:1  6:1 4:1 6:1 5:1 3:1 1:1  2:1 1:1 -1:1 0:3")
JIG_A2 = ("4:1 2:1 4:1 7:2 4:1  5:1 4:1 5:1 6:1 5:1 4:1  4:1 2:1 4:1 7:2 9:1  8:1 7:1 6:1 5:3 "
          "4:1 2:1 4:1 7:2 4:1  5:1 6:1 7:1 8:2 7:1  6:1 8:1 6:1 4:1 5:1 6:1  7:3 r:3")
JIG_B = ("9:2 8:1 9:2 7:1  8:1 7:1 8:1 6:2 4:1  9:2 8:1 9:1 10:1 11:1  11:3 9:3 "
         "12:1 11:1 10:1 9:1 8:1 7:1  8:1 9:1 8:1 6:2 4:1  5:1 7:1 6:1 5:1 4:1 2:1  1:3 4:3")
JIG_B2 = ("9:2 8:1 9:2 7:1  8:1 7:1 8:1 6:2 4:1  9:2 8:1 9:1 10:1 11:1  11:3 9:3 "
          "12:1 11:1 10:1 9:1 8:1 7:1  8:1 9:1 8:1 6:2 4:1  5:1 7:1 6:1 8:1 6:1 4:1  7:3 r:3")

JIG = {
    "id": "music_jig", "label": "Village jig",
    "bpm": 318, "beats_per_bar": 6, "section_bars": 8, "root_hz": 196.0, "mode": MAJOR,
    "seed": 31, "humanize_ms": 9,
    "form": ["A", "A2", "B", "B2"],
    "drones": [],
    "parts": [
        {"type": "notes", "instrument": "mandolin", "octave": 1, "sections": {
            "A": {"notes": JIG_A}, "A2": {"notes": JIG_A2}, "B": {"notes": JIG_B}, "B2": {"notes": JIG_B2}}},
        {"type": "notes", "instrument": "recorder", "octave": 1, "gain": 0.7, "sections": {
            "B": {"notes": JIG_B}, "B2": {"notes": JIG_B2}}},
        {"type": "arp", "instrument": "lute", "step_beats": 1, "pattern": [-7, 2, 4, -3, 2, 4],
         "soft": 0.55, "ring": 2, "sections": {
            "A": {"chords": [0, 5, 0, 4, 0, 3, 4, 0]},
            "A2": {"chords": [0, 5, 0, 4, 0, 3, 4, 0]},
            "B": {"chords": [5, 4, 0, 0, 3, 4, 0, 4]},
            "B2": {"chords": [5, 4, 0, 0, 3, 4, 4, 0]}}},
        {"type": "drum", "instrument": "bodhran", "step_beats": 1, "soft": 0.5, "sections": {
            "A": {"bars": ["X.xX.x", "X.xX.x", "X.xX.x", "X.xXxx"]},
            "A2": {"bars": ["X.xX.x", "X.xX.x", "X.xX.x", "X.xX.."]},
            "B": {"bars": ["X.xX.x", "XxxX.x"]},
            "B2": {"bars": ["X.xX.x", "XxxX.x", "X.xX.x", "X.xX.."]}}},
        {"type": "drum", "instrument": "tambourine", "step_beats": 1, "soft": 0.6, "sections": {
            "B": {"bars": ["..x..x"]}, "B2": {"bars": ["..x..x"]}}},
    ],
}

# ---- Under the hill: a gentle pastoral, F major, 3/4 ----
PAST_A = "4:2 5:1 4:2 2:1 3:1.5 2:.5 1:1 2:3 4:2 5:1 7:2 6:1 5:1 4:1 3:1 4:3"
PAST_B = "7:2 8:1 9:2 7:1 8:1 7:1 6:1 5:3 6:2 5:1 4:2 2:1 3:1 2:1 1:1 0:3"
PAST_A2 = "4:2 5:1 4:2 2:1 3:1.5 2:.5 1:1 2:2 4:1 7:2 6:.5 5:.5 4:2 2:1 1:1.5 2:.5 1:1 0:3"

PASTORAL = {
    "id": "music_pastoral", "label": "Under the hill",
    "bpm": 84, "beats_per_bar": 3, "section_bars": 8, "root_hz": 174.61, "mode": MAJOR,
    "seed": 37, "humanize_ms": 14,
    "form": ["A", "B", "A2", "C"],
    "drones": [{"instrument": "soft_drone", "degree": -7}, {"instrument": "soft_drone", "degree": -3, "gain": 0.6}],
    "parts": [
        {"type": "notes", "instrument": "recorder", "octave": 1, "sections": {
            "A": {"notes": PAST_A}, "B": {"notes": PAST_B}, "A2": {"notes": PAST_A2},
            "C": {"notes": "r:6 7:3 6:3 5:3 4:3 r:6", "gain": 0.75}}},
        {"type": "arp", "instrument": "lute", "step_beats": 0.5, "pattern": [-7, 0, 2, 4, 2, 0],
         "soft": 0.6, "ring": 2.5, "sections": {
            "A": {"chords": [0, 0, 3, 0, 0, 3, 4, 0]},
            "B": {"chords": [0, 0, 4, 5, 4, 0, 4, 0]},
            "A2": {"chords": [0, 0, 3, 0, 3, 0, 4, 0]},
            "C": {"chords": [0, 5, 3, 4, 0, 5, 3, 4], "pattern": [-7, None, 4, 2, 7, 2]}}},
        {"type": "notes", "instrument": "mandolin_trem", "octave": 0, "gain": 0.6, "sections": {
            "C": {"notes": "4:3 2:3 3:3 1:3 4:3 2:3 3:3 4:3"}}},
    ],
}

# ---- A wistful ballad: A minor, slow 4/4 ----
BAL_A = ("4:1.5 3:.5 2:1 1:1  0:2 2:1 4:1  5:1.5 4:.5 3:1 2:1  1:3 r:1 "
         "2:1.5 3:.5 4:1 5:1  4:2 7:1 6:1  5:1 4:1 3:1 1:1  0:3 r:1")
BAL_B = ("7:2 6:1 5:1  4:2 2:2  5:2 4:1 3:1  4:4 "
         "7:1.5 8:.5 9:1 7:1  6:2 4:2  5:1 3:1 1:1 -1:1  0:4")
BAL_A2 = ("4:1.5 3:.5 2:1 1:1  0:2 2:1 4:1  5:1.5 4:.5 3:1 2:1  1:3 r:1 "
          "2:1.5 3:.5 4:1 5:1  4:2 7:1 6:1  5:1 4:1 3:1 1:1  0:4")

BALLAD = {
    "id": "music_ballad", "label": "Wistful ballad",
    "bpm": 70, "beats_per_bar": 4, "section_bars": 8, "root_hz": 220.0, "mode": MINOR,
    "seed": 41, "humanize_ms": 16,
    "form": ["A", "B", "A2"],
    "drones": [{"instrument": "soft_drone", "degree": -7}],
    "parts": [
        {"type": "notes", "instrument": "recorder", "octave": 1, "sections": {
            "A": {"notes": BAL_A}, "B": {"notes": BAL_B}, "A2": {"notes": BAL_A2}}},
        {"type": "notes", "instrument": "mandolin_trem", "octave": 0, "gain": 0.7, "sections": {
            "B": {"notes": "2:4 0:4 1:4 -1:4 2:4 0:4 -1:4 -3:4"}}},
        {"type": "arp", "instrument": "lute", "step_beats": 0.5, "pattern": [-7, 0, 4, 2, 7, 2, 4, 0],
         "soft": 0.6, "ring": 2.5, "sections": {
            "A": {"chords": [0, 0, 3, 4, 2, 0, 3, 0]},
            "B": {"chords": [5, 0, 3, 4, 0, 2, 6, 0]},
            "A2": {"chords": [0, 0, 3, 4, 2, 0, 3, 0]}}},
        {"type": "drum", "instrument": "tabor", "step_beats": 1, "soft": 0.5, "gain": 0.6, "sections": {
            "B": {"bars": ["X...", "X.x."]}}},
    ],
}

# ---- Two mandolins: D major, a lively 4/4, the second a third below ----
MAN_A = ("4:.5 5:.5 4:.5 3:.5 2:1 4:1  3:.5 4:.5 3:.5 2:.5 1:1 3:1 "
         "2:.5 3:.5 4:.5 5:.5 6:.5 7:.5 6:.5 5:.5  4:2 2:1 r:1 "
         "4:.5 5:.5 4:.5 3:.5 2:1 4:1  7:.5 6:.5 5:.5 4:.5 3:1 5:1 "
         "4:.5 3:.5 2:.5 1:.5 3:1 1:1  0:3 r:1")
MAN_B = ("7:1 7:.5 8:.5 9:1 7:1  8:1 8:.5 9:.5 10:1 8:1  9:.5 8:.5 7:.5 6:.5 5:1 7:1  6:2 4:1 r:1 "
         "7:1 7:.5 8:.5 9:1 11:1  10:.5 9:.5 8:.5 7:.5 8:1 6:1  7:.5 6:.5 5:.5 4:.5 5:.5 6:.5 4:1  7:3 r:1")

MANDOLINS = {
    "id": "music_mandolins", "label": "Two mandolins",
    "bpm": 112, "beats_per_bar": 4, "section_bars": 8, "root_hz": 146.83, "mode": MAJOR,
    "seed": 43, "humanize_ms": 8,
    "form": ["A1", "B", "A", "B2"],
    "drones": [],
    "parts": [
        {"type": "notes", "instrument": "mandolin", "octave": 1, "sections": {
            "A1": {"notes": MAN_A}, "B": {"notes": MAN_B}, "A": {"notes": MAN_A}, "B2": {"notes": MAN_B}}},
        {"type": "notes", "instrument": "mandolin", "octave": 1, "gain": 0.75, "sections": {
            "B": {"notes": shifted(MAN_B, -2)}, "A": {"notes": shifted(MAN_A, -2)}, "B2": {"notes": shifted(MAN_B, -2)}}},
        {"type": "arp", "instrument": "lute", "step_beats": 0.5, "pattern": [-7, 4, 2, 4, -3, 4, 2, 4],
         "soft": 0.5, "ring": 1.6, "sections": {
            "A1": {"chords": [0, 3, 4, 4, 0, 3, 4, 0]},
            "B": {"chords": [0, 4, 5, 4, 0, 3, 4, 0]},
            "A": {"chords": [0, 3, 4, 4, 0, 3, 4, 0]},
            "B2": {"chords": [0, 4, 5, 4, 0, 3, 4, 0]}}},
        {"type": "drum", "instrument": "tabor", "step_beats": 0.5, "soft": 0.5, "sections": {
            "A1": {"bars": ["X...X...", "X...X.x."]},
            "B": {"bars": ["X..xX.x.", "X..xX.xx"]},
            "A": {"bars": ["X..xX.x.", "X..xX.x."]},
            "B2": {"bars": ["X..xX.x.", "X.xxX.xx"]}}},
        {"type": "drum", "instrument": "tambourine", "step_beats": 1, "soft": 0.6, "sections": {
            "B": {"bars": [".x.x"]}, "B2": {"bars": [".x.x"]}}},
    ],
}

# ---- The menu theme: a stately D major 3/4, a horn call, then the tune ----
MENU = {
    "id": "music_menu", "label": "Menu theme",
    "bpm": 92, "beats_per_bar": 3, "section_bars": 8, "root_hz": 146.83, "mode": MAJOR,
    "seed": 47, "humanize_ms": 10,
    "form": ["A", "B", "B2"],
    "drones": [{"instrument": "soft_drone", "degree": -7}],
    "parts": [
        {"type": "notes", "instrument": "horn_note", "octave": 1, "sections": {
            "A": {"notes": "0:1 0:.5 0:.5 4:1 4:2 2:1 3:1 4:1 5:1 4:3 0:1 0:.5 0:.5 4:1 7:2 6:1 5:1 4:1 1:1 0:3"}}},
        {"type": "notes", "instrument": "recorder", "octave": 1, "sections": {
            "B": {"notes": "4:2 5:1 6:1.5 5:.5 4:1 3:2 2:1 1:3 2:2 3:1 4:1.5 5:.5 6:1 7:2 6:1 7:3"},
            "B2": {"notes": "7:2 8:1 9:1.5 8:.5 7:1 6:2 5:1 4:3 5:1 4:1 3:1 2:1 3:1 1:1 2:1.5 1:.5 0:1 0:3"}}},
        {"type": "notes", "instrument": "mandolin_trem", "octave": 0, "gain": 0.55, "sections": {
            "B2": {"notes": "4:3 4:3 2:3 1:3 0:3 -1:3 1:3 0:3"}}},
        {"type": "arp", "instrument": "lute", "step_beats": 0.5, "pattern": [-7, 0, 4, 7, 4, 2],
         "soft": 0.6, "ring": 2, "sections": {
            "A": {"chords": [0, 0, 3, 4, 0, 0, 1, 0]},
            "B": {"chords": [0, 4, 3, 4, 0, 4, 0, 0]},
            "B2": {"chords": [0, 0, 4, 4, 3, 1, 4, 0]}}},
        {"type": "drum", "instrument": "tabor", "step_beats": 0.5, "soft": 0.5, "gain": 0.8, "sections": {
            "A": {"bars": ["X.x.x.", "X.xxx."]},
            "B2": {"bars": ["X...x.", "X.x.x."]}}},
    ],
}

SONGS = [JIG, PASTORAL, BALLAD, MANDOLINS]
