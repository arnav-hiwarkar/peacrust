use crate::cava::Cava;
use crate::clock::{self, Counter};
use crate::color;
use crate::command::{self, Action, CounterOp};
use crate::config::{Config, Mode, TARGETS, Target, Toggle, View, name_of};
use crate::prompt::{self, Edit, Prompt};
use crate::render::{self, Bottom, Grid, Panel, Scene};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geo {
    Ratio,
    Margin,
    Padding,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    Geo(Geo),
    Color(Target),
}

pub enum CavaState {
    Off,
    Running(Cava),
    Failed(String),
}

pub struct App {
    pub cfg: Config,
    pub timer: Counter,
    pub stopwatch: Counter,
    timer_fired: bool,
    pub prompt: Prompt,
    pub prompt_open: bool,
    status: Option<(String, Target, Instant)>,
    pub select: Option<Select>,
    pub cava: CavaState,
    pub cava_bin: String,
    history_path: Option<PathBuf>,
    depth: u8,
    pub quit: bool,
}

pub fn expand(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn run_detached(cmd: &str) {
    let child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        std::thread::spawn(move || child.wait());
    }
}

impl App {
    pub fn new(history_path: Option<PathBuf>) -> App {
        let history = history_path
            .as_deref()
            .map(prompt::load_history)
            .unwrap_or_default();
        App {
            cfg: Config::default(),
            timer: Counter::default(),
            stopwatch: Counter::default(),
            timer_fired: false,
            prompt: Prompt::with_history(history),
            prompt_open: false,
            status: None,
            select: None,
            cava: CavaState::Off,
            cava_bin: "cava".into(),
            history_path,
            depth: 0,
            quit: false,
        }
    }

    pub fn timer_left(&self, now: Instant) -> Duration {
        self.cfg.timer.saturating_sub(self.timer.elapsed(now))
    }

    pub fn stopwatch_value(&self, now: Instant) -> Duration {
        self.cfg.stopwatch + self.stopwatch.elapsed(now)
    }

    pub fn animating(&self) -> bool {
        matches!(self.cava, CavaState::Running(_))
    }

    fn set_status(&mut self, msg: impl Into<String>, t: Target, now: Instant) {
        self.status = Some((msg.into(), t, now));
    }

    fn report(&mut self, r: Result<String, String>, now: Instant) {
        match r {
            Ok(m) if m.is_empty() => self.status = None,
            Ok(m) => self.set_status(m, Target::Success, now),
            Err(e) => self.set_status(format!("error: {e}"), Target::Error, now),
        }
    }

    /// Timer completion and cava liveness. Call once per loop iteration.
    pub fn tick(&mut self, now: Instant) {
        if self.timer.running() && self.timer_left(now).is_zero() {
            self.timer.stop(now);
            if !self.timer_fired {
                self.timer_fired = true;
                if !self.cfg.timer_exec.is_empty() {
                    run_detached(&self.cfg.timer_exec);
                }
                self.set_status("timer complete", Target::Success, now);
            }
        }
        if let CavaState::Running(c) = &self.cava
            && !c.alive()
        {
            self.cava = CavaState::Failed(c.exit_message());
        }
    }

    fn counter_op(&mut self, timer: bool, op: CounterOp, now: Instant) {
        let c = if timer {
            &mut self.timer
        } else {
            &mut self.stopwatch
        };
        match op {
            CounterOp::Start => c.start(now),
            CounterOp::Stop => c.stop(now),
            CounterOp::Clear => c.clear(),
        }
        if timer && op == CounterOp::Clear {
            self.timer_fired = false;
        }
    }

    pub fn sync_cava(&mut self) {
        if !self.cfg.on(Toggle::Cava) {
            self.cava = CavaState::Off;
            return;
        }
        let respawn = match &self.cava {
            CavaState::Off => true,
            CavaState::Running(c) => c.bars != self.cfg.cava_bars,
            CavaState::Failed(_) => false,
        };
        if respawn {
            self.cava = CavaState::Off;
            self.cava = match Cava::spawn(&self.cava_bin, self.cfg.cava_bars) {
                Ok(c) => CavaState::Running(c),
                Err(e) => CavaState::Failed(e.to_string()),
            };
        }
    }

    /// Run one command line. Returns the status message on success.
    pub fn run_line(&mut self, line: &str, now: Instant) -> Result<String, String> {
        let action = command::execute(line, &mut self.cfg)?;
        let out = match action {
            Action::None => Ok(String::new()),
            Action::Quit => {
                self.quit = true;
                Ok(String::new())
            }
            Action::Help(term) => help(&term),
            Action::MkConfig { path, force } => {
                let p = expand(&path);
                if p.exists() && !force {
                    Err(format!("{path} exists, use mkconfig! to overwrite"))
                } else {
                    std::fs::write(&p, self.cfg.to_commands())
                        .map(|_| format!("wrote {path}"))
                        .map_err(|e| format!("{path}: {e}"))
                }
            }
            Action::Source(path) => self
                .source(&expand(&path), now)
                .map(|_| format!("loaded {path}")),
            Action::Timer(op) => {
                self.counter_op(true, op, now);
                Ok(String::new())
            }
            Action::Stopwatch(op) => {
                self.counter_op(false, op, now);
                Ok(String::new())
            }
        };
        self.sync_cava();
        out
    }

