#!/usr/bin/env python3
"""synth.py: School's bells' takes (pack school-bell, make_pack.py; the owner,
2026-11-27: "give me many versions, this should sound post-apocalypse like").

usage: python3 -I synth.py <out folder>   (numpy and scipy; writes <take>.wav, 44.1 kHz mono 16 bit)

A Soviet electric wall bell, a hammer buzzing on a steel dome, after decades
of rust, damp and dying current: each take below is its own wreck of it. No
recording: the dome's modes are synthesized (DOME, a dome's inharmonic
ratios), each hammer blow a click and their ring-down, the hammer at its
rate (its jitter, its gaps, a fading current), arcing contacts (crackle),
a coil's hum, all in an empty concrete corridor (a sparse, dark impulse
response). The random generator's seed is fixed: the same takes each run,
which make_pack.py pins by sha256.
"""
import sys
import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, sosfilt, fftconvolve
SR = 44100
OUT = sys.argv[1]
rng = np.random.default_rng(7)
# a dome's inharmonic modes (ratio, relative level, decay s at f0)
DOME = [(1.0, 1.0, 1.0), (2.32, 0.6, 0.55), (4.25, 0.35, 0.3), (6.63, 0.22, 0.18), (9.38, 0.12, 0.1), (12.1, 0.06, 0.06)]

def bp(x, lo, hi, order=2):
    return sosfilt(butter(order, [lo, hi], 'bandpass', fs=SR, output='sos'), x)
def lp(x, f, order=2):
    return sosfilt(butter(order, f, 'lowpass', fs=SR, output='sos'), x)
def hp(x, f, order=2):
    return sosfilt(butter(order, f, 'highpass', fs=SR, output='sos'), x)

def strike(f0, dur, damp=1.0, detune=0.0, crack=0.0, modes=DOME):
    """One hammer blow on the dome: its modes ringing down (damp > 1: rust)."""
    t = np.arange(int(dur * SR)) / SR
    y = np.zeros_like(t)
    for k, (r, a, d) in enumerate(modes):
        f = f0 * r * (1 + detune * rng.uniform(-1, 1))
        # a crack splits each mode in two close ones: a beating rattle
        pair = [(f, 1.0)] + ([(f * (1 + crack * (0.6 + 0.4 * k)), 0.8)] if crack else [])
        for ff, g in pair:
            if ff > SR / 2.2: continue
            y += a * g * np.sin(2 * np.pi * ff * t + rng.uniform(0, 6.28)) * np.exp(-t / (d / damp))
    click = rng.normal(0, 1, len(t)) * np.exp(-t / 0.0015) * 0.6
    return y + hp(click, 2000)

def buzz(f0, length, rate, damp=1.0, detune=0.0, crack=0.0, jitter=0.0, fade=None, gaps=0.0, power=None):
    """The hammer at `rate` blows a second for `length` s."""
    out = np.zeros(int((length + 1.5) * SR))
    t = 0.0
    while t < length:
        level = 1.0 if power is None else power(t / length)
        if level > 0.02 and rng.random() >= gaps:
            s = strike(f0 * (1 + 0.01 * rng.normal()), 1.2, damp, detune, crack) * level * rng.uniform(0.7, 1.0)
            i = int(t * SR); out[i:i + len(s)] += s[:len(out) - i]
        r = rate if fade is None else rate * fade(t / length)
        t += max(1.0 / max(r, 1.0), 0.01) * (1 + jitter * rng.normal())
    return out

def crackle(length, density, level):
    """Arcing contacts: sparse spark ticks and fizz."""
    n = int(length * SR); y = np.zeros(n)
    for _ in range(int(density * length)):
        i = rng.integers(0, n - 2000); w = rng.integers(40, 600)
        y[i:i + w] += rng.normal(0, 1, w) * np.exp(-np.arange(w) / (w / 4)) * rng.uniform(0.2, 1)
    return hp(y, 1500) * level

def hum(length, level, f=50.0):
    t = np.arange(int(length * SR)) / SR
    return level * np.tanh(3 * np.sin(2 * np.pi * f * t)) * (0.6 + 0.4 * np.sin(2 * np.pi * 0.7 * t))

def corridor(y, wet=0.35, tail=1.8):
    """An empty concrete school corridor: a sparse, dark, long impulse response."""
    n = int(tail * SR); t = np.arange(n) / SR
    ir = rng.normal(0, 1, n) * np.exp(-t / (tail / 5)); ir = lp(ir, 3500); ir[0] = 0
    for d in (0.023, 0.041, 0.067, 0.112):          # the walls' first echoes
        ir[int(d * SR)] += 0.6
    wetsig = np.pad(fftconvolve(y, ir), (0, 1))[:len(y) + n]
    dry = np.concatenate([y, np.zeros(n)])
    return dry * (1 - wet) + wetsig / (np.max(np.abs(wetsig)) + 1e-9) * np.max(np.abs(y)) * wet * 1.6

def mix(*parts):
    n = max(len(p) for p in parts); out = np.zeros(n)
    for p in parts: out[:len(p)] += p
    return out

def finish(name, y, desc):
    y = y[: np.max(np.nonzero(np.abs(y) > 1e-4 * np.max(np.abs(y)))) + 1]
    fade = min(len(y), int(0.05 * SR)); y[-fade:] *= np.linspace(1, 0, fade)
    y = hp(y, 120)
    y = y / np.max(np.abs(y)) * 0.89
    wavfile.write(f'{OUT}/{name}.wav', SR, (y * 32767).astype(np.int16))
    print(f'{name}.wav\t{len(y) / SR:.2f} s\t{desc}')

