use crate::color::Color;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Clock,
    Timer,
    Stopwatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Digital,
    Ascii,
    Binary,
    Date,
    Icon,
}

pub const MODES: [(Mode, &str); 3] = [(Mode::Clock, "clock"), (Mode::Timer, "timer"), (Mode::Stopwatch, "stopwatch")];
pub const VIEWS: [(View, &str); 5] = [
    (View::Digital, "digital"),
    (View::Ascii, "ascii"),
    (View::Binary, "binary"),
    (View::Date, "date"),
    (View::Icon, "icon"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Style {
    pub color: Option<Color>,
    pub reverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    ActiveFg,
    InactiveFg,
    ColonFg,
    ActiveBg,
    InactiveBg,
    ColonBg,
    Background,
    Date,
    Text,
    Prompt,
    Success,
    Error,
    Cava,
    Banner,
}

pub const TARGETS: [(Target, &str); 14] = [
    (Target::ActiveFg, "active-fg"),
    (Target::InactiveFg, "inactive-fg"),
    (Target::ColonFg, "colon-fg"),
    (Target::ActiveBg, "active-bg"),
    (Target::InactiveBg, "inactive-bg"),
    (Target::ColonBg, "colon-bg"),
    (Target::Background, "background"),
    (Target::Date, "date"),
    (Target::Text, "text"),
    (Target::Prompt, "prompt"),
    (Target::Success, "success"),
    (Target::Error, "error"),
    (Target::Cava, "cava"),
    (Target::Banner, "banner"),
];

pub fn name_of<T: PartialEq + Copy>(table: &[(T, &'static str)], v: T) -> &'static str {
    table.iter().find(|(t, _)| *t == v).map(|(_, n)| *n).unwrap_or("")
}

pub fn lookup<T: Copy>(table: &[(T, &str)], name: &str) -> Option<T> {
    table.iter().find(|(_, n)| *n == name).map(|(t, _)| *t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Hour24,
    Seconds,
    Date,
    AutoSize,
    AutoRatio,
    Cava,
    Banner,
}

pub const TOGGLES: [(Toggle, &str); 7] = [
    (Toggle::Hour24, "hour-24"),
    (Toggle::Seconds, "seconds"),
    (Toggle::Date, "date"),
    (Toggle::AutoSize, "auto-size"),
    (Toggle::AutoRatio, "auto-ratio"),
    (Toggle::Cava, "cava"),
    (Toggle::Banner, "banner"),
];

pub const DEFAULT_FILL: &str = "█";

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mode: Mode,
    pub view: View,
    pub locale: String,
    pub timezone: String,
    pub date_fmt: String,
    pub block: (u16, u16),
    pub padding: (u16, u16),
    pub margin: (u16, u16),
    pub ratio: (u16, u16),
    pub date_padding: u16,
    pub toggles: [bool; 7],
    pub fill_active: String,
    pub fill_inactive: String,
    pub fill_colon: String,
    pub fill_cava: String,
    pub rate_input: u64,
    pub rate_refresh: u64,
    pub rate_status: u64,
    pub styles: [Style; 14],
    pub timer: Duration,
    pub timer_exec: String,
    pub stopwatch: Duration,
    pub cava_bars: u16,
    pub cava_height: u16,
}

impl Default for Config {
    fn default() -> Self {
        let mut styles = [Style::default(); 14];
        let set = |styles: &mut [Style; 14], t: Target, c: Color| styles[t as usize].color = Some(c);
        set(&mut styles, Target::ActiveFg, Color::Ansi(4));
        set(&mut styles, Target::InactiveFg, Color::Ansi(8));
        set(&mut styles, Target::ColonFg, Color::Ansi(4));
        set(&mut styles, Target::Date, Color::Ansi(15));
        set(&mut styles, Target::Prompt, Color::Ansi(12));
        set(&mut styles, Target::Success, Color::Ansi(10));
        set(&mut styles, Target::Error, Color::Ansi(9));
        set(&mut styles, Target::Cava, Color::Ansi(6));
        set(&mut styles, Target::Banner, Color::Ansi(8));
        Config {
            mode: Mode::Clock,
            view: View::Digital,
            locale: String::new(),
            timezone: String::new(),
            date_fmt: "%a %b %d %Y".into(),
            block: (2, 1),
            padding: (1, 0),
            margin: (1, 1),
            ratio: (2, 1),
            date_padding: 1,
            toggles: [true, true, true, true, false, false, true],
            fill_active: DEFAULT_FILL.into(),
            fill_inactive: String::new(),
            fill_colon: DEFAULT_FILL.into(),
            fill_cava: DEFAULT_FILL.into(),
            rate_input: 50,
            rate_refresh: 250,
            rate_status: 2000,
            styles,
            timer: Duration::from_secs(10 * 60),
            timer_exec: String::new(),
            stopwatch: Duration::ZERO,
            cava_bars: 32,
            cava_height: 8,
        }
    }
}

impl Config {
    pub fn on(&self, t: Toggle) -> bool {
        self.toggles[t as usize]
    }

    pub fn style(&self, t: Target) -> Style {
        self.styles[t as usize]
    }

    /// The command file that reproduces this config when sourced over the defaults.
    pub fn to_commands(&self) -> String {
        let q = |s: &str| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"));
        let mut out = vec![
            "# peacrust config · created by Hiwarkar".to_string(),
            format!("mode {}", name_of(&MODES, self.mode)),
            format!("view {}", name_of(&VIEWS, self.view)),
            format!("locale {}", q(&self.locale)),
            format!("timezone {}", q(&self.timezone)),
            format!("date {}", q(&self.date_fmt)),
            format!("block {} {}", self.block.0, self.block.1),
            format!("padding {} {}", self.padding.0, self.padding.1),
            format!("margin {} {}", self.margin.0, self.margin.1),
            format!("ratio {} {}", self.ratio.0, self.ratio.1),
            format!("date-padding {}", self.date_padding),
        ];
        for (t, name) in TOGGLES {
            out.push(format!("set {name} {}", if self.on(t) { "on" } else { "off" }));
        }
        out.push(format!("fill-active {}", q(&self.fill_active)));
        out.push(format!("fill-inactive {}", q(&self.fill_inactive)));
        out.push(format!("fill-colon {}", q(&self.fill_colon)));
        out.push(format!("fill-cava {}", q(&self.fill_cava)));
        out.push(format!("rate-input {}", self.rate_input));
        out.push(format!("rate-refresh {}", self.rate_refresh));
        out.push(format!("rate-status {}", self.rate_status));
        for (t, name) in TARGETS {
            let s = self.style(t);
            out.push(format!("style {name} clear"));
            if let Some(c) = s.color {
                out.push(format!("style {name} {c}"));
            }
            if s.reverse {
                out.push(format!("style {name} reverse"));
            }
        }
        out.push(format!("timer {}", crate::command::format_duration(self.timer)));
        out.push(format!("timer-exec {}", q(&self.timer_exec)));
        out.push(format!("stopwatch {}", crate::command::format_duration(self.stopwatch)));
        out.push(format!("cava-bars {}", self.cava_bars));
        out.push(format!("cava-height {}", self.cava_height));
        out.join("\n") + "\n"
    }
}
