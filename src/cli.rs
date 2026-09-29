use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const HELP: &str = "\
Usage: peacrust [options] [CONFIG]

Options
  -h, --help              print this help
  -v, --version           print the version
      --license           print the license
  -u, --config PATH       config file to run at startup, NONE to skip
      --config-dir DIR    config directory, NONE to disable config and history
      --colour on|off|auto  colour in help and version output
      --install-cava      install cava with the system package manager

Keys
  q Q ctrl+c              quit
  :                       open the command prompt
  esc                     clear status and selection
  space backspace         start-stop / clear the timer or stopwatch
  w e r                   clock / timer / stopwatch mode
  W E R T Y               date / ascii / digital / binary / icon view
  a s d f g               toggle 24h / seconds / date / auto-size / auto-ratio
  z                       toggle the cava visualizer
  u i o p                 adjust ratio / margin / padding / block with h l j k
  x c v b n m , .         adjust active-fg / inactive-fg / colon-fg / active-bg / inactive-bg / colon-bg / background / date
  h l j k ; ' /           hue or x / saturation or y / luminosity / clear style

Commands
  mode clock|timer|stopwatch
  view digital|ascii|binary|date|icon
  locale 'NAME'  timezone 'Area/City'  date '%a %b %d'
  block X Y  block-x X  block-y Y  padding X Y  margin X Y  ratio X Y  date-padding N
  set hour-24|seconds|date|auto-size|auto-ratio|cava|banner on|off
  fill 'S'  fill-active 'S'  fill-inactive 'S'  fill-colon 'S'  fill-cava 'S'
  rate-input MS  rate-refresh MS  rate-status MS
  style TARGET COLOR|reverse|clear
  targets: active-fg inactive-fg colon-fg active-bg inactive-bg colon-bg background date text prompt success error cava banner
  colors: red green blue yellow magenta cyan black white bright-NAME 0-255 #rgb #rrggbb
  timer 10m:0s|start|stop|clear  timer-exec 'shell command'
  stopwatch 0s|start|stop|clear
  cava-bars N  cava-height N
  help [term]  quit  exit  mkconfig FILE  mkconfig! FILE  source FILE
";

pub const LICENSE: &str = "MIT License. Copyright (c) Hiwarkar. See LICENSE in the repository.";

#[derive(Debug, PartialEq, Eq)]
pub enum Run {
    Help,
    Version,
    License,
    InstallCava,
    App {
        config: Option<PathBuf>,
        config_required: bool,
        dir: Option<PathBuf>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Colour {
    On,
    Off,
    Auto,
}

pub fn default_dir() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let base =
        xdg.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("peacrust"))
}

fn none_or(v: String) -> Option<PathBuf> {
    (v != "NONE").then(|| PathBuf::from(v))
}

pub fn parse(
    args: impl IntoIterator<Item = String>,
    default_dir: Option<PathBuf>,
) -> Result<(Run, Colour), String> {
    let mut args = args.into_iter();
    let mut colour = Colour::Auto;
    let mut run = None;
    let mut config: Option<Option<PathBuf>> = None;
    let mut dir = default_dir;
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "-h" | "--help" => run = run.or(Some(Run::Help)),
            "-v" | "--version" => run = run.or(Some(Run::Version)),
            "--license" => run = run.or(Some(Run::License)),
            "--install-cava" => run = run.or(Some(Run::InstallCava)),
            "-u" | "--config" => config = Some(none_or(value()?)),
            "--config-dir" => dir = none_or(value()?),
            "--colour" | "--color" => {
                colour = match value()?.as_str() {
                    "on" => Colour::On,
                    "off" => Colour::Off,
                    "auto" => Colour::Auto,
                    v => return Err(format!("--colour expects on, off or auto, got '{v}'")),
                }
            }
            s if s.starts_with('-') && s.len() > 1 => return Err(format!("unknown option '{s}'")),
            _ if config.is_some() => return Err(format!("unexpected argument '{a}'")),
            _ => config = Some(Some(PathBuf::from(a))),
        }
    }
    let run = run.unwrap_or_else(|| {
        let config_required = matches!(config, Some(Some(_)));
        let config = match config {
            Some(c) => c,
            None => dir.as_ref().map(|d| d.join("config")),
        };
        Run::App {
            config,
            config_required,
            dir,
        }
    });
    Ok((run, colour))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<(Run, Colour), String> {
        parse(
            args.iter().map(|s| s.to_string()),
            Some(PathBuf::from("/home/u/.config/peacrust")),
        )
    }

    fn app(config: Option<&str>, required: bool, dir: Option<&str>) -> Run {
        Run::App {
            config: config.map(PathBuf::from),
            config_required: required,
            dir: dir.map(PathBuf::from),
        }
    }

    #[test]
    fn defaults_to_config_in_dir() {
        assert_eq!(
            p(&[]),
            Ok((
                app(
                    Some("/home/u/.config/peacrust/config"),
                    false,
                    Some("/home/u/.config/peacrust")
                ),
                Colour::Auto
            ))
        );
    }

    #[test]
    fn info_flags() {
        assert_eq!(p(&["-h"]).unwrap().0, Run::Help);
        assert_eq!(p(&["--version"]).unwrap().0, Run::Version);
        assert_eq!(p(&["--license"]).unwrap().0, Run::License);
        assert_eq!(p(&["--install-cava"]).unwrap().0, Run::InstallCava);
        assert_eq!(
            p(&["--colour", "off", "-v"]),
            Ok((Run::Version, Colour::Off))
        );
    }

    #[test]
    fn config_options() {
        let dir = Some("/home/u/.config/peacrust");
        assert_eq!(
            p(&["presets/neon"]).unwrap().0,
            app(Some("presets/neon"), true, dir)
        );
        assert_eq!(
            p(&["-u", "x.conf"]).unwrap().0,
            app(Some("x.conf"), true, dir)
        );
        assert_eq!(p(&["--config", "NONE"]).unwrap().0, app(None, false, dir));
        assert_eq!(
            p(&["--config-dir", "/d"]).unwrap().0,
            app(Some("/d/config"), false, Some("/d"))
        );
        assert_eq!(
            p(&["--config-dir", "NONE"]).unwrap().0,
            app(None, false, None)
        );
    }

    #[test]
    fn rejects_bad_arguments() {
        assert_eq!(p(&["--nope"]), Err("unknown option '--nope'".into()));
        assert_eq!(p(&["--config"]), Err("--config needs a value".into()));
        assert_eq!(
            p(&["--colour", "loud"]),
            Err("--colour expects on, off or auto, got 'loud'".into())
        );
        assert_eq!(p(&["a", "b"]), Err("unexpected argument 'b'".into()));
    }
}