    /// Run a command file. Bad lines are skipped and reported together.
    pub fn source(&mut self, path: &Path, now: Instant) -> Result<(), String> {
        if self.depth >= 8 {
            return Err("source nested too deeply".into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.depth += 1;
        let errors: Vec<String> = text
            .lines()
            .enumerate()
            .filter_map(|(i, l)| {
                self.run_line(l, now)
                    .err()
                    .map(|e| format!("line {}: {e}", i + 1))
            })
            .collect();
        self.depth -= 1;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("{}: {}", path.display(), errors.join("; ")))
        }
    }

    pub fn startup_error(&mut self, e: String, now: Instant) {
        self.report(Err(e), now);
    }

    pub fn key(&mut self, k: KeyEvent, now: Instant) {
        if k.kind == KeyEventKind::Release {
            return;
        }
        if self.prompt_open {
            return self.prompt_key(k, now);
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let KeyCode::Char(c) = k.code else {
            match k.code {
                KeyCode::Esc => {
                    self.status = None;
                    self.select = None;
                }
                KeyCode::Backspace => self.counter_key(CounterOp::Clear, now),
                _ => {}
            }
            return;
        };
        if ctrl {
            if c == 'c' {
                self.quit = true;
            }
            return;
        }
        match c {
            'q' | 'Q' => self.quit = true,
            ':' => {
                self.prompt_open = true;
                self.prompt.reset();
            }
            ' ' => self.counter_key(CounterOp::Start, now),
            'w' => self.cfg.mode = Mode::Clock,
            'e' => self.cfg.mode = Mode::Timer,
            'r' => self.cfg.mode = Mode::Stopwatch,
            'W' => self.cfg.view = View::Date,
            'E' => self.cfg.view = View::Ascii,
            'R' => self.cfg.view = View::Digital,
            'T' => self.cfg.view = View::Binary,
            'Y' => self.cfg.view = View::Icon,
            'a' => self.flip(Toggle::Hour24),
            's' => self.flip(Toggle::Seconds),
            'd' => self.flip(Toggle::Date),
            'f' => self.flip(Toggle::AutoSize),
            'g' => self.flip(Toggle::AutoRatio),
            'z' => {
                self.flip(Toggle::Cava);
                self.sync_cava();
            }
            _ => self.adjust_key(c, now),
        }
    }

    fn flip(&mut self, t: Toggle) {
        self.cfg.toggles[t as usize] ^= true;
    }

    fn counter_key(&mut self, op: CounterOp, now: Instant) {
        let timer = match self.cfg.mode {
            Mode::Clock => return,
            m => m == Mode::Timer,
        };
        let c = if timer { &self.timer } else { &self.stopwatch };
        let op = if op == CounterOp::Start && c.running() {
            CounterOp::Stop
        } else {
            op
        };
        self.counter_op(timer, op, now);
    }

    fn adjust_key(&mut self, c: char, now: Instant) {
        let pick = match c {
            'u' => Some(Select::Geo(Geo::Ratio)),
            'i' => Some(Select::Geo(Geo::Margin)),
            'o' => Some(Select::Geo(Geo::Padding)),
            'p' => Some(Select::Geo(Geo::Block)),
            'x' => Some(Select::Color(Target::ActiveFg)),
            'c' => Some(Select::Color(Target::InactiveFg)),
            'v' => Some(Select::Color(Target::ColonFg)),
            'b' => Some(Select::Color(Target::ActiveBg)),
            'n' => Some(Select::Color(Target::InactiveBg)),
            'm' => Some(Select::Color(Target::ColonBg)),
            ',' => Some(Select::Color(Target::Background)),
            '.' => Some(Select::Color(Target::Date)),
            _ => None,
        };
        if let Some(s) = pick {
            self.select = Some(s);
            let name = match s {
                Select::Geo(g) => ["ratio", "margin", "padding", "block"][g as usize],
                Select::Color(t) => name_of(&TARGETS, t),
            };
            return self.set_status(
                format!("adjust {name}: h/l j/k ; ' /"),
                Target::Success,
                now,
            );
        }
        let (dx, dy, dl): (i32, i32, i32) = match c {
            'h' => (-1, 0, 0),
            'l' => (1, 0, 0),
            'j' => (0, -1, 0),
            'k' => (0, 1, 0),
            ';' => (0, 0, -1),
            '\'' => (0, 0, 1),
            '/' => (0, 0, 0),
            _ => return,
        };
        match self.select {
            Some(Select::Geo(g)) if dl == 0 && c != '/' => {
                let min = if matches!(g, Geo::Ratio | Geo::Block) {
                    1
                } else {
                    0
                };
                let cfg = &mut self.cfg;
                let v = match g {
                    Geo::Ratio => {
                        cfg.toggles[Toggle::AutoRatio as usize] = true;
                        &mut cfg.ratio
                    }
                    Geo::Margin => &mut cfg.margin,
                    Geo::Padding => &mut cfg.padding,
                    Geo::Block => {
                        cfg.toggles[Toggle::AutoSize as usize] = false;
                        cfg.toggles[Toggle::AutoRatio as usize] = false;
                        &mut cfg.block
                    }
                };
                let step = |n: u16, d: i32| (n as i32 + d).clamp(min, u16::MAX as i32) as u16;
                *v = (step(v.0, dx), step(v.1, dy));
                let msg = format!("{} {}", v.0, v.1);
                self.set_status(msg, Target::Success, now);
            }
            Some(Select::Color(t)) => {
                let style = &mut self.cfg.styles[t as usize];
                if c == '/' {
                    *style = Default::default();
                    return self.set_status("cleared", Target::Success, now);
                }
                let col = color::adjust(
                    style.color,
                    dx as f64 * 10.0,
                    dy as f64 * 5.0,
                    dl as f64 * 5.0,
                );
                style.color = Some(col);
                self.set_status(col.to_string(), Target::Success, now);
            }
            _ => {}
        }
    }

    fn prompt_key(&mut self, k: KeyEvent, now: Instant) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let edit = match (k.code, ctrl) {
            (KeyCode::Enter, _) => {
                self.prompt_open = false;
                let line = self.prompt.submit();
                if let Some(p) = &self.history_path {
                    let _ = prompt::save_history(p, &self.prompt.history);
                }
                let r = self.run_line(&line, now);
                return self.report(r, now);
            }
            (KeyCode::Esc, _) | (KeyCode::Char('c'), true) => {
                self.prompt_open = false;
                return;
            }
            (KeyCode::Char('u'), true) => Edit::Clear,
            (KeyCode::Up, _) | (KeyCode::Char('p'), true) => Edit::Prev,
            (KeyCode::Down, _) | (KeyCode::Char('n'), true) => Edit::Next,
            (KeyCode::Left, _) | (KeyCode::Char('b'), true) => Edit::Left,
            (KeyCode::Right, _) | (KeyCode::Char('f'), true) => Edit::Right,
            (KeyCode::Home, _) | (KeyCode::Char('a'), true) => Edit::Home,
            (KeyCode::End, _) | (KeyCode::Char('e'), true) => Edit::End,
            (KeyCode::Delete, _) | (KeyCode::Char('d'), true) => Edit::Delete,
            (KeyCode::Backspace, _) | (KeyCode::Char('h'), true) => Edit::Backspace,
            (KeyCode::Char(c), false) => Edit::Insert(c),
            _ => return,
        };
        self.prompt.edit(edit);
    }

