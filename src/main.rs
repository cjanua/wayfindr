use std::io::{self, stdout, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
struct App {
    input: String,
    output: Vec<String>,
    exit_flag: bool,
}
impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            output: vec!(String::new()),
            exit_flag: false,
        }
    }
}

fn main() -> io::Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(
        CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    run_app(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        if app.exit_flag {
            return Ok(());
        }

        terminal.draw(|frame| ui(frame, app))?;
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => app.exit_flag = true,
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Enter => {
                            app.output.insert(0, app.input.clone());
                            app.input.clear();
                        }
                        _ => {
                            print!("Key attempted: {:?}\n", key);
                        }
                    }
                }
            }
        } 
    }
}

fn ui(
    frame: &mut Frame,
    app: &App,
) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input area
            Constraint::Length(3), // Empty output box
            Constraint::Min(0),    // History area
        ])
        .split(frame.area());

    let input_text = format!("> {}", app.input);
    let input_block = Paragraph::new(input_text)
        .block(Block::default().borders(Borders::ALL).title("Search"))
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(input_block, main_layout[0]);

    let output_block = Paragraph::new("")
        .block(Block::default().borders(Borders::ALL).title("Output"))
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(output_block, main_layout[1]);

    let mut history_text = app.output[0..std::cmp::min(app.output.len(), 10)].join("\n");
    if app.output.len() > 11 {
        history_text.push_str("\n...\n");
    }

    let history_block = Paragraph::new(history_text.to_string())
        .block(Block::default().borders(Borders::ALL).title("History"))
        .style(Style::default().fg(Color::White));

    frame.render_widget(history_block, main_layout[2]);
}