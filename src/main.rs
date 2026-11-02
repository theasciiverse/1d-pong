mod app;
mod audio;
mod cpu;
mod font;
mod game;
mod render;
mod rng;
mod store;

use app::{App, Mode, Screen, DT};
use game::Side;
use ratatui::crossterm::event::{
    self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use std::io;
use std::time::{Duration, Instant, SystemTime};

const FRAME: Duration = Duration::from_micros(16_667);
const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);

const HELP: &str = "1d-pong [--to N] [--cpu easy|normal|hard|king] [--two] [--watch] [--mute] [--pure] [--unfold N] [--seed N]

Pong with one dimension. The court is a single line; each side has a hit
zone at its end. Press when the ball is in your zone to send it back: the
later you hit it, the faster it goes. Miss and the point is theirs. Press
outside the zone and you are locked out for a moment.

One player:  SPACE (or A, Z, ←) hits
Two players: left A (or Z, S, X), right L (or K, ENTER, →); or two gamepads
P pauses, M mutes, V switches to the pure view (just the line) and back,
ESC goes back to the menu, Q quits from the menu

The game starts bare, just the line, and unfolds as you play: sound,
the score, the trail, impact, the whole game with its menu, the King of
Lineland, and one more thing. How far it has unfolded is remembered.

  --to N      points to win a match (1 to 21, default 7)
  --cpu NAME  start a one-player match right away against this computer
  --two       start a two-player match right away
  --watch     start with the computer playing itself
  --mute      no sound
  --pure      just the line: no scores above it, no glow, nothing else
  --seed N    fixed random seed
  --unfold N  set how far the game has unfolded, 0 (bare) to 7, or all;
              --unfold 0 starts it over from the bare line

Logging: RUST_LOG=debug 1d-pong 2>1d-pong.log
";

fn main() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let after = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    if has("--help") || has("-h") {
        print!("{HELP}");
        return Ok(());
    }
    if has("--version") || has("-V") {
        println!("1d-pong {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let target = match after("--to").map(|v| v.parse::<u8>()) {
        None => 7,
        Some(Ok(n)) if (1..=21).contains(&n) => n,
        Some(_) => {
            eprintln!("--to takes 1 to 21");
            std::process::exit(2);
        }
    };
    let start_cpu = match after("--cpu") {
        None => None,
        Some(name) => Some(cpu::Skill::parse(&name).unwrap_or_else(|| {
            eprintln!("--cpu takes easy, normal, hard or king");
            std::process::exit(2);
        })),
    };
    let seed = after("--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64));

    // open audio before the alternate screen, so any ALSA chatter lands above it
    let audio = audio::Audio::new(has("--mute"));
    let store = store::Store::load();
    log::info!("start version {} audio {} muted {} seed {seed} target {target} store {store:?}", env!("CARGO_PKG_VERSION"), audio.available(), audio.muted);
    let mut app = App::new(audio, store, target, seed);
    if has("--pure") {
        app.store.pure = true;
    }
    if let Some(v) = after("--unfold") {
        match v.parse::<u8>() {
            Ok(n) if n <= app::level::MAX => app.set_unfold(n),
            _ if v == "all" => app.set_unfold(app::level::MAX),
            _ => {
                eprintln!("--unfold takes 0 to {} or all", app::level::MAX);
                std::process::exit(2);
            }
        }
    }
    log::info!("unfold level {}", app.unfold());
    if let Some(skill) = start_cpu {
        app.skill = skill;
        app.start(Mode::Solo(skill));
    } else if has("--two") {
        app.start(Mode::Duo);
    } else if has("--watch") {
        app.start(Mode::Watch);
    } else if app.unfold() < app::level::FULL {
        // no title or menu yet: straight onto the line, against the gentlest
        app.start(Mode::Solo(cpu::Skill::Easy));
    }

    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    execute!(io::stdout(), EnableFocusChange)?;
    let size = terminal.size()?;
    log::info!("terminal {}x{} kitty_keyboard {kitty}", size.width, size.height);
    let mut pads = Pads::new();
    let result = run(&mut terminal, &mut app, &mut pads);
    let _ = execute!(io::stdout(), DisableFocusChange);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// What a key or button means, before it reaches the app.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cmd {
    Hit(Side),
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Pause,
    Mute,
    Back,
    Quit,
    Start(Mode),
    View,
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, pads: &mut Pads) -> io::Result<()> {
    let dt = Duration::from_secs_f32(DT);
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;
    let mut snapshot = Instant::now();
    let mut next_frame = Instant::now();
    // Advance the simulation to `now` before any input is applied, so a
    // press lands on the tick it happened in, not the next frame's.
    let advance = |app: &mut App, last: &mut Instant, acc: &mut Duration| {
        let now = Instant::now();
        *acc += now.duration_since(*last).min(Duration::from_millis(100));
        *last = now;
        while *acc >= dt {
            *acc -= dt;
            app.step();
        }
    };
    loop {
        let wait = next_frame.saturating_duration_since(Instant::now()).min(Duration::from_millis(4));
        if event::poll(wait)? {
            advance(app, &mut last, &mut acc);
            match event::read()? {
                Event::Key(k) => {
                    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    if let Some(cmd) = key_cmd(app, k) {
                        apply(app, cmd);
                    }
                }
                Event::Resize(w, h) => log::info!("resize {w}x{h}"),
                Event::FocusLost => {
                    if app.screen == Screen::Play && app.play.as_ref().is_some_and(|m| m.mode != Mode::Watch) && !app.paused {
                        app.paused = true;
                        log::info!("focus lost, paused");
                    }
                }
                Event::FocusGained => log::info!("focus gained"),
                _ => {}
            }
        }
        for cmd in pads.poll(app) {
            advance(app, &mut last, &mut acc);
            apply(app, cmd);
        }
        if app.quit {
            return Ok(());
        }
        advance(app, &mut last, &mut acc);
        if Instant::now() >= next_frame {
            terminal.draw(|f| render::draw(app, f.area(), f.buffer_mut()))?;
            next_frame += FRAME;
            if next_frame < Instant::now() {
                next_frame = Instant::now() + FRAME;
            }
        }
        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            log::debug!("snapshot {}", app.snapshot());
        }
    }
}