# 1. A shot shorts its wires: a full ring that sags as the old battery dies.
finish('01_dying_ring', corridor(mix(buzz(1250, 1.4, 22, damp=1.3, detune=0.004, jitter=0.15,
        fade=lambda u: 1 - 0.75 * u, power=lambda u: 1 - 0.8 * u ** 2), crackle(1.4, 25, 0.25))),
       'a full drrring sagging and slowing as the battery dies')
# 2. Rusted shut: a dull, choked dome, the hammer dragging.
finish('02_rusted_choke', corridor(mix(buzz(820, 1.0, 14, damp=3.5, detune=0.01, jitter=0.35, gaps=0.15),
        lp(crackle(1.0, 40, 0.4), 6000)), wet=0.25), 'dull, choked by rust, the hammer dragging')
# 3. Bad contacts: bursts that cut in and out, sparks between.
def stutter():
    parts, t = [], 0.0
    for length in (0.25, 0.12, 0.4, 0.08, 0.3):
        b = buzz(1180, length, 24, damp=1.6, detune=0.006, jitter=0.1)
        parts.append(np.concatenate([np.zeros(int(t * SR)), b])); t += length + rng.uniform(0.05, 0.18)
    return mix(*parts, crackle(t, 60, 0.5))
finish('03_bad_contacts', corridor(stutter()), 'bursts cutting in and out, sparking between')
# 4. Cracked dome: every blow rattles and beats.
finish('04_cracked_dome', corridor(buzz(1050, 1.2, 20, damp=1.4, crack=0.035, jitter=0.12,
        power=lambda u: 1 - 0.5 * u)), 'a cracked dome: a beating, rattling ring')
# 5. No current at all: the bullet itself strikes it, one clang and a long ringing tail.
finish('05_single_clang', corridor(mix(strike(980, 3.0, damp=0.45, crack=0.012),
        0.25 * strike(980 * 2.32, 0.2, damp=2.0)), wet=0.45, tail=2.5),
       'no power: the bullet strikes it once, a long cracked tail')
# 6. Two hits of the hammer, then it sticks with a fizz.
finish('06_hammer_sticks', corridor(mix(buzz(1200, 0.35, 23, damp=1.2, detune=0.005),
        np.concatenate([np.zeros(int(0.36 * SR)), hum(0.9, 0.12, 100) * np.linspace(1, 0, int(0.9 * SR)),
                        ]), crackle(1.2, 30, 0.35))), 'a short burst, then the hammer sticks and the coil hums')
# 7. Bent dome: the ring wobbles in pitch, like a warped saw.
def bent():
    y = buzz(1150, 1.3, 21, damp=1.5, detune=0.003, jitter=0.1, power=lambda u: 1 - 0.6 * u)
    t = np.arange(len(y)) / SR
    warp = t + 0.0009 * np.sin(2 * np.pi * 3.3 * t) + 0.0006 * np.sin(2 * np.pi * 5.1 * t)
    return np.interp(warp * SR, np.arange(len(y)), y)
finish('07_bent_dome', corridor(bent()), 'a bent dome: the ring wobbles in pitch')
# 8. Far down the empty corridor: a weak, rusty ring mostly echo.
finish('08_far_corridor', corridor(lp(buzz(1100, 1.1, 19, damp=2.0, jitter=0.2, power=lambda u: 0.8 - 0.5 * u), 3000),
        wet=0.75, tail=3.0), 'weak and far, mostly the corridor\'s echo')
# 9. Starts slow (cold coil), catches, then gives out.
finish('09_cold_start', corridor(mix(buzz(1220, 1.6, 22, damp=1.4, detune=0.005, jitter=0.12,
        fade=lambda u: 0.25 + 1.5 * u if u < 0.5 else 1.0 - 1.4 * (u - 0.5), power=lambda u: min(1, 0.3 + 2 * u) * (1 - 0.7 * u)),
        crackle(1.6, 20, 0.2))), 'slow at first, catches, then gives out')
# 10. A drowned bell: water in the housing, gurgling, muffled, short.
def drowned():
    y = lp(buzz(900, 0.9, 16, damp=4.0, jitter=0.4, gaps=0.25), 2200)
    t = np.arange(len(y)) / SR
    return y * (0.6 + 0.4 * np.sin(2 * np.pi * 7 * t + 2 * np.sin(2 * np.pi * 1.3 * t)))
finish('10_drowned', corridor(drowned(), wet=0.3), 'water in the housing: muffled, gurgling, short')
# 11. A long full ring as it was meant (the reference), only aged: for comparison.
finish('11_old_but_working', corridor(buzz(1300, 1.5, 24, damp=1.1, detune=0.002, jitter=0.05,
        power=lambda u: 1.0 if u < 0.9 else 1 - 9 * (u - 0.9))), 'an old bell still working: a full drrring')
# 12. A shot dents it mid-ring: bright ring that turns choked and stops.
finish('12_shot_dent', corridor(mix(buzz(1250, 0.45, 23, damp=1.1), np.concatenate([np.zeros(int(0.45 * SR)),
        buzz(1010, 0.6, 15, damp=3.0, crack=0.03, jitter=0.3, power=lambda u: 1 - u)]), crackle(1.1, 30, 0.3))),
       'rings bright, the shot dents it: choked and rattling to a stop')
