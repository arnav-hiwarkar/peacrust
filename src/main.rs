use peacrust::app::App;
use peacrust::cli::{self, Colour, Run};
use peacrust::{cava, term};
use std::io::IsTerminal;
use std::time::Instant;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let (run, colour) = match cli::parse(std::env::args().skip(1), cli::default_dir()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("peacrust: {e}\nTry 'peacrust --help'.");
            return 2;
        }
    };
    let colour = match colour {
        Colour::On => true,
        Colour::Off => false,
        Colour::Auto => std::io::stdout().is_terminal(),
    };
    let title = if colour {
        "\x1b[1;34mpeacrust\x1b[0m"
    } else {
        "peacrust"
    };
    let (config, config_required, dir) = match run {
        Run::Help => {
            println!(
                "{title} {} · created by Hiwarkar\n\n{}",
                cli::VERSION,
                cli::HELP
            );
            return 0;
        }
        Run::Version => {
            println!("{title} {}\ncreated by Hiwarkar", cli::VERSION);
            return 0;
        }
        Run::License => {
            println!("{}", cli::LICENSE);
            return 0;
        }
        Run::InstallCava => return cava::install_interactive(),
        Run::App {
            config,
            config_required,
            dir,
        } => (config, config_required, dir),
    };
    if !std::io::stdout().is_terminal() {
        eprintln!("peacrust: stdout is not a terminal");
        return 1;
    }
    let mut app = App::new(dir.map(|d| d.join("history")));
    let now = Instant::now();
    if let Some(path) = config {
        if config_required && !path.exists() {
            eprintln!("peacrust: config file '{}' not found", path.display());
            return 1;
        }
        if path.exists()
            && let Err(e) = app.source(&path, now)
        {
            app.startup_error(e, now);
        }
    }
    let result = term::TermGuard::enter().and_then(|_guard| term::run(&mut app));
    drop(app);
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("peacrust: {e}");
            1
        }
    }
}