fn key_cmd(app: &App, k: KeyEvent) -> Option<Cmd> {
    // only fresh presses: a held key's repeats would whiff
    if k.kind != KeyEventKind::Press {
        return None;
    }
    if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
        return Some(Cmd::Quit);
    }
    let ch = match k.code {
        KeyCode::Char(c) => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    if ch == Some('m') {
        return Some(Cmd::Mute);
    }
    if ch == Some('v') {
        return Some(Cmd::View);
    }
    if app.screen == Screen::Title {
        return match (k.code, ch) {
            (KeyCode::Up, _) | (_, Some('w')) | (_, Some('k')) => Some(Cmd::Up),
            (KeyCode::Down, _) | (_, Some('s')) | (_, Some('j')) => Some(Cmd::Down),
            (KeyCode::Left, _) | (_, Some('a')) | (_, Some('h')) => Some(Cmd::Left),
            (KeyCode::Right, _) | (_, Some('d')) | (_, Some('l')) => Some(Cmd::Right),
            (KeyCode::Enter, _) | (_, Some(' ')) => Some(Cmd::Confirm),
            (_, Some('1')) => Some(Cmd::Start(Mode::Solo(app.skill))),
            (_, Some('2')) => Some(Cmd::Start(Mode::Duo)),
            (_, Some('3')) => Some(Cmd::Start(Mode::Watch)),
            (KeyCode::Esc, _) | (_, Some('q')) => Some(Cmd::Quit),
            _ => None,
        };
    }
    let Some(m) = app.play.as_ref() else { return None };
    let over = m.over_for().is_some();
    match (k.code, ch) {
        (KeyCode::Esc, _) => return Some(Cmd::Back),
        (_, Some('p')) => return Some(Cmd::Pause),
        (_, Some('q')) if over || app.paused || m.mode == Mode::Watch => return Some(Cmd::Quit),
        _ => {}
    }
    match m.mode {
        Mode::Solo(_) => match (k.code, ch) {
            (KeyCode::Left | KeyCode::Enter, _) | (_, Some(' ' | 'a' | 'z' | 's' | 'x')) => Some(Cmd::Hit(Side::Left)),
            _ => None,
        },
        Mode::Duo => match (k.code, ch) {
            (KeyCode::Left, _) | (_, Some('a' | 'z' | 's' | 'x')) => Some(Cmd::Hit(Side::Left)),
            (KeyCode::Right | KeyCode::Enter, _) | (_, Some('l' | 'k' | ',' | '.')) => Some(Cmd::Hit(Side::Right)),
            // space only rematches, it belongs to neither player
            (_, Some(' ')) if over => Some(Cmd::Hit(Side::Left)),
            _ => None,
        },
        Mode::Watch => None,
    }
}

