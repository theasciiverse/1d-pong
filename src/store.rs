//! What carries over between runs: the last difficulty picked, the best
//! rally, whether the King was beaten, and the pure view. Kept in
//! $XDG_DATA_HOME/1d-pong/state (~/.local/share/1d-pong/state, or
//! %APPDATA%\1d-pong\state on Windows) as plain `key=value` lines.

use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Store {
    pub skill: Option<String>,
    pub best_rally: u32,
    pub king_beaten: bool,
    pub pure: bool,
}

fn path() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = env("XDG_DATA_HOME")
        .or_else(|| if cfg!(windows) { env("APPDATA") } else { None })
        .or_else(|| env("HOME").map(|h| h.join(".local/share")))?;
    Some(base.join("1d-pong").join("state"))
}

impl Store {
    pub fn load() -> Store {
        let mut s = Store::default();
        let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "skill" => s.skill = Some(v.trim().to_string()),
                "best_rally" => s.best_rally = v.trim().parse().unwrap_or(0),
                "king_beaten" => s.king_beaten = v.trim() == "true",
                "pure" => s.pure = v.trim() == "true",
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        let text = format!(
            "skill={}\nbest_rally={}\nking_beaten={}\npure={}\n",
            self.skill.as_deref().unwrap_or("normal"),
            self.best_rally,
            self.king_beaten,
            self.pure
        );
        let result = p.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&p, text));
        if let Err(e) = result {
            log::warn!("can't save to {}: {e}", p.display());
        }
    }
}
