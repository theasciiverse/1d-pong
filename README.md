# 1D-Pong

Pong with one dimension, in the terminal.

The court is a single line. Each side has a hit zone at its end. Press
when the ball is in your zone to send it back: the later you hit it, the
faster it goes. Miss it and the point is theirs. Press outside your zone
and you are locked out for a moment. First to 7 wins.

```
  3 ┃━━━━━━━━━━━━━━━━━━━━━━━━━━━━━●━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┃ 5
```

The game starts with nothing but the line and unfolds as you play. How
far it has unfolded is remembered; `--unfold all` skips ahead, `--unfold 0`
starts over.

## Play

```bash
cargo run --release              # you against the computer
cargo run --release -- --two     # two players on one keyboard
cargo run --release -- --watch   # the computer plays itself
cargo run --release -- --help    # everything else
```

One player: SPACE hits (A, Z, ← and ENTER too). Two players: left A,
right L, or two gamepads. P pauses, M mutes, ESC goes back to the menu.
V switches to the pure view, just the line and nothing else, and back.

The computer comes in four strengths: `--cpu easy`, `normal` (default),
`hard` and `king`, the King of Lineland.

Run with `RUST_LOG=debug 1d-pong 2>1d-pong.log` to log every key, press
and a snapshot of the match twice a second.

## Licence

MIT, see LICENSE.
