//! To-do list and Pomodoro timer state, kept free of Win32 so it can be unit tested.
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Todo {
    pub text: String,
    pub done: bool,
}

/// A file in `%APPDATA%\Wisp`, where everything that survives a restart is kept.
pub fn data_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("Wisp").join(name))
}

/// Writes a data file, creating the folder first; errors are ignored (the next change retries).
pub fn write_data(name: &str, contents: &str) {
    if let Some(path) = data_path(name) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, contents);
    }
}

/// `todos.txt`: one task per line as `<0|1>\t<text>`.
fn todos_path() -> Option<PathBuf> {
    data_path("todos.txt")
}

pub fn parse_todos(src: &str) -> Vec<Todo> {
    src.lines()
        .filter_map(|line| {
            let (flag, text) = line.split_once('\t')?;
            if text.trim().is_empty() {
                return None;
            }
            Some(Todo { text: text.to_string(), done: flag == "1" })
        })
        .collect()
}

pub fn format_todos(items: &[Todo]) -> String {
    items.iter().map(|t| format!("{}\t{}\n", if t.done { 1 } else { 0 }, t.text)).collect()
}

pub fn load_todos() -> Vec<Todo> {
    todos_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|src| parse_todos(&src))
        .unwrap_or_default()
}

pub fn save_todos(items: &[Todo]) {
    write_data("todos.txt", &format_todos(items));
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Focus,
    Short,
    Long,
}

impl Phase {
    pub const ALL: [Phase; 3] = [Phase::Focus, Phase::Short, Phase::Long];
    /// Default length in minutes: classic 25 / 5 / 15.
    pub const DEFAULT_MINUTES: [u32; 3] = [25, 5, 15];
    /// Adjustable range and step in minutes, per phase: focus and long breaks in 5s, short breaks in 1s.
    pub const LIMITS: [(u32, u32, u32); 3] = [(5, 90, 5), (1, 30, 1), (5, 60, 5)];
    fn index(self) -> usize {
        match self {
            Phase::Focus => 0,
            Phase::Short => 1,
            Phase::Long => 2,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Phase::Focus => "FOCUS",
            Phase::Short => "SHORT BREAK",
            Phase::Long => "LONG BREAK",
        }
    }
}

/// Pomodoro: focus, a short break, and a long break after every fourth focus. Lengths default to
/// 25 / 5 / 15 minutes and can be adjusted between sessions.
pub struct Pomodoro {
    pub phase: Phase,
    pub running: bool,
    pub completed: u32,
    minutes: [u32; 3],
    end: Instant,
    left: f64,
}

