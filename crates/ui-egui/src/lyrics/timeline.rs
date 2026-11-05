//! When a sung line plays and how its lettered items move, ported from the web UI's
//! `WordsTimelineController` (`apps/magic-cli/ui/words-design.js`). Pure functions only.

use super::model::{Animation, Item, StudioTrack, Word};

/// One sung line of a section: its id and its words, in sung order.
pub struct Line<'a> {
    pub id: &'a str,
    pub words: Vec<&'a Word>,
}

/// The section's lines, in the order their words are sung.
pub fn lines(anim: &Animation) -> Vec<Line<'_>> {
    let mut out: Vec<Line<'_>> = Vec::new();
    for w in &anim.words {
        match out.last_mut() {
            Some(line) if line.id == w.line_id => line.words.push(w),
            _ => out.push(Line { id: &w.line_id, words: vec![w] }),
        }
    }
    out
}

/// A word's time: its clip on the video's lyric track when it has one, else its sung start.
pub fn word_at(track: &StudioTrack, anim: &Animation, w: &Word) -> f64 {
    track
        .section_of(anim)
        .and_then(|s| s.clips.iter().find(|c| c.word_id == w.word_id))
        .map_or(w.start, |c| c.start)
}

/// The section's span on the song clock.
pub fn section_span(track: &StudioTrack, anim: &Animation) -> (f64, f64) {
    if let Some(s) = track.section_of(anim) {
        return (s.start, s.end);
    }
    let start = anim.words.iter().map(|w| word_at(track, anim, w)).fold(f64::INFINITY, f64::min);
    let end = anim.words.iter().map(|w| w.end).fold(f64::NEG_INFINITY, f64::max);
    if start.is_finite() && end.is_finite() { (start, end) } else { (0.0, 0.0) }
}

/// A line's span: from its first word to the next line's first word (or the section's end).
pub fn line_span(track: &StudioTrack, anim: &Animation, line_id: &str) -> (f64, f64) {
    let all = lines(anim);
    let Some(k) = all.iter().position(|l| l.id == line_id) else {
        return section_span(track, anim);
    };
    let start = word_at(track, anim, all[k].words[0]);
    let end = match all.get(k + 1) {
        Some(next) => word_at(track, anim, next.words[0]),
        None => section_span(track, anim).1,
    };
    (start, end)
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}
pub fn ease_out2(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(2)
}
pub fn ease_out3(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}
pub fn ease_sin(x: f32) -> f32 {
    0.5 - 0.5 * (std::f32::consts::PI * x).cos()
}

/// The pixel offset an item's motion has reached: `settled` and `arc` run 0→1 from its accent.
pub fn motion_offset(motion: &str, settled: f32, arc: f32) -> (f32, f32) {
    use std::f32::consts::PI;
    match motion {
        "settle" => (0.0, 6.0 * settled),
        "fall" => (0.0, 20.0 * settled),
        "rise" => (24.0 * settled, -22.0 * (arc * PI).sin() - 18.0 * settled),
        "east" => (18.0 * settled, 0.0),
        "west" => (-18.0 * settled, 0.0),
        "loop" => (12.0 * (arc * PI * 2.0).sin(), 7.0 * ((arc * PI * 2.0).cos() - 1.0)),
        _ => (0.0, 0.0),
    }
}

/// The "pop" swell on the accent: up to 1.12 over 0.13 s, back to 1 over the next 0.3 s.
pub fn pulse(local: f32, a: f32) -> f32 {
    if local < a || local >= a + 0.43 {
        1.0
    } else if local < a + 0.13 {
        1.0 + 0.12 * (local - a) / 0.13
    } else {
        1.12 - 0.12 * (local - a - 0.13) / 0.3
    }
}

/// Where an item is in its motion at line-local time `local`.
pub struct Pose {
    pub opacity: f32,
    pub tx: f32,
    pub ty: f32,
    pub sx: f32,
    pub sy: f32,
}

/// The pose of `it` at `local` seconds into its line, its accent word `a` seconds in.
pub fn pose(it: &Item, local: f32, a: f32) -> Pose {
    let settled = ease_out3(clamp01((local - a) / 0.65));
    let arc = clamp01((local - a) / 0.75);
    let swell = pulse(local, a);
    let (tx, ty) = motion_offset(&it.motion, settled, arc);
    let sx = if it.motion == "pop" { swell } else { 1.0 };
    let sy = match it.motion.as_str() {
        "pop" => swell,
        "settle" => 1.0 - 0.035 * settled,
        _ => 1.0,
    };
    Pose { opacity: ease_out2(clamp01((local + 0.4) / 0.4)), tx, ty, sx, sy }
}

/// For the "sleep" motion: letter `k`'s drop (px) and tilt (degrees).
pub fn sleep_letter(k: usize, local: f32, a: f32) -> (f32, f32) {
    let u = ease_sin(clamp01((local - a - k as f32 * 0.035) / 0.8));
    let ty = 7.0 * u + 3.0 * (k as f32 * 0.6).sin() * (u * std::f32::consts::PI).sin();
    (ty, -0.6 * u)
}

/// A stroke's drawn fraction at `local`, drawing on between its `from` and `to`.
pub fn drawn(local: f32, from: f32, to: f32) -> f32 {
    clamp01((local - from) / (to - from).max(0.01))
}

/// An item counts as the hero when it is large or pops.
pub fn is_hero(it: &Item) -> bool {
    it.size >= 140.0 || it.motion == "pop"
}

/// `m:ss.s`, the web UI's song clock.
pub fn clock(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:04.1}", (t / 60.0).floor() as u64, t % 60.0)
}
