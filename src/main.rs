use ratatui::{backend::CrosstermBackend, crossterm, layout::{Constraint, Layout}, widgets::Block, Terminal};
use tui_textarea::TextArea;

use std::io;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::crossterm::{event, execute};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode};

struct Temp<'a>{
    text_area: TextArea<'a>,
}
impl <'a> Temp <'a> {
    fn new()->Self{
        Temp{
            text_area : TextArea::default(),
        }
    }
}
fn main() -> Result<(), io::Error> {
    // Initialize terminal
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut text_area = Temp::new();
    // Create a new TextArea with initial content
    text_area.text_area = TextArea::new(vec!["Hello".to_string()]);

    // Main event loop
    loop {
        // Render the TextArea
        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());
            text_area.text_area.set_block(Block::default().title("Text Editor").borders(ratatui::widgets::Borders::ALL));
            f.render_widget(&text_area.text_area, chunks[0]);
        })?;

        // Handle input events
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Char(c) => {
                    text_area.text_area.insert_char(c); // Insert character into TextArea
                }
                KeyCode::Backspace => {
                    text_area.text_area.delete_char(); // Delete character before the cursor
                }
                KeyCode::Esc => {
                    // Exit the program
                    break;
                }
                _ => {}
            }
        }
    }

    // Clean up terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}
