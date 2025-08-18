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
}

impl Default for App {
    fn default() -> Self {
        Self {
            bar_values: Arc::new(Mutex::new(vec![0.0; 100])),
            exit: false,
            recording: true,
            shutdown_tx: None,
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

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
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

        let center_x = inner.x + inner.width / 2;
        let center_y = inner.y + inner.height / 2;
        let max_bar_height = (inner.height / 2).saturating_sub(3);

        let left = center_x - (bar_values.len() as u16 / 2);

        for (i, &value) in bar_values.iter().enumerate() {
            // TODO: I do like having a gap of one between each bar, but need to iron out the math
            let bar_x = 2 * (i as u16);
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

            // TODO I don't love the look of the middle yet.
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
