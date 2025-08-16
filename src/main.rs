use std::io;

mod audio;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

#[derive(Debug, Default)]
pub struct App {
    counter: u8,
    exit: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn increment_counter(&mut self) {
        self.counter += 1;
    }

    fn decrement_counter(&mut self) {
        self.counter -= 1;
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left => self.decrement_counter(),
            KeyCode::Right => self.increment_counter(),
            _ => {}
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
            " Cancel ".into(),
            "<C-c>".blue().bold(),
            " Quit ".into(),
            "<q> ".blue().bold(),
        ]);

        let block = Block::bordered()
            .title_bottom(Line::from(" Recording...").left_aligned())
            .title_bottom(instructions.right_aligned())
            .border_set(border::ROUNDED);

        let inner = block.inner(area);
        block.render(area, buf);

        let center_y = inner.y + inner.height / 2;
        let center_x = inner.x + inner.width / 2;

        let max_bar_height = (inner.height / 2).saturating_sub(1);
        let bar_height = (self.counter as u16 * max_bar_height) / 10;

        for i in 0..bar_height {
            if center_y >= inner.y + i + 1 {
                buf[(center_x, center_y - i - 1)].set_char('█');
            }
            if center_y + i + 1 < inner.y + inner.height {
                buf[(center_x, center_y + i + 1)].set_char('█');
            }
        }

        buf[(center_x, center_y)].set_char('─');
        let counter_text = Text::from(vec![Line::from(vec![
            "Value: ".into(),
            self.counter.to_string().yellow(),
        ])]);

        Paragraph::new(counter_text).render(Rect::new(inner.x + 1, inner.y, 20, 1), buf);
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::default().run(&mut terminal);
    ratatui::restore();
    result
}
