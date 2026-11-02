# Roadmap

1D-Pong starts absolutely pure: one line across the middle of an empty
terminal, with the scores at its ends. Things come back a little at a
time, each one played before the next goes in (David, 2026-11-02).

A fuller first draft (title screen and menu, big block scores, the ball's
glow and trail, grade popups, screen shake, synthesised sound, saved best
rally) was written the same day and set aside; its code is the source
for the steps below, not a plan to restore it all at once.

## 0.1.0: the line (now)

- The line, the posts, the ball and the two scores, all on one row
- Hit zones shown in amber; the leader's zone shrinks a little per point ahead
- Late hits go back faster, early ones slower; a press outside the zone locks you out
- One player against the computer (easy, normal, hard, king), two players, watch
- Keyboard and gamepads; session logging

## Next, one at a time

- Sound: hit blips that climb with speed, a smash, a whiff, points and the win
- The ball's colour heating from green through amber to red as rallies speed up
- A trail behind the ball
- Light leaking out of the line around the ball (half blocks above and below)
- Grades for each hit (early, good, great, smash)
- Big scores and a title screen with the menu
- Best rally and beating the King of Lineland, remembered between runs
- Lineland: a story mode after Abbott's Flatland, a companion to the Flatland movie
- Things on the line: blocks, a ghost ball, a split ball, a moving middle wall
- LAN play over iroh
