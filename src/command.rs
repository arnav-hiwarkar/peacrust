use crate::color::Color;
use crate::config::{Config, MODES, TARGETS, TOGGLES, VIEWS, lookup};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterOp {
    Start,
    Stop,
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Help(String),
    MkConfig { path: String, force: bool },
    Source(String),
    Timer(CounterOp),
    Stopwatch(CounterOp),
}

pub fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut chars = line.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut tok = String::new();
        if c == '\'' || c == '"' {
            chars.next();
            loop {
                match chars.next() {
                    None => return Err("unterminated quote".into()),
                    Some('\\') if chars.peek().is_some_and(|&n| n == c || n == '\\') => tok.push(chars.next().unwrap_or(c)),
                    Some(ch) if ch == c => break,
                    Some(ch) => tok.push(ch),
                }
            }
        } else {
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                tok.push(ch);
                chars.next();
            }
        }
        out.push(tok);
    }
    Ok(out)
}

/// Accepts `10m:0s`, `01h:30m:00s`, `90s`, `5m` or bare seconds.
pub fn parse_duration(s: &str) -> Option<Duration> {
    if let Ok(n) = s.parse::<u64>() {
        return Some(Duration::from_secs(n));
    }
    let mut total = 0u64;
    for part in s.split(':') {
        let (num, unit) = part.split_at(part.len().checked_sub(1)?);
        let n: u64 = num.parse().ok()?;
        total = total.checked_add(n.checked_mul(match unit {
            "h" => 3600,
            "m" => 60,
            "s" => 1,
            _ => return None,
        })?)?;
    }
    Some(Duration::from_secs(total))
}

pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}h:{:02}m:{:02}s", s / 3600, s / 60 % 60, s % 60)
}

fn num<T: std::str::FromStr>(s: Option<&String>) -> Result<T, String> {
    let s = s.ok_or("missing number")?;
    s.parse().map_err(|_| format!("invalid number '{s}'"))
}

fn pair(args: &[String]) -> Result<(u16, u16), String> {
    Ok((num(args.first())?, num(args.get(1))?))
}

fn text(args: &[String]) -> Result<String, String> {
    match args {
        [s] => Ok(s.clone()),
        [] => Err("missing value".into()),
        _ => Err("too many values, quote the text".into()),
    }
}

fn counter_op(args: &[String], dur: &mut Duration) -> Result<CounterOp, String> {
    match args.first().map(String::as_str) {
        Some("start") => Ok(CounterOp::Start),
        Some("stop") => Ok(CounterOp::Stop),
        Some("clear") => Ok(CounterOp::Clear),
        Some(s) => {
            *dur = parse_duration(s).ok_or(format!("invalid duration '{s}'"))?;
            Ok(CounterOp::Clear)
        }
        None => Err("expected start, stop, clear or a duration".into()),
    }
}

/// Run one command line against the config. On error the config is left untouched.
pub fn execute(line: &str, cfg: &mut Config) -> Result<Action, String> {
    let toks = tokenize(line)?;
    let Some((cmd, args)) = toks.split_first() else {
        return Ok(Action::None);
    };
    if cmd.starts_with('#') {
        return Ok(Action::None);
    }
    let mut c = cfg.clone();
    let arg0 = args.first().map(String::as_str).unwrap_or("");
    let action = match cmd.as_str() {
        "quit" | "exit" => Action::Quit,
        "help" => Action::Help(args.join(" ")),
        "mkconfig" | "mkconfig!" => Action::MkConfig { path: text(args)?, force: cmd == "mkconfig!" },
        "source" => Action::Source(text(args)?),
        "mode" => {
            c.mode = lookup(&MODES, arg0).ok_or(format!("unknown mode '{arg0}'"))?;
            Action::None
        }
        "view" => {
            c.view = lookup(&VIEWS, arg0).ok_or(format!("unknown view '{arg0}'"))?;
            Action::None
        }
        "locale" => {
            c.locale = text(args)?;
            Action::None
        }
        "timezone" => {
            let tz = text(args)?;
            if !tz.is_empty() && jiff::tz::TimeZone::get(&tz).is_err() {
                return Err(format!("unknown timezone '{tz}'"));
            }
            c.timezone = tz;
            Action::None
        }
        "date" => {
            c.date_fmt = text(args)?;
            Action::None
        }
        "set" => {
            let t = lookup(&TOGGLES, arg0).ok_or(format!("unknown setting '{arg0}'"))?;
            c.toggles[t as usize] = match args.get(1).map(String::as_str) {
                Some("on") => true,
                Some("off") => false,
                _ => return Err("expected on or off".into()),
            };
            Action::None
        }
        "style" => {
            let t = lookup(&TARGETS, arg0).ok_or(format!("unknown style target '{arg0}'"))?;
            let v = args.get(1).ok_or("missing color")?;
            let s = &mut c.styles[t as usize];
            match v.as_str() {
                "clear" => *s = Default::default(),
                "reverse" => s.reverse = true,
                _ => s.color = Some(Color::parse(v).ok_or(format!("invalid color '{v}'"))?),
            }
            Action::None
        }
        "timer" => Action::Timer(counter_op(args, &mut c.timer)?),
        "stopwatch" => Action::Stopwatch(counter_op(args, &mut c.stopwatch)?),
        "timer-exec" => {
            c.timer_exec = text(args)?;
            Action::None
        }
        _ => {
            geometry_or_value(cmd, args, &mut c)?;
            Action::None
        }
    };
    *cfg = c;
    Ok(action)
}

