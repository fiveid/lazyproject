#![forbid(unsafe_code)]

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use lazyproject::app::AppState;
use lazyproject::scanner::scan_workspace;
use lazyproject::ui;

#[derive(Parser, Debug)]
#[command(
    name = "lazyproject",
    version = env!("LAZYPROJECT_VERSION"),
    about = "Read-only TUI for inspecting git status, remote sync, and toolchains across your projects"
)]
struct Cli {
    /// Root directory containing projects (defaults to current working directory)
    path: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let root = match cli.path {
        Some(p) => p,
        None => std::env::current_dir()?,
    };

    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory.", root.display());
        std::process::exit(1);
    }

    let scan = scan_workspace(&root)?;
    let mut app = AppState::new(scan);

    // Ensure terminal is restored even on panic
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }

    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
) -> io::Result<()> {
    while !app.should_quit {
        app.poll_background();
        terminal.draw(|frame| ui::render(frame, app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key);
        }
    }
    Ok(())
}
