use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use peacrust::app::{App, CavaState};
use peacrust::color::Color;
use peacrust::config::{Mode, Target, Toggle, View};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn zoned(s: &str) -> jiff::Zoned {
    s.parse().unwrap()
}

fn noon() -> jiff::Zoned {
    zoned("2026-09-29T12:34:56[UTC]")
}

fn press(app: &mut App, keys: &str, now: Instant) {
    for c in keys.chars() {
        app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), now);
    }
}

fn code(app: &mut App, k: KeyCode, now: Instant) {
    app.key(KeyEvent::new(k, KeyModifiers::NONE), now);
}

fn command(app: &mut App, line: &str, now: Instant) {
    press(app, ":", now);
    press(app, line, now);
    code(app, KeyCode::Enter, now);
}

fn screen(app: &App, now: Instant) -> String {
    app.render(now, &noon(), 60, 20).text()
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("peacrust-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn keys_switch_modes_and_views() {
    let t = Instant::now();
    let mut app = App::new(None);
    assert_eq!(app.time_text(t, &noon()), "12:34:56");
    press(&mut app, "e", t);
    assert_eq!(
        (app.cfg.mode, app.time_text(t, &noon())),
        (Mode::Timer, "00:10:00".into())
    );
    press(&mut app, "r", t);
    assert_eq!(
        (app.cfg.mode, app.time_text(t, &noon())),
        (Mode::Stopwatch, "00:00:00".into())
    );
    for (k, v) in [
        ('W', View::Date),
        ('E', View::Ascii),
        ('T', View::Binary),
        ('Y', View::Icon),
        ('R', View::Digital),
    ] {
        press(&mut app, &k.to_string(), t);
        assert_eq!(app.cfg.view, v);
    }
    press(&mut app, "wY", t);
    assert!(screen(&app, t).contains("12:34:56"));
}

#[test]
fn toggles_change_the_readout() {
    let t = Instant::now();
    let mut app = App::new(None);
    let midnight = zoned("2026-09-29T00:30:05[UTC]");
    assert_eq!(app.time_text(t, &midnight), "00:30:05");
    press(&mut app, "a", t);
    assert_eq!(app.time_text(t, &midnight), "12:30:05");
    press(&mut app, "s", t);
    assert_eq!(app.time_text(t, &midnight), "12:30");
    press(&mut app, "dfg", t);
    assert_eq!(
        [Toggle::Date, Toggle::AutoSize, Toggle::AutoRatio].map(|x| app.cfg.on(x)),
        [false, false, true]
    );
}

#[test]
fn timer_counts_down_stops_and_fires_once() {
    let dir = tmp("timer");
    let log = dir.join("fired");
    let t = Instant::now();
    let mut app = App::new(None);
    command(&mut app, "timer 3s", t);
    command(
        &mut app,
        &format!("timer-exec 'echo done >> {}'", log.display()),
        t,
    );
    press(&mut app, "e ", t);
    assert_eq!(
        app.time_text(t + Duration::from_secs(1), &noon()),
        "00:00:02"
    );
    for s in 3..6 {
        app.tick(t + Duration::from_secs(s));
    }
    assert!(!app.timer.running());
    assert_eq!(
        app.time_text(t + Duration::from_secs(9), &noon()),
        "00:00:00"
    );
    assert!(screen(&app, t + Duration::from_secs(4)).contains("timer complete"));
    press(&mut app, " ", t + Duration::from_secs(6));
    app.tick(t + Duration::from_secs(7));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !log.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "done\n");

    code(&mut app, KeyCode::Backspace, t);
    assert_eq!(app.time_text(t, &noon()), "00:00:03");
}

#[test]
fn stopwatch_runs_pauses_and_clears() {
    let t = Instant::now();
    let mut app = App::new(None);
    press(&mut app, "r ", t);
    assert_eq!(
        app.time_text(t + Duration::from_secs(65), &noon()),
        "00:01:05"
    );
    press(&mut app, " ", t + Duration::from_secs(65));
    assert_eq!(
        app.time_text(t + Duration::from_secs(500), &noon()),
        "00:01:05"
    );
    code(&mut app, KeyCode::Backspace, t);
    assert_eq!(app.time_text(t, &noon()), "00:00:00");
    command(&mut app, "stopwatch 30s", t);
    assert_eq!(app.time_text(t, &noon()), "00:00:30");
}

#[test]
fn space_does_nothing_in_clock_mode() {
    let t = Instant::now();
    let mut app = App::new(None);
    press(&mut app, " ", t);
    code(&mut app, KeyCode::Backspace, t);
    assert_eq!(
        (app.timer.running(), app.stopwatch.running()),
        (false, false)
    );
    press(&mut app, "e ", t);
    assert!(app.timer.running());
}

#[test]
fn prompt_runs_commands_and_keeps_history() {
    let dir = tmp("history");
    let hist = dir.join("history");
    let t = Instant::now();
    let mut app = App::new(Some(hist.clone()));
    command(&mut app, "view ascii", t);
    assert_eq!(app.cfg.view, View::Ascii);
    assert_eq!(std::fs::read_to_string(&hist).unwrap(), "view ascii\n");

    command(&mut app, "bogus", t);
    assert_eq!(
        app.render(t, &noon(), 60, 20).row(19).trim_end(),
        "error: unknown command 'bogus'"
    );

    let mut again = App::new(Some(hist));
    press(&mut again, ":v", t);
    code(&mut again, KeyCode::Up, t);
    assert_eq!(
        again.render(t, &noon(), 60, 20).row(19).trim_end(),
        ":view ascii"
    );
    code(&mut again, KeyCode::Esc, t);
    assert!(!again.prompt_open);
}

