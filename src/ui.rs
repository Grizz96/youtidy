use crate::{App, AppState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.state {
        AppState::MainMenu => {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Spacer
                    Constraint::Length(3), // Title
                    Constraint::Length(7), // Menu box
                    Constraint::Min(0),    // Instructions
                ])
                .split(f.area());

            let title_p = Paragraph::new("youtidy 🎵")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, layout[1]);
            app.menu_area = layout[2];

            let item_search = if app.menu_index == 0 {
                " > 🔍 Search & Download Single Song < "
            } else {
                "   🔍 Search & Download Single Song   "
            };
            let item_playlist = if app.menu_index == 1 {
                " > 📋 Download from Playlist (YouTube / Spotify) < "
            } else {
                "   📋 Download from Playlist (YouTube / Spotify)   "
            };

            let menu_lines = vec![
                Line::from(""),
                Line::from(ratatui::text::Span::styled(
                    item_search,
                    if app.menu_index == 0 {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    }
                )),
                Line::from(""),
                Line::from(ratatui::text::Span::styled(
                    item_playlist,
                    if app.menu_index == 1 {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    }
                )),
            ];

            let menu_block = Paragraph::new(menu_lines)
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Select Mode ")
                        .border_style(Style::default().fg(Color::Yellow))
                );
            f.render_widget(menu_block, layout[2]);

            let help_text = Paragraph::new("[↑/↓] Navigate  [Enter] Select Option  [q] Quit")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(help_text, layout[3]);
        }
        AppState::PlaylistInput => {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Top spacing/Title
                    Constraint::Length(3), // Input box
                    Constraint::Min(0),    // Instructions
                ])
                .split(f.area());

            let title_p = Paragraph::new("youtidy 🎵 - Playlist Downloader")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, layout[0]);

            let input_title = if app.playlist_is_loading {
                " Fetching Playlist Tracks (Please wait...) "
            } else {
                " Enter YouTube Playlist URL & Press Enter "
            };

            let input_block = Paragraph::new(app.playlist_input.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if app.playlist_is_loading {
                        Style::default().fg(Color::Yellow)
                    } else if app.playlist_error.is_some() {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Green)
                    })
                    .title(input_title),
            );
            f.render_widget(input_block, layout[1]);

            let mut help_lines = vec![
                Line::from(""),
                Line::from(ratatui::text::Span::styled(
                    "[Esc] Back to Main Menu  [Enter] Load Playlist",
                    Style::default().fg(Color::DarkGray)
                ))
            ];
            if app.playlist_is_loading {
                help_lines.push(Line::from(""));
                help_lines.push(Line::from(ratatui::text::Span::styled(
                    "⏳ Loading YouTube playlist tracks via yt-dlp...",
                    Style::default().fg(Color::Yellow)
                )));
            }
            if let Some(ref err) = app.playlist_error {
                help_lines.push(Line::from(""));
                help_lines.push(Line::from(ratatui::text::Span::styled(
                    format!("❌ Load Error: {}", err),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                )));
            }
            let help_p = Paragraph::new(help_lines).alignment(Alignment::Center);
            f.render_widget(help_p, layout[2]);
        }
        AppState::PlaylistConfirm => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(60), // Left: list
                    Constraint::Percentage(40), // Right: Info/actions
                ])
                .split(f.area());
            app.playlist_area = chunks[0];

            let items: Vec<ListItem> = app.playlist_tracks
                .iter()
                .enumerate()
                .map(|(idx, track)| {
                    let is_selected = app.playlist_selected_indices.contains(&idx);
                    let checkbox = if is_selected { "[X] " } else { "[ ] " };
                    let style = if is_selected {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    ListItem::new(Line::from(vec![
                        ratatui::text::Span::styled(checkbox, if is_selected { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
                        ratatui::text::Span::styled(format!("{} - {}", track.artist, track.title), style),
                    ]))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green))
                        .title(" Playlist Tracks ")
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );

            f.render_stateful_widget(list, chunks[0], &mut app.playlist_list_state);

            let scrollbar = ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("┃");
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(app.playlist_tracks.len())
                .position(app.playlist_list_state.selected().unwrap_or(0));
            f.render_stateful_widget(
                scrollbar,
                chunks[0].inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                &mut scrollbar_state,
            );

            let total = app.playlist_tracks.len();
            let selected = app.playlist_selected_indices.len();
            
            let info_lines = vec![
                Line::from(""),
                Line::from(vec![
                    ratatui::text::Span::styled(" Playlist Loaded!", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(format!(" Total Tracks    : {}", total)),
                Line::from(format!(" Selected        : {}", selected)),
                Line::from(""),
                Line::from(" Controls:"),
                Line::from(ratatui::text::Span::styled(" [↑/↓]  Navigate Tracks", Style::default().fg(Color::Cyan))),
                Line::from(ratatui::text::Span::styled(" [Space] Toggle Selection", Style::default().fg(Color::Cyan))),
                Line::from(ratatui::text::Span::styled(" [A]     Select/Deselect All", Style::default().fg(Color::Cyan))),
                Line::from(""),
                Line::from(ratatui::text::Span::styled(" [D] or [Enter] Start Download", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                Line::from(ratatui::text::Span::styled(" [Esc]   Back to Link Input", Style::default().fg(Color::Yellow))),
            ];

            let info_block = Paragraph::new(info_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Info & Controls ")
                        .border_style(Style::default().fg(Color::Yellow))
                );
            f.render_widget(info_block, chunks[1]);
        }
        AppState::PlaylistProcessing => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40), // Left: Track Statuses
                    Constraint::Percentage(60), // Right: Detailed processing log
                ])
                .split(f.area());

            let items: Vec<ListItem> = app.playlist_tracks
                .iter()
                .enumerate()
                .map(|(idx, track)| {
                    let is_active = idx == app.current_playlist_track_index;
                    let (status_icon, style) = match &track.status {
                        crate::PlaylistTrackStatus::Pending => ("⏳ ", Style::default().fg(Color::DarkGray)),
                        crate::PlaylistTrackStatus::Processing => ("🔄 ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        crate::PlaylistTrackStatus::Success(_) => ("✅ ", Style::default().fg(Color::Green)),
                        crate::PlaylistTrackStatus::Failed(_) => ("❌ ", Style::default().fg(Color::Red)),
                    };
                    
                    let mut line_style = style;
                    if is_active && track.status == crate::PlaylistTrackStatus::Processing {
                        line_style = line_style.add_modifier(Modifier::UNDERLINED);
                    }

                    ListItem::new(Line::from(vec![
                        ratatui::text::Span::styled(status_icon, style),
                        ratatui::text::Span::styled(format!("{} - {}", track.artist, track.title), line_style),
                    ]))
                })
                .collect();

            let queue_block = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan))
                        .title(" Download Progress ")
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );

            app.playlist_list_state.select(Some(app.current_playlist_track_index));
            f.render_stateful_widget(queue_block, chunks[0], &mut app.playlist_list_state);

            let queue_scrollbar = ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("┃");
            let mut queue_scrollbar_state = ratatui::widgets::ScrollbarState::new(app.playlist_tracks.len())
                .position(app.current_playlist_track_index);
            f.render_stateful_widget(
                queue_scrollbar,
                chunks[0].inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                &mut queue_scrollbar_state,
            );

            let logs_lines: Vec<Line> = app.logs
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

            let status_title = if app.logs.iter().any(|l| l.contains("finished")) {
                " Real-time Logs (FINISHED - Press Esc to go back) "
            } else {
                " Real-time Logs (Processing...) "
            };

            let total_lines = logs_lines.len() as u16;
            let visible_height = chunks[1].height.saturating_sub(2);
            if !app.log_manual_scroll {
                app.log_scroll_y = total_lines.saturating_sub(visible_height);
            } else {
                let max_scroll = total_lines.saturating_sub(visible_height);
                if app.log_scroll_y > max_scroll {
                    app.log_scroll_y = max_scroll;
                    app.log_manual_scroll = false;
                }
            }

            let logs_paragraph = Paragraph::new(logs_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title(status_title),
                )
                .scroll((app.log_scroll_y, 0))
                .wrap(Wrap { trim: true });
            f.render_widget(logs_paragraph, chunks[1]);

            let logs_scrollbar = ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("┃");
            let mut logs_scrollbar_state = ratatui::widgets::ScrollbarState::new(total_lines as usize)
                .position(app.log_scroll_y as usize);
            f.render_stateful_widget(
                logs_scrollbar,
                chunks[1].inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                &mut logs_scrollbar_state,
            );
        }
        AppState::Searching | AppState::Selecting | AppState::Processing => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Top: Search Input
                    Constraint::Min(0),    // Bottom: Dynamic region
                ])
                .split(f.area());

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
                    app.results_area = chunks[1];

                    if app.is_searching {
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

                        let scrollbar = ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                            .begin_symbol(Some("▲"))
                            .end_symbol(Some("▼"))
                            .track_symbol(Some("│"))
                            .thumb_symbol("┃");
                        let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(app.search_results.len())
                            .position(app.list_state.selected().unwrap_or(0));
                        f.render_stateful_widget(
                            scrollbar,
                            chunks[1].inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                            &mut scrollbar_state,
                        );
                    }
                }
                AppState::Processing => {
                    let sub_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Middle: Selected song summary block
                            Constraint::Min(0),    // Bottom: Processing Box logs
                        ])
                        .split(chunks[1]);

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

                    let status_title = if app.logs.iter().any(|l| l.contains("successful")) {
                        " Real-time Async Logs (FINISHED - Press Esc to search again, q to exit) "
                    } else if app.logs.iter().any(|l| l.contains("Error:")) {
                        " Real-time Async Logs (FAILED - Press Esc to try again, q to exit) "
                    } else {
                        " Real-time Async Logs (Processing...) "
                    };

                    let total_lines = logs_lines.len() as u16;
                    let visible_height = sub_chunks[1].height.saturating_sub(2);
                    if !app.log_manual_scroll {
                        app.log_scroll_y = total_lines.saturating_sub(visible_height);
                    } else {
                        let max_scroll = total_lines.saturating_sub(visible_height);
                        if app.log_scroll_y > max_scroll {
                            app.log_scroll_y = max_scroll;
                            app.log_manual_scroll = false;
                        }
                    }

                    let logs_paragraph = Paragraph::new(logs_lines)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Yellow))
                                .title(status_title),
                        )
                        .scroll((app.log_scroll_y, 0))
                        .wrap(Wrap { trim: true });
                    f.render_widget(logs_paragraph, sub_chunks[1]);

                    let logs_scrollbar = ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("▲"))
                        .end_symbol(Some("▼"))
                        .track_symbol(Some("│"))
                        .thumb_symbol("┃");
                    let mut logs_scrollbar_state = ratatui::widgets::ScrollbarState::new(total_lines as usize)
                        .position(app.log_scroll_y as usize);
                    f.render_stateful_widget(
                        logs_scrollbar,
                        sub_chunks[1].inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                        &mut logs_scrollbar_state,
                    );
                }
                _ => {}
            }
        }
    }
}

