//! VQS Master Grid Designer - Interactive TUI
//!
//! Interactive terminal application for building LSC2 master grids
//! using the systematic framework from the thesis.
//!
//! ## Usage
//!
//! ```bash
//! # Basic usage with default table
//! vqs-designer
//!
//! # With custom initial depths and min-dz values
//! vqs-designer --depths 1,5,10,20,50,100 --min-dzs 0.5,1,2,5,10
//!
//! # With hgrid for direct vgrid.in generation
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
    about = "Interactive VQS master grid designer using the LSC2 framework"
)]
struct Cli {
    /// Optional hgrid file for live vgrid.in generation
    #[clap(short = 'g', long, value_name = "FILE")]
    hgrid: Option<PathBuf>,

    /// Output directory for generated files
    #[clap(short, long, default_value = ".", value_name = "DIR")]
    output: PathBuf,

    /// Initial depth values (comma-separated, e.g., "1,5,10,20,50,100")
    #[clap(long, value_delimiter = ',', value_name = "DEPTHS")]
    depths: Option<Vec<f64>>,

    /// Initial min-dz values (comma-separated, e.g., "0.5,1,2,5,10")
    #[clap(long, value_delimiter = ',', value_name = "DZS")]
    min_dzs: Option<Vec<f64>>,
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

    // Create app with optional custom initial values
    let mut app = if cli.depths.is_some() || cli.min_dzs.is_some() {
        let depths = cli.depths.unwrap_or_else(|| {
            vec![
                0.5, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
                7500.0, 11000.0,
            ]
        });
        let min_dzs = cli
            .min_dzs
            .unwrap_or_else(|| vec![0.5, 1.0, 3.0, 10.0, 50.0, 100.0]);
        App::with_table(depths, min_dzs, cli.hgrid, cli.output)
    } else {
        App::new(cli.hgrid, cli.output)
    };

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
