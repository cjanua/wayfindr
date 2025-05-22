// src/ui.rs
use crate::app::{App, FocusState};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input area
            Constraint::Min(1),    // Results area
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    render_input(frame, app, main_layout[0]);
    render_results(frame, app, main_layout[1]);
    render_status_bar(frame, app, main_layout[2]);

    // Render error popup if there's an error
    if let Some(ref error) = app.error_message {
        render_error_popup(frame, error);
    }
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let input_style = Style::default().fg(Color::Yellow);
    let border_style = if app.focus == FocusState::Input {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Gray)
    };

    let input_text = if app.input.is_empty() && app.focus == FocusState::Input {
        "Type to search apps, directories, or 'ai: <question>' for AI..."
    } else {
        &app.input
    };

    let input_paragraph = Paragraph::new(format!("> {}", input_text))
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search")
                .border_style(border_style),
        );

    frame.render_widget(input_paragraph, area);
}

fn render_results(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.focus == FocusState::Results {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Gray)
    };

    let results_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Results ({})", app.results.len()))
        .border_style(border_style);

    if app.is_loading {
        let loading_paragraph = Paragraph::new("🔍 Searching...")
            .style(Style::default().fg(Color::Cyan))
            .block(results_block);
        frame.render_widget(loading_paragraph, area);
    } else if app.results.is_empty() {
        let empty_paragraph = Paragraph::new("No results found. Try a different search term.")
            .style(Style::default().fg(Color::DarkGray))
            .block(results_block);
        frame.render_widget(empty_paragraph, area);
    } else {
        render_results_list(frame, app, area, results_block);
    }
}

fn render_results_list(frame: &mut Frame, app: &App, area: Rect, block: Block) {
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let icon = get_result_icon(result);
            let provider_tag = format!("[{}]", result.provider);

            let item_text = if result.description.is_empty() {
                format!("{} {} {}", icon, provider_tag, result.title)
            } else {
                format!(
                    "{} {} {} - {}",
                    icon,
                    provider_tag,
                    result.title,
                    truncate_description(&result.description, 60)
                )
            };

            let style = if app.focus == FocusState::Results && i == app.selected_index {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(item_text).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan));

    let mut list_state = ListState::default();
    if app.focus == FocusState::Results && !app.results.is_empty() {
        list_state.select(Some(app.selected_index));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut status_parts = Vec::new();

    // Focus indicator
    let focus_text = match app.focus {
        FocusState::Input => "INPUT",
        FocusState::Results => "RESULTS",
    };
    status_parts.push(format!("Focus: {}", focus_text));

    // History indicator
    if !app.history.is_empty() {
        status_parts.push(format!("History: {}", app.history.len()));
    }

    // Controls
    status_parts.push("ESC:Exit".to_string());
    status_parts.push("TAB:Switch".to_string());
    status_parts.push("↑↓:Navigate".to_string());
    status_parts.push("Enter:Select".to_string());

    let status_text = status_parts.join(" | ");
    let status_paragraph = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));

    frame.render_widget(status_paragraph, area);
}

fn render_error_popup(frame: &mut Frame, error_message: &str) {
    let popup_area = centered_rect(60, 20, frame.area());

    // Clear the background
    frame.render_widget(Clear, popup_area);

    let error_paragraph = Paragraph::new(error_message)
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .border_style(Style::default().fg(Color::Red)),
        );

    frame.render_widget(error_paragraph, popup_area);
}

fn get_result_icon(result: &crate::types::ActionResult) -> &'static str {
    use crate::types::ActionType;

    match &result.action {
        ActionType::Launch {
            needs_terminal: true,
        } => "⚡",
        ActionType::Launch {
            needs_terminal: false,
        } => "🚀",
        ActionType::Navigate { .. } => "📁",
        ActionType::AiResponse => "🤖",
        ActionType::Custom { .. } => "⚙️",
    }
}

fn truncate_description(description: &str, max_length: usize) -> String {
    if description.len() <= max_length {
        description.to_string()
    } else {
        format!("{}...", &description[..max_length.saturating_sub(3)])
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
