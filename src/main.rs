mod app;
mod cpu;
mod game;
mod render;
mod rng;

use app::{App, Mode, DT};
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

const HELP: &str = "1d-pong [--cpu easy|normal|hard|king] [--two] [--watch] [--to N] [--seed N]

Pong with one dimension. The court is a single line; each side has a hit
zone at its end. Press when the ball is in your zone to send it back: the
later you hit it, the faster it goes. Miss and the point is theirs. Press
outside the zone and you are locked out for a moment.

One player:  SPACE (or A, Z, ←, ENTER) hits; you are on the left
Two players: left A (or Z, S, X, ←), right L (or K, ENTER, →); or two gamepads
P pauses, Q or ESC quits

  --cpu NAME  the computer you play against (default normal)
  --two       two players
  --watch     the computer plays itself
  --to N      points to win a match (1 to 21, default 7)
  --seed N    fixed random seed

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
    let skill = match after("--cpu") {
        None => cpu::Skill::Normal,
        Some(name) => cpu::Skill::parse(&name).unwrap_or_else(|| {
            eprintln!("--cpu takes easy, normal, hard or king");
            std::process::exit(2);
        }),
    };
    let mode = if has("--two") {
        Mode::Duo
    } else if has("--watch") {
        Mode::Watch
    } else {
        Mode::Solo(skill)
    };
    let seed = after("--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64));
    log::info!("start version {} mode {mode:?} target {target} seed {seed}", env!("CARGO_PKG_VERSION"));
    let mut app = App::new(mode, target, seed);

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cmd {
    Hit(Side),
    Pause,
    Quit,
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
                    if let Some(cmd) = key_cmd(app.mode, k) {
                        apply(app, cmd);
                    }
                }
                Event::Resize(w, h) => log::info!("resize {w}x{h}"),
                Event::FocusLost => {
                    if app.mode != Mode::Watch && !app.paused && app.over_for().is_none() {
                        app.paused = true;
                        log::info!("focus lost, paused");
                    }
                }
                Event::FocusGained => log::info!("focus gained"),
                _ => {}
            }
        }
        for cmd in pads.poll(app.mode) {
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

fn key_cmd(mode: Mode, k: KeyEvent) -> Option<Cmd> {
    // only fresh presses: a held key's repeats would whiff
    if k.kind != KeyEventKind::Press {
        return None;
    }
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
        return Some(Cmd::Quit);
    }
    let ch = match k.code {
        KeyCode::Char(c) => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    match (k.code, ch) {
        (KeyCode::Esc, _) | (_, Some('q')) => return Some(Cmd::Quit),
        (_, Some('p')) => return Some(Cmd::Pause),
        _ => {}
    }
    match mode {
        Mode::Solo(_) => match (k.code, ch) {
            (KeyCode::Left | KeyCode::Enter, _) | (_, Some(' ' | 'a' | 'z' | 's' | 'x')) => Some(Cmd::Hit(Side::Left)),
            _ => None,
        },
        Mode::Duo => match (k.code, ch) {
            (KeyCode::Left, _) | (_, Some('a' | 'z' | 's' | 'x')) => Some(Cmd::Hit(Side::Left)),
            (KeyCode::Right | KeyCode::Enter, _) | (_, Some('l' | 'k' | ',' | '.')) => Some(Cmd::Hit(Side::Right)),
            _ => None,
        },
        Mode::Watch => None,
    }
}

fn apply(app: &mut App, cmd: Cmd) {
    log::debug!("cmd {cmd:?}");
    match cmd {
        Cmd::Quit => app.quit = true,
        Cmd::Pause => {
            if app.over_for().is_none() {
                app.paused = !app.paused;
                log::info!("paused {}", app.paused);
            }
        }
        Cmd::Hit(side) => app.press(side),
    }
}

/// Gamepads: the first pad to press a button is the left player, the
/// second the right. In a one-player game every pad plays the left side.
/// Any face, shoulder or d-pad button hits; Start pauses.
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
    fn poll(&mut self, _mode: Mode) -> Vec<Cmd> {
        Vec::new()
    }

    #[cfg(feature = "gamepad")]
    fn poll(&mut self, mode: Mode) -> Vec<Cmd> {
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
                    let side = if mode == Mode::Duo && seat % 2 == 1 { Side::Right } else { Side::Left };
                    match b {
                        Button::Start => out.push(Cmd::Pause),
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
                        | Button::DPadRight => out.push(Cmd::Hit(side)),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        out
    }
}
