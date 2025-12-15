//! Event handling for the TUI
//!
//! Handles keyboard and mouse events via crossterm

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Application events
#[derive(Debug, Clone)]
pub enum Event {
    /// Periodic tick for animations and updates
    Tick,
    /// Keyboard event
    Key(KeyEvent),
    /// Mouse event
    Mouse(MouseEvent),
    /// Terminal resize
    Resize(u16, u16),
}

/// Event handler that polls for terminal events
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
}

/// Minimum interval between drag events (16ms = ~60fps)
const DRAG_THROTTLE_MS: u64 = 16;

impl EventHandler {
    /// Create a new event handler with the given tick rate in milliseconds
    pub fn new(tick_rate_ms: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate_ms);
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut last_drag_time = Instant::now();
            let drag_throttle = Duration::from_millis(DRAG_THROTTLE_MS);

            loop {
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if tx.send(Event::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            // Throttle drag events to prevent lag
                            if matches!(mouse.kind, MouseEventKind::Drag(_)) {
                                let now = Instant::now();
                                if now.duration_since(last_drag_time) < drag_throttle {
                                    // Skip this drag event - too soon after the last one
                                    continue;
                                }
                                last_drag_time = now;
                            }
                            if tx.send(Event::Mouse(mouse)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            if tx.send(Event::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                } else {
                    // Send tick event
                    if tx.send(Event::Tick).is_err() {
                        break;
                    }
                }
            }
        });

        Self { rx }
    }

    /// Wait for the next event
    pub async fn next(&mut self) -> anyhow::Result<Event> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }
}