impl Pomodoro {
    pub fn new() -> Self {
        Self { phase: Phase::Focus, running: false, completed: 0, minutes: Phase::DEFAULT_MINUTES, end: Instant::now(), left: 25. * 60. }
    }
    pub fn minutes(&self, phase: Phase) -> u32 {
        self.minutes[phase.index()]
    }
    pub fn all_minutes(&self) -> [u32; 3] {
        self.minutes
    }
    pub fn secs(&self, phase: Phase) -> f64 {
        self.minutes(phase) as f64 * 60.
    }
    /// Restores saved lengths (clamped to each phase's range); a session in progress keeps its time.
    pub fn set_minutes(&mut self, minutes: [u32; 3]) {
        for (i, m) in minutes.into_iter().enumerate() {
            let (lo, hi, _) = Phase::LIMITS[i];
            self.minutes[i] = m.clamp(lo, hi);
        }
        if !self.active() {
            self.left = self.secs(self.phase);
        }
    }
    /// Lengthens (+1) or shortens (-1) the selected phase by one step. Only between sessions;
    /// returns whether anything changed.
    pub fn adjust(&mut self, steps: i32) -> bool {
        if self.active() {
            return false;
        }
        let i = self.phase.index();
        let (lo, hi, step) = Phase::LIMITS[i];
        let next = (self.minutes[i] as i32 + steps * step as i32).clamp(lo as i32, hi as i32) as u32;
        if next == self.minutes[i] {
            return false;
        }
        self.minutes[i] = next;
        self.left = self.secs(self.phase);
        true
    }
    /// Whether the selected phase can still be shortened / lengthened.
    pub fn can_adjust(&self) -> (bool, bool) {
        let (lo, hi, _) = Phase::LIMITS[self.phase.index()];
        let m = self.minutes(self.phase);
        (!self.active() && m > lo, !self.active() && m < hi)
    }
    pub fn remaining(&self, now: Instant) -> f64 {
        if self.running { self.end.saturating_duration_since(now).as_secs_f64() } else { self.left }
    }
    pub fn fraction(&self, now: Instant) -> f64 {
        (self.remaining(now) / self.secs(self.phase)).clamp(0., 1.)
    }
    /// Running, or paused part-way through a phase.
    pub fn active(&self) -> bool {
        self.running || self.left < self.secs(self.phase)
    }
    pub fn toggle(&mut self, now: Instant) {
        if self.running {
            self.left = self.remaining(now);
            self.running = false;
        } else {
            self.end = now + Duration::from_secs_f64(self.left);
            self.running = true;
        }
    }
    pub fn reset(&mut self) {
        self.running = false;
        self.left = self.secs(self.phase);
    }
    pub fn select(&mut self, phase: Phase) {
        self.phase = phase;
        self.reset();
    }
    /// Moves to the next phase once the running one has ended; true once per completion.
    pub fn tick(&mut self, now: Instant) -> bool {
        if !self.running || self.remaining(now) > 0. {
            return false;
        }
        if self.phase == Phase::Focus {
            self.completed += 1;
            self.phase = if self.completed % 4 == 0 { Phase::Long } else { Phase::Short };
        } else {
            self.phase = Phase::Focus;
        }
        self.reset();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_round_trip() {
        let items = vec![Todo { text: "Buy milk".into(), done: false }, Todo { text: "Ship notch".into(), done: true }];
        let parsed = parse_todos(&format_todos(&items));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "Buy milk");
        assert!(!parsed[0].done);
        assert!(parsed[1].done);
        assert!(parse_todos("garbage\n1\t  \n").is_empty());
    }

    #[test]
    fn pause_keeps_remaining_time() {
        let start = Instant::now();
        let mut p = Pomodoro::new();
        assert!(!p.active());
        p.toggle(start);
        p.toggle(start + Duration::from_secs(60));
        assert!(!p.running && p.active());
        assert!((p.remaining(start) - (25. * 60. - 60.)).abs() < 0.01);
        p.reset();
        assert!(!p.active());
    }

    #[test]
    fn durations_adjust_between_sessions_only() {
        let mut p = Pomodoro::new();
        assert!(p.adjust(1));
        assert_eq!(p.minutes(Phase::Focus), 30);
        assert!((p.remaining(Instant::now()) - 30. * 60.).abs() < 0.01);
        // Clamped to the range, in the phase's step.
        for _ in 0..40 {
            p.adjust(1);
        }
        assert_eq!(p.minutes(Phase::Focus), 90);
        assert!(!p.adjust(1));
        p.select(Phase::Short);
        assert!(p.adjust(-1));
        assert_eq!(p.minutes(Phase::Short), 4);
        // Not while a session is running or paused part-way.
        p.toggle(Instant::now());
        assert!(!p.adjust(1));
        assert_eq!(p.can_adjust(), (false, false));
        // Saved values are clamped on load.
        let mut q = Pomodoro::new();
        q.set_minutes([200, 0, 15]);
        assert_eq!(q.all_minutes(), [90, 1, 15]);
    }

    #[test]
    fn phases_cycle_with_long_break_every_fourth_focus() {
        let mut p = Pomodoro::new();
        let mut now = Instant::now();
        let mut phases = Vec::new();
        for _ in 0..8 {
            p.toggle(now);
            assert!(!p.tick(now));
            now += Duration::from_secs_f64(p.secs(p.phase) + 1.);
            assert!(p.tick(now));
            phases.push(p.phase);
        }
        use Phase::*;
        assert_eq!(phases, [Short, Focus, Short, Focus, Short, Focus, Long, Focus]);
        assert_eq!(p.completed, 4);
    }
}