    /// The digits shown for the current mode, e.g. `09:41:07` or `09:41`.
    pub fn time_text(&self, now: Instant, zoned: &jiff::Zoned) -> String {
        let [h, m, s] = match self.cfg.mode {
            Mode::Clock => clock::time_of_day(zoned, self.cfg.on(Toggle::Hour24)),
            Mode::Timer => clock::hms(self.timer_left(now)),
            Mode::Stopwatch => clock::hms(self.stopwatch_value(now)),
        };
        if self.cfg.on(Toggle::Seconds) {
            format!("{h:02}:{m:02}:{s:02}")
        } else {
            format!("{h:02}:{m:02}")
        }
    }

    pub fn render(&self, now: Instant, zoned: &jiff::Zoned, w: u16, h: u16) -> Grid {
        let time = self.time_text(now, zoned);
        let date = clock::format_date(&self.cfg.date_fmt, zoned);
        let frame = match &self.cava {
            CavaState::Running(c) => c.frame(),
            _ => Vec::new(),
        };
        let panel = match (&self.cava, self.cfg.on(Toggle::Cava)) {
            (_, false) | (CavaState::Off, _) => Panel::Off,
            (CavaState::Running(_), _) => Panel::Bars(&frame),
            (CavaState::Failed(msg), _) => Panel::Message(msg),
        };
        let input = self.prompt.text();
        let live = self.status.as_ref().filter(|(_, _, at)| {
            now.saturating_duration_since(*at) < Duration::from_millis(self.cfg.rate_status)
        });
        let bottom = match live {
            _ if self.prompt_open => Bottom::Prompt(&input, self.prompt.cursor()),
            Some((msg, t, _)) => Bottom::Status(msg, *t),
            None => Bottom::Idle,
        };
        render::render(
            &Scene {
                cfg: &self.cfg,
                time: &time,
                date: &date,
                bottom,
                panel,
            },
            w,
            h,
        )
    }
}

pub fn help(term: &str) -> Result<String, String> {
    let term = term.trim().to_lowercase();
    let mut hits: Vec<&str> = crate::cli::HELP
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.to_lowercase().contains(&term))
        .collect();
    hits.sort_by_key(|l| !l.starts_with(&term));
    match hits.as_slice() {
        [] => Err(format!("no help for '{term}'")),
        _ if term.is_empty() => Ok(
            "keys: w e r modes · W E R T Y views · a s d f g toggles · z cava · : prompt · q quit"
                .into(),
        ),
        h => Ok(h
            .iter()
            .take(3)
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("  |  ")),
    }
}
