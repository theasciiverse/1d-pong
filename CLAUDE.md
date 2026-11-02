# 1D-Pong

Pong with one dimension, for the Asciiverse catalog. Rust + Ratatui.
Started 2026-11-02 (David: "We NEED an AWESOME 1D pong game!").

**Status:** 0.1.0, playable. Own git repo (local so far), folder `games/1d-pong`.
Crate `one-d-pong` (Cargo package names can't start with a digit),
binary `1d-pong`. What comes next is in `ROADMAP.md`.

## The idea

1D Pong already exists as a folk hobby build: an LED strip with a button
at each end (Arduino builds, CCC 2025, OneSwitch's footswitch version,
Mega 1D Pong on the Mega65, blinry's browser Pong 1D). Nobody owns it,
and a search on 2026-11-02 found no terminal version. This one is ours,
written from scratch.

## Start pure, add slowly (David, 2026-11-02)

"Just the line, for a little while, and then gradually add things." The
whole screen is one row: score, post, line, post, score. Every addition
gets played before the next one goes in. Keep it that way: don't put
several roadmap steps in at once.

## Rules (src/game.rs)

- The court runs 0.0 (left end) to 1.0 (right end), so the game plays the
  same at any width; speeds are in line lengths per second
- Each side's hit zone is 15% of the line, shrinking 1.2% per point that
  side leads, never below 8%
- Hitting at depth d (0 at the zone's inner edge, 1 at your own end)
  multiplies the speed by 0.84 + 0.42 d; d >= 0.92 is a smash (+0.10).
  Clamped to 0.34..2.7
- A press that hits nothing (ball not in your zone or going away) locks
  that side out for 0.32 s; held keys don't repeat into whiffs
- The loser of a point serves; the serve goes by itself after 3 s
- First to 7 (`--to N`)

## Computer (src/cpu.rs)

Picks an aim depth each time the ball comes, mean and spread by
strength, the spread growing with speed. Aim past 1 and the ball gets by;
aim before the zone and it whiffs, gets locked out, and aims deeper.
King against King rallies run to 20 and more.

## Timing

The simulation steps at 240 Hz. Input is read as it arrives and the
simulation is brought up to that moment before the press is applied, so
a press counts on the tick it happened in, not at the next frame.

## Notes

- gilrs reports 3D mice (David's SpaceMouse) as gamepads; only pads with
  SDL mappings are used, `ONE_D_PONG_ANY_PAD=1` allows all
- The 2026-11-02 fuller draft (title, menu, sound, glow, big font) is the
  source for later roadmap steps
