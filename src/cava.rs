use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const NOT_FOUND: &str = "cava not found · run peacrust --install-cava";
pub const EXITED: &str = "cava exited";

/// cava config for raw ascii bars on stdout. `input` is the user's own `[input]` section, if any.
pub fn config_text(bars: u16, input: &str) -> String {
    let input = match input {
        "" if cfg!(target_os = "macos") => "[input]\nmethod = portaudio\nsource = auto\n",
        s => s,
    };
    format!(
        "{input}\n[general]\nbars = {bars}\nframerate = 60\n\n[output]\nmethod = raw\nraw_target = /dev/stdout\n\
         data_format = ascii\nascii_max_range = 1000\nbar_delimiter = 59\nframe_delimiter = 10\n\
         channels = mono\nmono_option = average\n"
    )
}

/// The `[input]` section of a cava config, without comments, so the user's audio source is kept.
pub fn input_section(conf: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in conf.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[input]";
        }
        if inside && !line.is_empty() && !line.starts_with(['#', ';']) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.lines().count() > 1 {
        out
    } else {
        String::new()
    }
}

fn user_input_section() -> String {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    let text = dir.and_then(|d| std::fs::read_to_string(d.join("cava/config")).ok());
    input_section(&text.unwrap_or_default())
}

/// One raw ascii frame, e.g. `12;400;1000;`. Malformed frames are dropped.
pub fn parse_frame(line: &str) -> Option<Vec<u16>> {
    let bars: Option<Vec<u16>> = line
        .trim()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().ok())
        .collect();
    bars.filter(|b| !b.is_empty())
}

pub struct Cava {
    child: Child,
    conf: PathBuf,
    frame: Arc<Mutex<Vec<u16>>>,
    dead: Arc<AtomicBool>,
    err: Arc<Mutex<String>>,
    pub bars: u16,
}

impl Cava {
    pub fn spawn(bin: &str, bars: u16) -> Result<Cava, &'static str> {
        static SPAWNS: AtomicUsize = AtomicUsize::new(0);
        let n = SPAWNS.fetch_add(1, Ordering::Relaxed);
        let conf =
            std::env::temp_dir().join(format!("peacrust-cava-{}-{n}.conf", std::process::id()));
        std::fs::write(&conf, config_text(bars, &user_input_section())).map_err(|_| EXITED)?;
        let spawned = Command::new(bin)
            .arg("-p")
            .arg(&conf)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&conf);
                return Err(if e.kind() == std::io::ErrorKind::NotFound {
                    NOT_FOUND
                } else {
                    EXITED
                });
            }
        };
        let frame = Arc::new(Mutex::new(Vec::new()));
        let dead = Arc::new(AtomicBool::new(false));
        let out = child.stdout.take().ok_or(EXITED)?;
        let err = Arc::new(Mutex::new(String::new()));
        let err_reader = child.stderr.take().map(|stderr| {
            let e = err.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let clean: String = line.chars().filter(|c| !c.is_control()).collect();
                    if !clean.trim().is_empty() {
                        *e.lock().unwrap_or_else(|e| e.into_inner()) = clean.trim().to_string();
                    }
                }
            })
        });
        let (f, d) = (frame.clone(), dead.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                if let Some(bars) = parse_frame(&line) {
                    *f.lock().unwrap_or_else(|e| e.into_inner()) = bars;
                }
            }
            if let Some(h) = err_reader {
                let _ = h.join();
            }
            d.store(true, Ordering::Relaxed);
        });
        Ok(Cava {
            child,
            conf,
            frame,
            dead,
            err,
            bars,
        })
    }

    pub fn frame(&self) -> Vec<u16> {
        self.frame.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn conf(&self) -> &Path {
        &self.conf
    }

    /// Why cava stopped, from the last line it printed to stderr.
    pub fn exit_message(&self) -> String {
        match self.err.lock().unwrap_or_else(|e| e.into_inner()).as_str() {
            "" => EXITED.to_string(),
            e => format!("{EXITED}: {e}"),
        }
    }

    pub fn alive(&self) -> bool {
        !self.dead.load(Ordering::Relaxed)
    }
}

