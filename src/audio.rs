//! Sounds, all made here as square and triangle waves: no sound files.

use rodio::buffer::SamplesBuffer;
use rodio::{MixerDeviceSink, Source};
use std::num::NonZero;

const RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug)]
pub enum Sound {
    /// a hit; the pitch climbs with the ball's speed (0..1)
    Hit(f32),
    Smash,
    Whiff,
    Serve,
    /// a point; true when the player at this keyboard (or pad) won it
    Point(bool),
    Win,
    Lose,
    Menu,
}

pub struct Audio {
    sink: Option<MixerDeviceSink>,
    pub muted: bool,
}

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

/// One note: frequency sweeps from `f0` to `f1` over `secs`, with a short
/// attack and a linear decay.
fn note(out: &mut Vec<f32>, wave: Wave, f0: f32, f1: f32, secs: f32, vol: f32) {
    let n = (secs * RATE as f32) as usize;
    let mut phase = 0.0f32;
    let mut seed = 0x9e37_79b9u32;
    for i in 0..n {
        let k = i as f32 / n as f32;
        let f = f0 + (f1 - f0) * k;
        phase = (phase + f / RATE as f32).fract();
        let s = match wave {
            Wave::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
            Wave::Noise => {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            }
        };
        let attack = (i as f32 / (RATE as f32 * 0.003)).min(1.0);
        out.push(s * vol * attack * (1.0 - k));
    }
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut s) => {
                s.log_on_drop(false);
                Some(s)
            }
            Err(e) => {
                log::warn!("no audio: {e}");
                None
            }
        };
        Audio { sink, muted }
    }

    pub fn available(&self) -> bool {
        self.sink.is_some()
    }

    pub fn play(&self, s: Sound) {
        if self.muted {
            return;
        }
        let Some(sink) = &self.sink else { return };
        let mut v = Vec::new();
        match s {
            Sound::Hit(heat) => {
                let f = 330.0 + heat * 520.0;
                note(&mut v, Wave::Square, f, f * 1.02, 0.07, 0.22);
            }
            Sound::Smash => {
                note(&mut v, Wave::Square, 990.0, 1320.0, 0.05, 0.25);
                note(&mut v, Wave::Noise, 0.0, 0.0, 0.12, 0.18);
            }
            Sound::Whiff => note(&mut v, Wave::Triangle, 160.0, 110.0, 0.06, 0.3),
            Sound::Serve => note(&mut v, Wave::Square, 660.0, 660.0, 0.04, 0.15),
            Sound::Point(good) => {
                if good {
                    note(&mut v, Wave::Square, 523.0, 523.0, 0.08, 0.18);
                    note(&mut v, Wave::Square, 784.0, 784.0, 0.16, 0.18);
                } else {
                    note(&mut v, Wave::Triangle, 330.0, 110.0, 0.35, 0.35);
                }
            }
            Sound::Win => {
                for f in [523.0, 659.0, 784.0, 1047.0] {
                    note(&mut v, Wave::Square, f, f, 0.11, 0.18);
                }
                note(&mut v, Wave::Square, 1047.0, 1047.0, 0.35, 0.18);
            }
            Sound::Lose => {
                for f in [392.0, 330.0, 262.0] {
                    note(&mut v, Wave::Triangle, f, f, 0.16, 0.3);
                }
                note(&mut v, Wave::Triangle, 196.0, 150.0, 0.5, 0.3);
            }
            Sound::Menu => note(&mut v, Wave::Square, 880.0, 880.0, 0.025, 0.1),
        }
        let buf = SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(RATE).unwrap(), v);
        sink.mixer().add(buf.amplify(0.8));
    }
}
