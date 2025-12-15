//! VQS Master Grid Designer - Interactive TUI
//!
//! Interactive terminal application for building VQS master grids.
//!
//! ## Usage
//!
//! ```bash
//! # Basic usage
//! vqs-designer
//!
//! # With hgrid for mesh-aware statistics and vgrid.in generation
//! vqs-designer -g hgrid.gr3 -o ./output/
//! ```

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::stdout;
use std::path::PathBuf;

use schismrs_vgrid::tui::{App, Event, EventHandler};

#[derive(Parser, Debug)]
#[command(
    name = "vqs-designer",
    author,
    version,
    about = "Interactive VQS master grid designer"
)]
struct Cli {
    /// Optional hgrid file for mesh-aware statistics and vgrid.in generation
    #[clap(short = 'g', long, value_name = "FILE")]
    hgrid: Option<PathBuf>,

    /// Output directory for generated files
    #[clap(short, long, default_value = ".", value_name = "DIR")]
    output: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(cli.hgrid, cli.output);

    // Create event handler (100ms tick rate)
    let mut event_handler = EventHandler::new(100);

    // Main loop
    loop {
        // Draw the UI
        terminal.draw(|frame| {
            schismrs_vgrid::tui::ui::draw(frame, &mut app);
        })?;

        // Handle events
        match event_handler.next().await? {
            Event::Tick => {
                app.on_tick();
            }
            Event::Key(key) => {
                app.on_key(key);
                if app.should_quit {
                    break;
                }
            }
            Event::Mouse(mouse) => {
                app.on_mouse(mouse);
            }
            Event::Resize(_, _) => {
                // Terminal will redraw automatically
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Print final configuration if path was valid
    if app.path.is_valid() {
        println!("\nFinal configuration:");
        println!("  {}", app.generate_cli_args());
        println!("\nYAML format:");
        println!("{}", app.generate_yaml());
    }

    Ok(())
}