impl Drop for Cava {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.conf);
    }
}

pub fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_str()?
        .split(':')
        .map(|d| Path::new(d).join(bin))
        .find(|p| p.is_file())
}

/// The package-manager command that installs cava here, if any is known.
pub fn install_command(has: impl Fn(&str) -> bool, root: bool) -> Option<Vec<&'static str>> {
    let table: [(&str, &[&str], bool); 6] = [
        ("brew", &["brew", "install", "cava"], false),
        ("apt-get", &["apt-get", "install", "-y", "cava"], true),
        ("dnf", &["dnf", "install", "-y", "cava"], true),
        (
            "pacman",
            &["pacman", "-S", "--needed", "--noconfirm", "cava"],
            true,
        ),
        ("zypper", &["zypper", "install", "-y", "cava"], true),
        ("apk", &["apk", "add", "cava"], true),
    ];
    let (_, cmd, needs_root) = table.into_iter().find(|(bin, _, _)| has(bin))?;
    let mut out = if needs_root && !root {
        vec!["sudo"]
    } else {
        vec![]
    };
    out.extend(cmd);
    Some(out)
}

/// `--install-cava`: show the command, ask, run it. Returns the process exit code.
pub fn install_interactive() -> i32 {
    if which("cava").is_some() {
        println!("cava is already installed.");
        return 0;
    }
    // SAFETY: geteuid has no preconditions.
    let root = unsafe { libc::geteuid() } == 0;
    let Some(cmd) = install_command(|b| which(b).is_some(), root) else {
        eprintln!(
            "No supported package manager found. Install cava from https://github.com/karlstav/cava"
        );
        return 1;
    };
    print!("Install cava with `{}`? [y/N] ", cmd.join(" "));
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    if !matches!(answer.trim(), "y" | "Y" | "yes") {
        println!("Skipped.");
        return 1;
    }
    match Command::new(cmd[0]).args(&cmd[1..]).status() {
        Ok(s) if s.success() => 0,
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("failed to run {}: {e}", cmd[0]);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_frames() {
        assert_eq!(parse_frame("0;512;1000;\n"), Some(vec![0, 512, 1000]));
        assert_eq!(parse_frame("7"), Some(vec![7]));
        assert_eq!(parse_frame(""), None);
        assert_eq!(parse_frame("1;x;3;"), None);
        assert_eq!(parse_frame("1;99999;"), None);
    }

    #[test]
    fn config_requests_raw_ascii_bars() {
        let t = config_text(48, "[input]\nmethod = pulse\nsource = auto\n");
        assert!(t.starts_with("[input]\nmethod = pulse\n"));
        assert!(
            t.contains("bars = 48\n")
                && t.contains("method = raw\n")
                && t.contains("data_format = ascii\n")
        );
    }

    #[test]
    fn keeps_only_the_input_section() {
        let conf = "[general]\nbars = 3\n[input]\n; method = alsa\nmethod = pulse\nsource = auto\n\n[output]\nmethod = ncurses\n";
        assert_eq!(
            input_section(conf),
            "[input]\nmethod = pulse\nsource = auto\n"
        );
        assert_eq!(input_section("[input]\n; all commented\n"), "");
        assert_eq!(input_section(""), "");
    }

    #[test]
    fn picks_package_manager() {
        assert_eq!(
            install_command(|b| b == "brew" || b == "apt-get", false),
            Some(vec!["brew", "install", "cava"])
        );
        assert_eq!(
            install_command(|b| b == "apt-get", false),
            Some(vec!["sudo", "apt-get", "install", "-y", "cava"])
        );
        assert_eq!(
            install_command(|b| b == "pacman", true),
            Some(vec!["pacman", "-S", "--needed", "--noconfirm", "cava"])
        );
        assert_eq!(install_command(|_| false, false), None);
    }

    #[test]
    fn which_finds_sh() {
        assert!(which("sh").is_some());
        assert_eq!(which("peacrust-no-such-binary"), None);
    }
}
