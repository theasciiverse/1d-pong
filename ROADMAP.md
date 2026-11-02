# Roadmap

1D-Pong starts absolutely pure: one line across the middle of an empty
terminal, with the scores at its ends. Things come back a little at a
time, each one played before the next goes in (David, 2026-11-02).

A fuller first draft (title screen and menu, big block scores, the ball's
glow and trail, grade popups, screen shake, synthesised sound, saved best
rally) was written the same day and set aside; its code, on the `full-draft` branch, is the source
for the steps below, not a plan to restore it all at once.

## 0.1.0: the line (now)

- The line, the posts, the ball and the two scores, all on one row
- Hit zones shown in amber; the leader's zone shrinks a little per point ahead
- Late hits go back faster, early ones slower; a press outside the zone locks you out
- One player against the computer (easy, normal, hard, king), two players, watch
- Keyboard and gamepads; session logging

## 0.2.0: fleshed out (2026-11-02)

- Sound, all synthesised: hit blips that climb with speed, smash, whiff, serve, points, win and loss
- The ball heats from green through amber and red to white hot as rallies speed up
- A trail behind the ball, and its light leaking out of the line in half blocks
- Grades for each hit (early, good, GREAT, SMASH!), a shake on smashes and points
- Big block scores, a rally counter with a speed meter
- Title screen with the computer playing itself behind the menu
- Best rally and beating the King of Lineland, remembered between runs
- The pure view stays: V (or `--pure`) switches back to just the line, and it is remembered

## Next

- Lineland: a story mode after Abbott's Flatland, a companion to the Flatland movie
- Things on the line: blocks, a ghost ball, a split ball, a moving middle wall
- LAN play over iroh
