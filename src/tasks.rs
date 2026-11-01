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
            Some(Todo {
                text: text.to_string(),
                done: flag == "1",
            })
        })
        .collect()
}

pub fn format_todos(items: &[Todo]) -> String {
    items
        .iter()
        .map(|t| format!("{}\t{}\n", if t.done { 1 } else { 0 }, t.text))
        .collect()
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

#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    pub id: u64,
    pub date: String,        // "YYYY-MM-DD"
    pub start_time: String,  // "HH:MM"
    pub end_time: String,    // "HH:MM" (optional)
    pub title: String,       // required
    pub description: String, // optional
}

/// Helper to validate YYYY-MM-DD format
#[allow(dead_code)]
pub fn is_valid_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let Ok(y) = parts[0].parse::<i32>() else {
        return false;
    };
    let Ok(m) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(d) = parts[2].parse::<u32>() else {
        return false;
    };
    if y < 1900 || y > 2100 || m < 1 || m > 12 {
        return false;
    }
    d >= 1 && d <= days_in_month(y, m)
}

/// Helper to validate HH:MM format
#[allow(dead_code)]
pub fn is_valid_time(s: &str) -> bool {
    if s.is_empty() {
        return true;
    } // optional end_time
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return false;
    }
    let Ok(h) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(m) = parts[1].parse::<u32>() else {
        return false;
    };
    h < 24 && m < 60
}

/// Minutes after midnight for a typed time: `9:30`, `09:30`, `9.30`, `21:05`.
pub fn parse_time(token: &str) -> Option<u32> {
    let (h, m) = token.trim().split_once(|c| c == ':' || c == '.')?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// `HH:MM` for minutes after midnight.
pub fn format_time(minutes: u32) -> String {
    let minutes = minutes % (24 * 60);
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Splits an optional leading time off an event title, so typing `9:30 Standup` or
/// `9:30-10:00 Standup` still works next to the time picker: (start, end, title).
pub fn split_leading_time(text: &str) -> (Option<u32>, Option<u32>, String) {
    let text = text.trim();
    let (first, rest) = text.split_once(' ').unwrap_or((text, ""));
    let (start, end) = match first.split_once(|c| c == '-' || c == '\u{2013}') {
        Some((a, b)) => (parse_time(a), parse_time(b)),
        None => (parse_time(first), None),
    };
    match start {
        Some(start) if !rest.trim().is_empty() => (Some(start), end, rest.trim().to_string()),
        _ => (None, None, text.to_string()),
    }
}

pub fn parse_schedules(src: &str) -> Vec<Schedule> {
    let mut items = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 5 {
            continue;
        }
        let Ok(id) = parts[0].parse::<u64>() else {
            continue;
        };
        let date = parts[1].trim().to_string();
        let start_time = parts[2].trim().to_string();
        let end_time = parts[3].trim().to_string();
        let title = parts[4].trim().to_string();
        let description = if parts.len() >= 6 {
            parts[5].trim().to_string()
        } else {
            String::new()
        };
        if title.is_empty() {
            continue;
        }
        items.push(Schedule {
            id,
            date,
            start_time,
            end_time,
            title,
            description,
        });
    }
    items.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then(a.start_time.cmp(&b.start_time))
            .then(a.id.cmp(&b.id))
    });
    items
}

pub fn format_schedules(items: &[Schedule]) -> String {
    let mut out = String::new();
    for s in items {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            s.id, s.date, s.start_time, s.end_time, s.title, s.description
        ));
    }
    out
}

