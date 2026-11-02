# Roadmap

1D-Pong starts absolutely pure: one line across the middle of an empty
terminal, with the scores at its ends. Things come back a little at a
time, each one played before the next goes in (David, 2026-11-02).

A fuller first draft (title screen and menu, big block scores, the ball's
glow and trail, grade popups, screen shake, synthesised sound, saved best
rally) was written the same day and set aside, then came back as 0.2.0
when David said to flesh it out.

## 0.1.0: the line

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

## 0.3.0: paddles and impact (2026-11-02)

- The hit zones drawn as paddles: solid amber bars two half-rows tall, as long as the zone,
  so the leader's shrinking paddle shows; the smash spot at the back brighter
- Each hit swings the paddle forward a cell or two and flashes it; a whiff turns it red for the lockout
- Hit stop: the game holds still for a moment on every hit, 25 ms for an early one up to 110 ms for a smash, and 140 ms on a point
- Sparks off every hit, more and further the better the hit, and a red burst from the post on a point
- The ball drawn round (●) so it never looks like part of a paddle

## Next

- Rally milestones (10, 20, 30 hits): a banner, a sound, the line starting to glow
- Match point tension: a heartbeat, the posts pulsing, a longer trail
- A ripple down the line from each hit; afterimages of the ball near top speed

- Lineland: a story mode after Abbott's Flatland, a companion to the Flatland movie
- Things on the line: blocks, a ghost ball, a split ball, a moving middle wall
- LAN play over iroh
