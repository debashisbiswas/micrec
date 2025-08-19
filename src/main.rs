use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{io, thread, time::Duration};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Widget},
    DefaultTerminal, Frame,
};

#[derive(Debug)]
pub struct App {
    bar_values: Arc<Mutex<Vec<f32>>>,
    exit: bool,
    recording: bool,
    shutdown_tx: Option<Sender<()>>,
    last_terminal_width: u16,
}

impl Default for App {
    fn default() -> Self {
        Self {
            bar_values: Arc::new(Mutex::new(vec![0.0; 50])), // Start with fewer bars
            exit: false,
            recording: true,
            shutdown_tx: None,
            last_terminal_width: 0,
        }
    }
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let (audio_tx, audio_rx) = channel::<Arc<[f32]>>();
        let (shutdown_tx, shutdown_rx) = channel::<()>();

        self.shutdown_tx = Some(shutdown_tx);

        let audio_thread = thread::spawn(move || {
            record_audio(audio_tx, shutdown_rx);
        });

        while !self.exit {
            while let Ok(samples) = audio_rx.try_recv() {
                self.process_audio_samples(&samples);
            }

            terminal.draw(|frame| self.draw(frame))?;

            if crossterm::event::poll(Duration::from_millis(16))? {
                self.handle_events()?;
            }
        }

        if let Some(tx) = &self.shutdown_tx {
            tx.send(()).ok();
        }
        audio_thread.join().ok();

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        // Check if terminal width changed and update bar count
        let current_width = frame.area().width;
        if current_width != self.last_terminal_width {
            self.update_bar_count(current_width);
            self.last_terminal_width = current_width;
        }
        frame.render_widget(&*self, frame.area());
    }

    fn update_bar_count(&mut self, terminal_width: u16) {
        // Calculate optimal bar count based on terminal width
        // Account for border and spacing: 2 chars per bar (bar + gap), minus some padding
        let usable_width = terminal_width.saturating_sub(4); // Account for borders
        let optimal_bar_count = (usable_width / 2).max(10) as usize; // Minimum 10 bars

        if let Ok(mut bars) = self.bar_values.lock() {
            bars.resize(optimal_bar_count, 0.0);
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char(' ') if self.recording => self.stop_recording(),
            KeyCode::Char('q') => self.exit(),
            _ => {}
        }
    }

    fn stop_recording(&mut self) {
        if let Some(tx) = &self.shutdown_tx {
            tx.send(()).ok();
        }
        self.recording = false;
    }

    fn process_audio_samples(&mut self, samples: &[f32]) {
        if let Ok(mut bars) = self.bar_values.lock() {
            let chunk_size = samples.len() / bars.len();
            if chunk_size == 0 {
                return;
            }

            let num_bars = bars.len();
            for (i, bar_value) in bars.iter_mut().enumerate() {
                let start = i * chunk_size;
                let end = if i == num_bars - 1 {
                    samples.len()
                } else {
                    (i + 1) * chunk_size
                };

                let chunk = &samples[start..end];
                let rms = (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();

                *bar_value = (rms * 10.0).min(1.0);
            }
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            Event::Resize(_, _) => {
                // Terminal resize will be handled in the next draw call
            }
            _ => {}
        };
        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let instructions = Line::from(vec![
            " Stop ".into(),
            "<Space>".blue().bold(),
            " Quit ".into(),
            "<q> ".blue().bold(),
        ]);

        let status = if self.recording {
            " Recording...".red().bold()
        } else {
            " Processing...".green().bold()
        };

        let block = Block::new()
            .title_bottom(Line::from(status).left_aligned())
            .title_bottom(instructions.right_aligned());

        let inner = block.inner(area);
        block.render(area, buf);

        let bar_values = self.bar_values.lock().unwrap();

        let center_y = inner.y + inner.height / 2;
        let max_bar_height = (inner.height / 2).saturating_sub(3);

        let available_width = inner.width;
        let bar_spacing = 2; // 1 char for bar + 1 char gap
        let num_bars = bar_values.len() as u16;

        if num_bars == 0 || available_width < bar_spacing {
            return; // No bars to render or terminal too small
        }

        // Calculate starting position to center all bars
        // Note: we don't need the gap after the last bar, so subtract 1 from total width
        let total_width = (num_bars * bar_spacing).saturating_sub(1);
        let start_x = inner.x + (available_width.saturating_sub(total_width)) / 2;

        for (i, &value) in bar_values.iter().enumerate() {
            let bar_x = start_x + (i as u16 * bar_spacing);

            // Ensure bar is within bounds
            if bar_x >= inner.x + inner.width {
                break;
            }

            let bar_height = (value * max_bar_height as f32) as u16;

            let brightness = (value * 255.0) as u8;
            let bar_color = ratatui::style::Color::Rgb(brightness, brightness, brightness);

            for j in 0..bar_height {
                if center_y >= inner.y + j + 1 {
                    buf[(bar_x, center_y - j - 1)]
                        .set_char('█')
                        .set_fg(bar_color);
                }
                if center_y + j + 1 < inner.y + inner.height {
                    buf[(bar_x, center_y + j + 1)]
                        .set_char('█')
                        .set_fg(bar_color);
                }
            }

            buf[(bar_x, center_y)]
                .set_char('█')
                .set_fg(ratatui::style::Color::Rgb(50, 50, 50));
        }
    }
}

fn record_audio(ui_tx: Sender<Arc<[f32]>>, shutdown_rx: Receiver<()>) {
    let host = cpal::default_host();
    let device = host.default_input_device().unwrap();
    let config = device.default_input_config().unwrap();

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if data.is_empty() {
                    return;
                }

                let arc: Arc<[f32]> = Arc::from(data);
                ui_tx.send(arc).ok();
            },
            |err| eprintln!("Audio error: {}", err),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    while shutdown_rx.try_recv().is_err() {
        thread::sleep(Duration::from_millis(10));
    }

    drop(stream);
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::default().run(&mut terminal);
    ratatui::restore();
    result
}