#[test]
fn status_expires_and_banner_returns() {
    let t = Instant::now();
    let mut app = App::new(None);
    let banner = "peacrust · created by Hiwarkar";
    assert!(screen(&app, t).lines().last().unwrap().contains(banner));
    command(&mut app, "help timer", t);
    let bottom = screen(&app, t).lines().last().unwrap().to_string();
    assert!(bottom.contains("timer 10m:0s"), "{bottom}");
    assert!(
        screen(&app, t + Duration::from_secs(3))
            .lines()
            .last()
            .unwrap()
            .contains(banner)
    );
    command(&mut app, "set banner off", t);
    assert!(!screen(&app, t + Duration::from_secs(3)).contains(banner));
}

#[test]
fn interactive_adjustment() {
    let t = Instant::now();
    let mut app = App::new(None);
    press(&mut app, "pllk", t);
    assert_eq!(
        (app.cfg.block, app.cfg.on(Toggle::AutoSize)),
        ((4, 2), false)
    );
    press(&mut app, "ihj", t);
    assert_eq!(app.cfg.margin, (0, 0));
    command(&mut app, "style active-fg #ff0000", t);
    press(&mut app, "x", t);
    for _ in 0..12 {
        press(&mut app, "l", t);
    }
    assert_eq!(
        app.cfg.style(Target::ActiveFg).color,
        Some(Color::Rgb(0, 255, 0))
    );
    press(&mut app, "'", t);
    assert_eq!(
        app.cfg.style(Target::ActiveFg).color,
        Some(Color::Rgb(26, 255, 26))
    );
    press(&mut app, "/", t);
    assert_eq!(app.cfg.style(Target::ActiveFg).color, None);
}

#[test]
fn mkconfig_and_source_round_trip() {
    let dir = tmp("mkconfig");
    let file = dir.join("saved");
    let t = Instant::now();
    let mut app = App::new(None);
    command(&mut app, "view binary", t);
    command(&mut app, "style background #102030", t);
    command(&mut app, &format!("mkconfig {}", file.display()), t);
    assert!(file.exists());
    command(&mut app, &format!("mkconfig {}", file.display()), t);
    assert!(
        app.render(t, &noon(), 300, 20)
            .text()
            .contains("exists, use mkconfig! to overwrite")
    );
    command(&mut app, &format!("mkconfig! {}", file.display()), t);

    let mut fresh = App::new(None);
    fresh.source(&file, t).unwrap();
    assert_eq!(fresh.cfg, app.cfg);
}

#[test]
fn source_reports_bad_lines_but_applies_good_ones() {
    let dir = tmp("source");
    let file = dir.join("config");
    std::fs::write(&file, "# comment\nview icon\nnope\nset seconds off\n").unwrap();
    let mut app = App::new(None);
    let err = app.source(&file, Instant::now()).unwrap_err();
    assert!(err.ends_with("line 3: unknown command 'nope'"), "{err}");
    assert_eq!(
        (app.cfg.view, app.cfg.on(Toggle::Seconds)),
        (View::Icon, false)
    );
}

#[test]
fn presets_all_load_cleanly() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let mut app = App::new(None);
        app.cava_bin = "peacrust-no-such-cava".into();
        app.source(&path, Instant::now())
            .unwrap_or_else(|e| panic!("{e}"));
        count += 1;
    }
    assert!(count >= 5, "found {count} presets");
}

#[test]
fn missing_cava_shows_install_hint() {
    let t = Instant::now();
    let mut app = App::new(None);
    app.cava_bin = "peacrust-no-such-cava".into();
    press(&mut app, "z", t);
    assert!(screen(&app, t).contains("cava not found · run peacrust --install-cava"));
    press(&mut app, "z", t);
    assert!(!screen(&app, t).contains("cava not found"));
}

#[test]
fn cava_bars_flow_from_child_to_screen() {
    let dir = tmp("cava");
    let fake = dir.join("cava");
    std::fs::write(
        &fake,
        "#!/bin/sh\nwhile true; do echo '1000;1000;1000;1000;'; sleep 0.02; done\n",
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&fake)
        .status()
        .unwrap();
    let t = Instant::now();
    let mut app = App::new(None);
    app.cava_bin = fake.display().to_string();
    command(&mut app, "cava-bars 4", t);
    press(&mut app, "z", t);
    assert!(app.animating());
    let deadline = Instant::now() + Duration::from_secs(3);
    let full = "█".repeat(14);
    while !screen(&app, t).contains(&full) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let lines: Vec<String> = screen(&app, t).lines().map(String::from).collect();
    assert_eq!(
        lines[18],
        "██████████████ ██████████████ ██████████████ ██████████████"
    );
    let CavaState::Running(c) = &app.cava else {
        panic!("cava not running")
    };
    let conf = c.conf().to_path_buf();
    assert!(conf.exists());
    press(&mut app, "z", t);
    assert!(matches!(app.cava, CavaState::Off));
    assert!(!conf.exists());
}