fn geometry_or_value(cmd: &str, args: &[String], c: &mut Config) -> Result<(), String> {
    let (field, axis) = match cmd.rsplit_once('-') {
        Some((f, "x")) => (f, Some(0)),
        Some((f, "y")) => (f, Some(1)),
        _ => (cmd, None),
    };
    let target = match field {
        "block" => Some(&mut c.block),
        "padding" => Some(&mut c.padding),
        "margin" => Some(&mut c.margin),
        "ratio" => Some(&mut c.ratio),
        _ => None,
    };
    if let Some(t) = target {
        match axis {
            None => *t = pair(args)?,
            Some(0) => t.0 = num(args.first())?,
            Some(_) => t.1 = num(args.first())?,
        }
        if field == "block" || field == "ratio" {
            *t = (t.0.max(1), t.1.max(1));
        }
        return Ok(());
    }
    let fill = |f: &mut String| -> Result<(), String> {
        *f = text(args)?;
        Ok(())
    };
    match cmd {
        "fill" => {
            let s = text(args)?;
            let s = if s.is_empty() { crate::config::DEFAULT_FILL.to_string() } else { s };
            c.fill_active = s.clone();
            c.fill_inactive = s.clone();
            c.fill_colon = s;
        }
        "fill-active" => fill(&mut c.fill_active)?,
        "fill-inactive" => fill(&mut c.fill_inactive)?,
        "fill-colon" => fill(&mut c.fill_colon)?,
        "fill-cava" => fill(&mut c.fill_cava)?,
        "date-padding" => c.date_padding = num(args.first())?,
        "rate-input" => c.rate_input = num::<u64>(args.first())?.max(1),
        "rate-refresh" => c.rate_refresh = num::<u64>(args.first())?.max(1),
        "rate-status" => c.rate_status = num(args.first())?,
        "cava-bars" => c.cava_bars = num::<u16>(args.first())?.clamp(1, 512),
        "cava-height" => c.cava_height = num::<u16>(args.first())?.max(1),
        _ => return Err(format!("unknown command '{cmd}'")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Mode, Style, Target, Toggle, View};

    fn run(lines: &[&str]) -> Config {
        let mut c = Config::default();
        for l in lines {
            execute(l, &mut c).unwrap_or_else(|e| panic!("{l}: {e}"));
        }
        c
    }

    #[test]
    fn tokenizes_quotes() {
        assert_eq!(tokenize("date '%a %b'").unwrap(), ["date", "%a %b"]);
        assert_eq!(tokenize(r#"timer-exec "say \"hi\"""#).unwrap(), ["timer-exec", "say \"hi\""]);
        assert_eq!(tokenize("fill ''").unwrap(), ["fill", ""]);
        assert!(tokenize("date 'oops").is_err());
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("10m:0s"), Some(Duration::from_secs(600)));
        assert_eq!(parse_duration("01h:30m:05s"), Some(Duration::from_secs(5405)));
        assert_eq!(parse_duration("90s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_duration("300"), Some(Duration::from_secs(300)));
        for bad in ["", "5x", "m", "1h:", "-5s"] {
            assert_eq!(parse_duration(bad), None, "{bad}");
        }
        assert_eq!(format_duration(Duration::from_secs(5405)), "01h:30m:05s");
    }

    #[test]
    fn mode_view_and_toggles() {
        let c = run(&["mode timer", "view binary", "set seconds off", "set cava on"]);
        assert_eq!(c.mode, Mode::Timer);
        assert_eq!(c.view, View::Binary);
        assert!(!c.on(Toggle::Seconds));
        assert!(c.on(Toggle::Cava));
    }

    #[test]
    fn geometry() {
        let c = run(&["block 4 2", "block-y 3", "padding-x 5", "margin 0 2", "ratio 0 0", "date-padding 4"]);
        assert_eq!((c.block, c.padding.0, c.margin, c.ratio, c.date_padding), ((4, 3), 5, (0, 2), (1, 1), 4));
    }

    #[test]
    fn fills_and_rates() {
        let c = run(&["fill '#'", "fill-colon ':'", "rate-refresh 0", "cava-bars 9999"]);
        assert_eq!((c.fill_active.as_str(), c.fill_inactive.as_str(), c.fill_colon.as_str()), ("#", "#", ":"));
        assert_eq!((c.rate_refresh, c.cava_bars), (1, 512));
        assert_eq!(run(&["fill ''"]).fill_active, "█");
    }

    #[test]
    fn styles() {
        let c = run(&["style active-fg #7c3aed", "style date reverse", "style error clear"]);
        assert_eq!(c.style(Target::ActiveFg).color, Some(Color::Rgb(0x7c, 0x3a, 0xed)));
        assert!(c.style(Target::Date).reverse);
        assert_eq!(c.style(Target::Error), Style::default());
    }

    #[test]
    fn counters_and_actions() {
        let mut c = Config::default();
        assert_eq!(execute("timer 5m", &mut c), Ok(Action::Timer(CounterOp::Clear)));
        assert_eq!(c.timer, Duration::from_secs(300));
        assert_eq!(execute("stopwatch start", &mut c), Ok(Action::Stopwatch(CounterOp::Start)));
        assert_eq!(execute("exit", &mut c), Ok(Action::Quit));
        assert_eq!(execute("help timer", &mut c), Ok(Action::Help("timer".into())));
        assert_eq!(execute("mkconfig! /tmp/x", &mut c), Ok(Action::MkConfig { path: "/tmp/x".into(), force: true }));
        assert_eq!(execute("# comment", &mut c), Ok(Action::None));
        assert_eq!(execute("   ", &mut c), Ok(Action::None));
    }

    #[test]
    fn errors_leave_config_unchanged() {
        let mut c = Config::default();
        for bad in [
            "mode nap",
            "view 3d",
            "block 4",
            "block x 2",
            "set seconds maybe",
            "set nope on",
            "style active-fg purple",
            "style nope red",
            "timer soon",
            "timezone 'Mars/Olympus'",
            "date a b",
            "frobnicate",
        ] {
            assert!(execute(bad, &mut c).is_err(), "{bad}");
        }
        assert_eq!(c, Config::default());
    }

    #[test]
    fn timezone_accepts_real_zone() {
        assert_eq!(run(&["timezone 'America/Vancouver'"]).timezone, "America/Vancouver");
    }

    #[test]
    fn mkconfig_round_trips() {
        let c = run(&[
            "mode stopwatch",
            "view ascii",
            "date '%H it\\'s %p'",
            "block 3 2",
            "set hour-24 off",
            "set banner off",
            "fill-active '▓'",
            "style active-fg #ff0080",
            "style active-fg reverse",
            "style date clear",
            "timer 1h:2m:3s",
            "timer-exec 'notify-send \"done\"'",
            "stopwatch 30s",
            "cava-height 5",
        ]);
        let mut back = Config::default();
        for line in c.to_commands().lines() {
            execute(line, &mut back).unwrap_or_else(|e| panic!("{line}: {e}"));
        }
        assert_eq!(back, c);
    }
}