fn apply(app: &mut App, cmd: Cmd) {
    log::debug!("cmd {cmd:?}");
    match cmd {
        Cmd::Quit => app.quit = true,
        Cmd::Mute => {
            app.audio.muted = !app.audio.muted;
            log::info!("muted {}", app.audio.muted);
        }
        Cmd::Up => app.menu_move(-1),
        Cmd::Down => app.menu_move(1),
        Cmd::Left => app.skill_move(-1),
        Cmd::Right => app.skill_move(1),
        Cmd::Confirm => app.menu_pick(),
        Cmd::Back => app.to_title(),
        Cmd::Pause => {
            if app.play.as_ref().is_some_and(|m| m.over_for().is_none()) {
                app.paused = !app.paused;
                log::info!("paused {}", app.paused);
            }
        }
        Cmd::Hit(side) => app.press(side),
        Cmd::Start(mode) => app.start(mode),
        Cmd::View => {
            app.store.pure = !app.store.pure;
            app.store.save();
            log::info!("pure view {}", app.store.pure);
        }
    }
}

/// Gamepads: the first pad to press a button is the left player, the
/// second the right. In one-player games every pad plays the left side.
struct Pads {
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    #[cfg(feature = "gamepad")]
    order: Vec<gilrs::GamepadId>,
}

impl Pads {
    fn new() -> Pads {
        #[cfg(feature = "gamepad")]
        {
            let gilrs = match gilrs::Gilrs::new() {
                Ok(g) => Some(g),
                Err(e) => {
                    log::warn!("no gamepads: {e}");
                    None
                }
            };
            Pads { gilrs, order: Vec::new() }
        }
        #[cfg(not(feature = "gamepad"))]
        Pads {}
    }

    #[cfg(not(feature = "gamepad"))]
    fn poll(&mut self, _app: &App) -> Vec<Cmd> {
        Vec::new()
    }

    #[cfg(feature = "gamepad")]
    fn poll(&mut self, app: &App) -> Vec<Cmd> {
        use gilrs::{Button, EventType};
        let mut out = Vec::new();
        let Some(g) = &mut self.gilrs else { return out };
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::Connected => log::info!("gamepad connected: {}", g.gamepad(ev.id).name()),
                EventType::Disconnected => {
                    log::info!("gamepad disconnected");
                    self.order.retain(|&id| id != ev.id);
                }
                EventType::ButtonPressed(b, _) => {
                    log::debug!("pad {:?} button {b:?}", ev.id);
                    // gilrs also reports 3D mice and tablet key pads as
                    // gamepads; only trust the ones SDL knows
                    let any = std::env::var("ONE_D_PONG_ANY_PAD").is_ok_and(|v| v == "1");
                    if !any && g.gamepad(ev.id).mapping_source() != gilrs::MappingSource::SdlMappings {
                        continue;
                    }
                    if !self.order.contains(&ev.id) {
                        self.order.push(ev.id);
                    }
                    let seat = self.order.iter().position(|&id| id == ev.id).unwrap_or(0);
                    let title = app.screen == Screen::Title;
                    let cmd = match b {
                        Button::DPadUp if title => Some(Cmd::Up),
                        Button::DPadDown if title => Some(Cmd::Down),
                        Button::DPadLeft if title => Some(Cmd::Left),
                        Button::DPadRight if title => Some(Cmd::Right),
                        Button::South | Button::Start if title => Some(Cmd::Confirm),
                        Button::East if title => Some(Cmd::Quit),
                        Button::Start => Some(Cmd::Pause),
                        Button::Select => Some(Cmd::Back),
                        Button::South
                        | Button::East
                        | Button::West
                        | Button::North
                        | Button::LeftTrigger
                        | Button::RightTrigger
                        | Button::LeftTrigger2
                        | Button::RightTrigger2
                        | Button::DPadUp
                        | Button::DPadDown
                        | Button::DPadLeft
                        | Button::DPadRight => {
                            let side = match app.play.as_ref().map(|m| m.mode) {
                                Some(Mode::Duo) if seat % 2 == 1 => Side::Right,
                                _ => Side::Left,
                            };
                            Some(Cmd::Hit(side))
                        }
                        _ => None,
                    };
                    out.extend(cmd);
                }
                _ => {}
            }
        }
        out
    }
}
