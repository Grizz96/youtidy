use crate::{App, AppState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top: Search Input
            Constraint::Min(0),    // Bottom: Dynamic region
        ])
        .split(f.area());

    // Render Top Search Input Block
    let search_title = if app.is_searching {
        " Search YouTube (Searching...) "
    } else {
        " Search YouTube (Press Enter) "
    };
    
    let search_block = Paragraph::new(app.search_input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if app.is_searching {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            })
            .title(search_title),
    );
    f.render_widget(search_block, chunks[0]);

    match app.state {
        AppState::Searching | AppState::Selecting => {
            // Layout State 1: Search & Select Mode
            // Bottom area is the Search Results list
            app.results_area = chunks[1];

            if app.is_searching {
                // High-fidelity Loading Screen
                let loading_lines = vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from(""),
                    Line::from(vec![
                        ratatui::text::Span::styled("🔍 Searching for: ", Style::default().fg(Color::Cyan)),
                        ratatui::text::Span::styled(format!("\"{}\"", app.search_input), Style::default().add_modifier(Modifier::ITALIC).fg(Color::White)),
                    ]),
                    Line::from(""),
                    Line::from(ratatui::text::Span::styled("Fetching metadata from YouTube, please wait...", Style::default().fg(Color::DarkGray))),
                ];
                let loading_p = Paragraph::new(loading_lines)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Yellow))
                            .title(" Search Results "),
                    );
                f.render_widget(loading_p, chunks[1]);
            } else {
                let items: Vec<ListItem> = app
                    .search_results
                    .iter()
                    .map(|res| ListItem::new(Line::from(res.title.as_str())))
                    .collect();

                let list_title = if app.search_results.is_empty() {
                    " Search Results (Type a query and press Enter) "
                } else {
                    " Search Results (Up/Down Arrows, Enter, or Mouse Click) "
                };

                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(if !app.search_results.is_empty() {
                                Style::default().fg(Color::Green)
                            } else {
                                Style::default()
                            })
                            .title(list_title)
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );

                f.render_stateful_widget(list, chunks[1], &mut app.list_state);
            }
        }
        AppState::Processing => {
            // Layout State 2: Processing Mode
            // Subdivide the bottom area into Middle (Selected Summary) and Bottom (Processing Box)
            let sub_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Middle: Selected song summary block
                    Constraint::Min(0),    // Bottom: Processing Box logs
                ])
                .split(chunks[1]);

            // Middle: Selected Song Summary
            let selected_title = app
                .selected_video
                .as_ref()
                .map(|v| v.title.as_str())
                .unwrap_or("Unknown Song");
            let summary_block = Paragraph::new(selected_title).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Selected Video "),
            );
            f.render_widget(summary_block, sub_chunks[0]);

            // Bottom: Large Processing Box showing logs
            let logs_lines: Vec<Line> = app
                .logs
                .iter()
                .map(|log| {
                    let style = if log.starts_with("[+]") {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else if log.starts_with("[!]") {
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                    } else if log.starts_with("[>]") {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(ratatui::text::Span::styled(log.as_str(), style))
                })
                .collect();

            // Auto-scroll logic: if there are more logs than can fit in height, we can show them.
            // Since we use standard paragraph, let's show status in the title.
            let status_title = if app.logs.iter().any(|l| l.contains("successful")) {
                " Real-time Async Logs (FINISHED - Press Esc to search again, q to exit) "
            } else if app.logs.iter().any(|l| l.contains("Error:")) {
                " Real-time Async Logs (FAILED - Press Esc to try again, q to exit) "
            } else {
                " Real-time Async Logs (Processing...) "
            };

            let logs_paragraph = Paragraph::new(logs_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title(status_title),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(logs_paragraph, sub_chunks[1]);
        }
    }
}