pub fn load_schedules() -> Vec<Schedule> {
    data_path("calendar.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| parse_schedules(&s))
        .unwrap_or_default()
}

pub fn save_schedules(items: &[Schedule]) {
    write_data("calendar.txt", &format_schedules(items));
}

pub fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Day of the week, 0 = Sunday (Sakamoto's method).
pub fn weekday_of(year: i32, month: u32, day: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    ((y + y / 4 - y / 100 + y / 400 + t[(month.clamp(1, 12) - 1) as usize] + day as i32) % 7)
        as u32
}

pub fn first_weekday_of_month(year: i32, month: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let dow = (y + y / 4 - y / 100 + y / 400 + t[(month - 1) as usize] + 1) % 7;
    ((dow + 6) % 7) as u32
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarState {
    pub view_year: i32,
    pub view_month: u32,
    pub selected_year: i32,
    pub selected_month: u32,
    pub selected_day: u32,
    pub schedules: Vec<Schedule>,
}

impl CalendarState {
    pub fn new(year: i32, month: u32, day: u32, schedules: Vec<Schedule>) -> Self {
        let mut s = schedules;
        s.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then(a.start_time.cmp(&b.start_time))
                .then(a.id.cmp(&b.id))
        });
        Self {
            view_year: year,
            view_month: month.clamp(1, 12),
            selected_year: year,
            selected_month: month.clamp(1, 12),
            selected_day: day.clamp(1, days_in_month(year, month.clamp(1, 12))),
            schedules: s,
        }
    }

    pub fn prev_month(&mut self) {
        if self.view_month == 1 {
            self.view_month = 12;
            self.view_year -= 1;
        } else {
            self.view_month -= 1;
        }
    }

    pub fn next_month(&mut self) {
        if self.view_month == 12 {
            self.view_month = 1;
            self.view_year += 1;
        } else {
            self.view_month += 1;
        }
    }

    pub fn select_date(&mut self, year: i32, month: u32, day: u32) {
        let m = month.clamp(1, 12);
        let max_d = days_in_month(year, m);
        self.selected_year = year;
        self.selected_month = m;
        self.selected_day = day.clamp(1, max_d);
    }

    pub fn selected_date_str(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            self.selected_year, self.selected_month, self.selected_day
        )
    }

    pub fn selected_schedules(&self) -> Vec<&Schedule> {
        let d = self.selected_date_str();
        self.schedules.iter().filter(|s| s.date == d).collect()
    }

    pub fn month_has_schedule(&self, year: i32, month: u32, day: u32) -> bool {
        let d = format!("{:04}-{:02}-{:02}", year, month, day);
        self.schedules.iter().any(|s| s.date == d)
    }

    pub fn add_schedule(
        &mut self,
        start_time: &str,
        end_time: &str,
        title: &str,
        description: &str,
    ) -> Option<u64> {
        let title = title.trim();
        if title.is_empty() {
            return None;
        }
        let date = self.selected_date_str();
        let next_id = self.schedules.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        // An empty start time is an all-day event.
        let st = start_time.trim().to_string();
        let et = end_time.trim().to_string();
        self.schedules.push(Schedule {
            id: next_id,
            date,
            start_time: st,
            end_time: et,
            title: title.to_string(),
            description: description.trim().to_string(),
        });
        self.schedules.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then(a.start_time.cmp(&b.start_time))
                .then(a.id.cmp(&b.id))
        });
        save_schedules(&self.schedules);
        Some(next_id)
    }

    pub fn edit_schedule(
        &mut self,
        id: u64,
        start_time: &str,
        end_time: &str,
        title: &str,
        description: &str,
    ) -> bool {
        let title = title.trim();
        if title.is_empty() {
            return false;
        }
        if let Some(pos) = self.schedules.iter().position(|s| s.id == id) {
            self.schedules[pos].start_time = start_time.trim().to_string();
            self.schedules[pos].end_time = end_time.trim().to_string();
            self.schedules[pos].title = title.to_string();
            self.schedules[pos].description = description.trim().to_string();
            self.schedules.sort_by(|a, b| {
                a.date
                    .cmp(&b.date)
                    .then(a.start_time.cmp(&b.start_time))
                    .then(a.id.cmp(&b.id))
            });
            save_schedules(&self.schedules);
            true
        } else {
            false
        }
    }

    pub fn delete_schedule(&mut self, id: u64) -> bool {
        let len_before = self.schedules.len();
        self.schedules.retain(|s| s.id != id);
        if self.schedules.len() != len_before {
            save_schedules(&self.schedules);
            true
        } else {
            false
        }
    }
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
        Self {
            phase: Phase::Focus,
            running: false,
            completed: 0,
            minutes: Phase::DEFAULT_MINUTES,
            end: Instant::now(),
            left: 25. * 60.,
        }
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
        let next =
            (self.minutes[i] as i32 + steps * step as i32).clamp(lo as i32, hi as i32) as u32;
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
        if self.running {
            self.end.saturating_duration_since(now).as_secs_f64()
        } else {
            self.left
        }
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
            self.phase = if self.completed % 4 == 0 {
                Phase::Long
            } else {
                Phase::Short
            };
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
        let items = vec![
            Todo {
                text: "Buy milk".into(),
                done: false,
            },
            Todo {
                text: "Ship notch".into(),
                done: true,
            },
        ];
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
        assert_eq!(
            phases,
            [Short, Focus, Short, Focus, Short, Focus, Long, Focus]
        );
        assert_eq!(p.completed, 4);
    }

    #[test]
    fn pomodoro_start_session_sets_running_and_deadline() {
        let start = Instant::now();
        let mut p = Pomodoro::new();
        assert!(!p.running);
        assert!(!p.active());
        assert_eq!(p.remaining(start), 25. * 60.);

        // Start session
        p.toggle(start);
        assert!(p.running);
        assert!(p.active());
        assert_eq!(p.remaining(start), 25. * 60.);

        // Advance 10 seconds
        let t1 = start + Duration::from_secs(10);
        assert!((p.remaining(t1) - (25. * 60. - 10.)).abs() < 0.01);
    }

    #[test]
    fn pomodoro_pause_session_freezes_time_and_preserves_phase() {
        let start = Instant::now();
        let mut p = Pomodoro::new();
        p.select(Phase::Short);
        assert_eq!(p.phase, Phase::Short);

        // Start
        p.toggle(start);
        assert!(p.running);

        // Advance 15s and pause
        let t1 = start + Duration::from_secs(15);
        p.toggle(t1);
        assert!(!p.running);
        assert!(p.active());
        assert_eq!(p.phase, Phase::Short);
        let preserved_left = p.remaining(t1);
        assert!((preserved_left - (5. * 60. - 15.)).abs() < 0.01);

        // Time passes while paused; remaining duration stays frozen
        let t2 = t1 + Duration::from_secs(45);
        assert!((p.remaining(t2) - preserved_left).abs() < 0.01);
        // Completion tick must return false while paused
        assert!(!p.tick(t2));
    }

    #[test]
    fn pomodoro_resume_session_continues_countdown_from_preserved_time() {
        let start = Instant::now();
        let mut p = Pomodoro::new();

        // Start, advance 30s, pause
        p.toggle(start);
        let t_pause = start + Duration::from_secs(30);
        p.toggle(t_pause);
        assert!(!p.running);
        let paused_left = p.remaining(t_pause);

        // Later resume
        let t_resume = t_pause + Duration::from_secs(120);
        p.toggle(t_resume);
        assert!(p.running);
        assert!((p.remaining(t_resume) - paused_left).abs() < 0.01);

        // Advance 10s after resume
        let t_after = t_resume + Duration::from_secs(10);
        assert!((p.remaining(t_after) - (paused_left - 10.)).abs() < 0.01);
    }

    #[test]
    fn pomodoro_reset_session_restores_duration_from_running_and_paused() {
        let start = Instant::now();
        let mut p = Pomodoro::new();

        // 1. Reset while running
        p.toggle(start);
        let t1 = start + Duration::from_secs(100);
        assert!(p.running);
        assert!(p.remaining(t1) < 25. * 60.);
        p.reset();
        assert!(!p.running);
        assert!(!p.active());
        assert_eq!(p.remaining(t1), 25. * 60.);
        assert!(p.can_adjust().0 || p.can_adjust().1);

        // 2. Reset while paused
        p.toggle(t1);
        let t2 = t1 + Duration::from_secs(50);
        p.toggle(t2); // paused
        assert!(!p.running && p.active());
        p.reset();
        assert!(!p.running);
        assert!(!p.active());
        assert_eq!(p.remaining(t2), 25. * 60.);

        // Phase is preserved across resets
        p.select(Phase::Long);
        p.toggle(t2);
        p.reset();
        assert_eq!(p.phase, Phase::Long);
        assert_eq!(p.remaining(t2), 15. * 60.);
    }

    #[test]
    fn test_calendar_date_selection_and_clamping() {
        let mut cal = CalendarState::new(2026, 11, 1, Vec::new());
        assert_eq!(cal.selected_date_str(), "2026-11-01");
        cal.select_date(2026, 11, 15);
        assert_eq!(cal.selected_date_str(), "2026-11-15");
        // Clamping to max days in month (November has 30 days)
        cal.select_date(2026, 11, 35);
        assert_eq!(cal.selected_date_str(), "2026-11-30");
    }

    #[test]
    fn test_calendar_month_navigation_and_year_boundary() {
        let mut cal = CalendarState::new(2026, 1, 15, Vec::new());
        assert_eq!(cal.view_year, 2026);
        assert_eq!(cal.view_month, 1);

        // Previous month crosses year boundary to Dec 2025
        cal.prev_month();
        assert_eq!(cal.view_year, 2025);
        assert_eq!(cal.view_month, 12);

        // Next month returns to Jan 2026
        cal.next_month();
        assert_eq!(cal.view_year, 2026);
        assert_eq!(cal.view_month, 1);

        // Forward to Dec 2026 and cross to Jan 2027
        for _ in 0..11 {
            cal.next_month();
        }
        assert_eq!(cal.view_year, 2026);
        assert_eq!(cal.view_month, 12);
        cal.next_month();
        assert_eq!(cal.view_year, 2027);
        assert_eq!(cal.view_month, 1);
    }

    #[test]
    fn test_calendar_leap_years_and_weekdays() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2025));
        assert!(!is_leap_year(2026));
        assert!(!is_leap_year(2100)); // century non-leap
        assert!(is_leap_year(2000)); // 400-year leap

        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2026, 11), 30);
        assert_eq!(days_in_month(2026, 12), 31);

        // 2026-11-01 is Sunday -> weekday index 6 (Monday = 0 .. Sunday = 6)
        assert_eq!(first_weekday_of_month(2026, 11), 6);
        // 2024-01-01 is Monday -> weekday index 0
        assert_eq!(first_weekday_of_month(2024, 1), 0);
    }

    #[test]
    fn test_schedule_crud_and_ordering() {
        let mut cal = CalendarState::new(2026, 11, 5, Vec::new());
        // 1. Creation
        let id1 = cal
            .add_schedule("14:00", "15:00", "Team Retro", "Discuss sprint")
            .unwrap();
        assert_eq!(id1, 1);
        let id2 = cal
            .add_schedule("09:30", "10:00", "Daily Standup", "")
            .unwrap();
        assert_eq!(id2, 2);

        // Multiple events on same date ordered by start_time (09:30 before 14:00)
        let events = cal.selected_schedules();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, id2);
        assert_eq!(events[0].title, "Daily Standup");
        assert_eq!(events[1].id, id1);
        assert_eq!(events[1].title, "Team Retro");

        // Schedule indicator on month
        assert!(cal.month_has_schedule(2026, 11, 5));
        assert!(!cal.month_has_schedule(2026, 11, 6));

        // 2. Editing
        let edited = cal.edit_schedule(id1, "16:00", "17:00", "Updated Retro", "New notes");
        assert!(edited);
        let events_after_edit = cal.selected_schedules();
        assert_eq!(events_after_edit[1].start_time, "16:00");
        assert_eq!(events_after_edit[1].title, "Updated Retro");

        // 3. Deletion
        let deleted = cal.delete_schedule(id2);
        assert!(deleted);
        let events_after_del = cal.selected_schedules();
        assert_eq!(events_after_del.len(), 1);
        assert_eq!(events_after_del[0].id, id1);

        // Delete non-existent ID
        assert!(!cal.delete_schedule(9999));
    }

    #[test]
    fn test_schedule_validation_and_parsing() {
        assert!(is_valid_date("2026-11-01"));
        assert!(is_valid_date("2024-02-29")); // leap
        assert!(!is_valid_date("2025-02-29")); // not leap
        assert!(!is_valid_date("bad-date"));
        assert!(!is_valid_date("2026-13-01"));

        assert!(is_valid_time("09:00"));
        assert_eq!(weekday_of(2026, 11, 2), 1); // Monday
        assert_eq!(weekday_of(2024, 2, 29), 4); // Thursday
        assert_eq!(parse_time("9:30"), Some(570));
        assert_eq!(parse_time("09.05"), Some(545));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("930"), None);
        assert_eq!(parse_time("Lunch"), None);
        assert_eq!(format_time(570), "09:30");
        assert_eq!(split_leading_time("9:30 Standup"), (Some(570), None, "Standup".into()));
        assert_eq!(
            split_leading_time("9:30-10:15 Design review"),
            (Some(570), Some(615), "Design review".into())
        );
        assert_eq!(split_leading_time("Lunch with the team"), (None, None, "Lunch with the team".into()));
        assert_eq!(split_leading_time("12:00"), (None, None, "12:00".into()));
        assert!(is_valid_time("23:59"));
        assert!(is_valid_time("")); // optional
        assert!(!is_valid_time("25:00"));
        assert!(!is_valid_time("12:60"));
        assert!(!is_valid_time("bad"));

        // Format & parse round trip
        let schedules = vec![
            Schedule {
                id: 10,
                date: "2026-11-01".into(),
                start_time: "10:00".into(),
                end_time: "11:00".into(),
                title: "Design Review".into(),
                description: "Review notch UI".into(),
            },
            Schedule {
                id: 11,
                date: "2026-11-02".into(),
                start_time: "13:00".into(),
                end_time: "".into(),
                title: "Dentist".into(),
                description: "".into(),
            },
        ];
        let formatted = format_schedules(&schedules);
        let parsed = parse_schedules(&formatted);
        assert_eq!(parsed, schedules);

        // Malformed records handled safely without discarding valid ones
        let malformed = "invalid line without tabs\n12\t2026-11-03\t15:00\t16:00\tValid Event\tDesc\n13\tshort\tline\n\n";
        let parsed_malformed = parse_schedules(malformed);
        assert_eq!(parsed_malformed.len(), 1);
        assert_eq!(parsed_malformed[0].id, 12);
        assert_eq!(parsed_malformed[0].title, "Valid Event");
    }
}
