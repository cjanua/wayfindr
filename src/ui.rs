use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::app::{App, FocusBlock}; // Adjusted path

// ui function remains largely the same, just ensure paths to App and FocusBlock are correct
pub fn ui(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input area
            Constraint::Min(1),    // Output area (ensure at least 1 line)
            Constraint::Length(5), // History area (fixed height for demo)
        ])
        .split(frame.area());

    // Input Block styling based on focus
    let input_title_style = Style::default().fg(Color::Yellow);
    let input_border_style = if matches!(app.focus, FocusBlock::Input) || matches!(app.focus, FocusBlock::History){
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let input_paragraph = Paragraph::new(format!("> {}", app.input))
        .style(input_title_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search (Input)")
                .border_style(input_border_style),
        );
    frame.render_widget(input_paragraph, main_layout[0]);

    // Output Block styling and list creation
    let output_border_style = if matches!(app.focus, FocusBlock::Output) {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let output_block_base = Block::default()
        .borders(Borders::ALL)
        .title("Results (Output)")
        .border_style(output_border_style);

    if app.is_loading {
        frame.render_widget(
            Paragraph::new("Loading...").style(Style::default().fg(Color::Cyan)).block(output_block_base),
            main_layout[1],
        );
    } else if !app.err_msg.is_empty() {
        frame.render_widget(
            Paragraph::new(app.err_msg.as_str())
                .style(Style::default().fg(Color::Red))
                .block(output_block_base),
            main_layout[1],
        );
    } else if !app.output.is_empty() {
        let items: Vec<ListItem> = app
            .output
            .iter()
            .enumerate()
            .map(|(i, res)| {
                let item_text = format!("[{}] {} :: {}", res.spawner, res.action, res.description);
                if matches!(app.focus, FocusBlock::Output) && i == app.selected_output_index {
                    ListItem::new(item_text).style(Style::default().fg(Color::Black).bg(Color::Cyan))
                } else {
                    ListItem::new(item_text).style(Style::default().fg(Color::Cyan))
                }
            })
            .collect();
        frame.render_widget(
            List::new(items).block(output_block_base),
            main_layout[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("No results. Type a query and press Enter.").style(Style::default().fg(Color::DarkGray)).block(output_block_base),
            main_layout[1],
        );
    }

    // History Block
    let history_border_style = if matches!(app.focus, FocusBlock::History) {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let history_items: Vec<ListItem> = app.history.iter().enumerate().map(|(idx, entry)|{
        if matches!(app.focus, FocusBlock::History) && app.history_index == Some(idx) {
            ListItem::new(entry.as_str()).style(Style::default().fg(Color::Black).bg(Color::Magenta))
        } else {
            ListItem::new(entry.as_str()).style(Style::default().fg(Color::Gray))
        }
    }).collect();

    let history_list = List::new(history_items)
        .block(Block::default().borders(Borders::ALL).title("History").border_style(history_border_style));
    frame.render_widget(history_list, main_layout[2]);
}