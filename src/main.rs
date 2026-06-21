use dotenvy::dotenv;
use std::env;
use std::error::Error;
use std::io;
use std::process::Command;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::Rect,
    widgets::ListState,
};
use serde::Deserialize;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use lofty::picture::{MimeType, Picture, PictureType};

mod ui;
mod lyrics;

#[derive(Debug, PartialEq, Clone)]
enum AppState {
    MainMenu,
    Searching,
    Selecting,
    Processing,
    PlaylistInput,
    PlaylistConfirm,
    PlaylistProcessing,
}

struct ApiLimiter {
    last_call: tokio::sync::Mutex<std::time::Instant>,
}

impl ApiLimiter {
    fn new() -> Self {
        Self {
            last_call: tokio::sync::Mutex::new(
                std::time::Instant::now() - std::time::Duration::from_secs(2)
            ),
        }
    }

    async fn acquire(&self) {
        let mut last = self.last_call.lock().await;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(*last);
        let min_duration = std::time::Duration::from_millis(1000);
        if elapsed < min_duration {
            let wait_time = min_duration - elapsed;
            tokio::time::sleep(wait_time).await;
        }
        *last = std::time::Instant::now();
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PlaylistTrackStatus {
    Pending,
    Processing,
    Success(String),
    Failed(String),
}

#[derive(Debug, Clone)]
struct PlaylistTrack {
    title: String,
    artist: String,
    url: Option<String>,
    id: String,
    status: PlaylistTrackStatus,
}

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
    id: String,
}

struct App {
    state: AppState,
    search_input: String,
    search_results: Vec<SearchResult>,
    list_state: ListState,
    results_area: Rect,
    selected_video: Option<SearchResult>,
    logs: Vec<String>,
    is_searching: bool,
    
    // Playlist fields
    menu_index: usize,
    playlist_input: String,
    playlist_tracks: Vec<PlaylistTrack>,
    playlist_list_state: ListState,
    playlist_selected_indices: std::collections::HashSet<usize>,
    current_playlist_track_index: usize,
    playlist_is_loading: bool,
    playlist_error: Option<String>,
    log_scroll_y: u16,
    log_manual_scroll: bool,
    playlist_area: Rect,
    menu_area: Rect,
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::MainMenu,
            search_input: String::new(),
            search_results: Vec::new(),
            list_state: ListState::default(),
            results_area: Rect::default(),
            selected_video: None,
            logs: Vec::new(),
            is_searching: false,
            
            menu_index: 0,
            playlist_input: String::new(),
            playlist_tracks: Vec::new(),
            playlist_list_state: ListState::default(),
            playlist_selected_indices: std::collections::HashSet::new(),
            current_playlist_track_index: 0,
            playlist_is_loading: false,
            playlist_error: None,
            log_scroll_y: 0,
            log_manual_scroll: false,
            playlist_area: Rect::default(),
            menu_area: Rect::default(),
        }
    }

    fn next(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.search_results.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.search_results.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn playlist_next(&mut self) {
        if self.playlist_tracks.is_empty() {
            return;
        }
        let i = match self.playlist_list_state.selected() {
            Some(i) => {
                if i >= self.playlist_tracks.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    fn playlist_previous(&mut self) {
        if self.playlist_tracks.is_empty() {
            return;
        }
        let i = match self.playlist_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.playlist_tracks.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    fn toggle_track_selection(&mut self) {
        if let Some(i) = self.playlist_list_state.selected() {
            if self.playlist_selected_indices.contains(&i) {
                self.playlist_selected_indices.remove(&i);
            } else {
                self.playlist_selected_indices.insert(i);
            }
        }
    }

    fn on_click(&mut self, row: u16) -> Option<SearchResult> {
        if row > self.results_area.y && row < self.results_area.y + self.results_area.height - 1 {
            let index = self.list_state.offset() + (row - self.results_area.y - 1) as usize;
            if index < self.search_results.len() {
                self.list_state.select(Some(index));
                return Some(self.search_results[index].clone());
            }
        }
        None
    }
}

#[derive(Debug)]
enum AppEvent {
    Input(Event),
    SearchStart,
    SearchSuccess(Vec<SearchResult>),
    SearchError(String),
    ProcessingLog(String),
    ProcessingFinished(Result<(String, std::time::Duration), String>),
    
    // Playlist events
    PlaylistLoadStart,
    PlaylistLoadSuccess(Vec<PlaylistTrack>),
    PlaylistLoadError(String),
    PlaylistTrackUpdate(usize, PlaylistTrackStatus),
    PlaylistFinished(std::time::Duration),
}


#[derive(Debug, Deserialize)]
struct YtdlSearchResponse {
    entries: Option<Vec<YtdlEntry>>,
}

#[derive(Debug, Deserialize)]
struct YtdlEntry {
    id: String,
    title: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FpcalcOutput {
    duration: f64,
    fingerprint: String,
}

#[derive(Debug, Deserialize)]
struct AcoustIdResponse {
    status: String,
    results: Vec<AcoustIdResult>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AcoustIdResult {
    score: f64,
    id: String,
    recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(Debug, Deserialize)]
struct AcoustIdRecording {
    id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct MusicBrainzRecording {
    id: String,
    title: String,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<ArtistCredit>>,
    releases: Option<Vec<MusicBrainzRelease>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct ArtistCredit {
    name: String,
    joinphrase: Option<String>,
    artist: Artist,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct Artist {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct MusicBrainzRelease {
    id: String,
    title: String,
    date: Option<String>,
    #[serde(rename = "release-group")]
    release_group: Option<ReleaseGroup>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct ReleaseGroup {
    id: String,
    title: Option<String>,
    #[serde(rename = "primary-type")]
    primary_type: Option<String>,
    #[serde(rename = "secondary-types", default)]
    secondary_types: Vec<String>,
    #[serde(rename = "first-release-date")]
    first_release_date: Option<String>,
}
fn load_config() {
    // 1. Try to find config.toml in current directory
    let mut config_path = std::path::PathBuf::from("config.toml");
    if !config_path.exists() {
        // 2. Try ~/.config/youtidy/config.toml
        if let Ok(home_dir) = env::var("HOME") {
            let mut home = std::path::PathBuf::from(home_dir);
            home.push(".config");
            home.push("youtidy");
            home.push("config.toml");
            config_path = home;
        }
    }

    if config_path.exists()
        && let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(config) = content.parse::<toml::Table>() {
                if let Some(music) = config.get("music").and_then(|v| v.as_table()) {
                    if let Some(dir) = music.get("directory").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("YOUTIDY_MUSIC_DIR", dir); }
                    }
                    if let Some(fmt) = music.get("format").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("YOUTIDY_FORMAT", fmt); }
                    }
                    if let Some(save_cov) = music.get("save_cover_file").and_then(|v| v.as_bool()) {
                        unsafe { env::set_var("YOUTIDY_SAVE_COVER_FILE", save_cov.to_string()); }
                    }
                    if let Some(cache_d) = music.get("cache_directory").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("YOUTIDY_CACHE_DIR", cache_d); }
                    }
                    if let Some(bitrate) = music.get("bitrate") {
                        if let Some(b_int) = bitrate.as_integer() {
                            unsafe { env::set_var("YOUTIDY_BITRATE", b_int.to_string()); }
                        } else if let Some(b_str) = bitrate.as_str() {
                            unsafe { env::set_var("YOUTIDY_BITRATE", b_str); }
                        }
                    }
                    if let Some(quality) = music.get("audio_quality") {
                        if let Some(q_int) = quality.as_integer() {
                            unsafe { env::set_var("YOUTIDY_AUDIO_QUALITY", q_int.to_string()); }
                        } else if let Some(q_str) = quality.as_str() {
                            unsafe { env::set_var("YOUTIDY_AUDIO_QUALITY", q_str); }
                        }
                    }
                    if let Some(folder_struct) = music.get("folder_structure").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("YOUTIDY_FOLDER_STRUCTURE", folder_struct); }
                    }
                    if let Some(file_temp) = music.get("filename_template").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("YOUTIDY_FILENAME_TEMPLATE", file_temp); }
                    }
                    if let Some(emb_lyr) = music.get("embed_lyrics").and_then(|v| v.as_bool()) {
                        unsafe { env::set_var("YOUTIDY_EMBED_LYRICS", emb_lyr.to_string()); }
                    }
                    if let Some(mpc) = music.get("mpc_add").and_then(|v| v.as_bool()) {
                        unsafe { env::set_var("YOUTIDY_MPC_ADD", mpc.to_string()); }
                    }
                    if let Some(mpd_h) = music.get("mpd_host").and_then(|v| v.as_str()) {
                        let expanded = expand_tilde(mpd_h);
                        unsafe { env::set_var("MPD_HOST", expanded); }
                    }
                }
                if let Some(api) = config.get("api").and_then(|v| v.as_table()) {
                    if let Some(id) = api.get("acoustid_client_id").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("ACOUSTID_CLIENT_ID", id); }
                    }
                    if let Some(tok) = api.get("genius_access_token").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("GENIUS_ACCESS_TOKEN", tok); }
                    }
                }
                if let Some(tui) = config.get("tui").and_then(|v| v.as_table()) {
                    if let Some(limit) = tui.get("search_limit") {
                        if let Some(l_int) = limit.as_integer() {
                            unsafe { env::set_var("YOUTIDY_SEARCH_LIMIT", l_int.to_string()); }
                        } else if let Some(l_str) = limit.as_str() {
                            unsafe { env::set_var("YOUTIDY_SEARCH_LIMIT", l_str); }
                        }
                    }
                    if let Some(p_limit) = tui.get("playlist_limit") {
                        if let Some(pl_int) = p_limit.as_integer() {
                            unsafe { env::set_var("YOUTIDY_PLAYLIST_LIMIT", pl_int.to_string()); }
                        } else if let Some(pl_str) = p_limit.as_str() {
                            unsafe { env::set_var("YOUTIDY_PLAYLIST_LIMIT", pl_str); }
                        }
                    }
                }
                if let Some(network) = config.get("network").and_then(|v| v.as_table()) {
                    if let Some(ua) = network.get("user_agent").and_then(|v| v.as_str()) {
                        unsafe { env::set_var("YOUTIDY_USER_AGENT", ua); }
                    }
                    if let Some(proxy) = network.get("proxy").and_then(|v| v.as_str()) {
                        unsafe {
                            env::set_var("YOUTIDY_PROXY", proxy);
                            env::set_var("HTTP_PROXY", proxy);
                            env::set_var("HTTPS_PROXY", proxy);
                            env::set_var("http_proxy", proxy);
                            env::set_var("https_proxy", proxy);
                        }
                    }
                }
            }
}

fn build_http_client() -> Result<reqwest::Client, Box<dyn Error + Send + Sync>> {
    let mut builder = reqwest::Client::builder();
    
    let user_agent = env::var("YOUTIDY_USER_AGENT")
        .unwrap_or_else(|_| "youtidy/1.0.0 ( github.com/Grizz96/youtidy )".to_string());
    if !user_agent.trim().is_empty() {
        builder = builder.user_agent(user_agent);
    }
    
    if let Ok(proxy_str) = env::var("YOUTIDY_PROXY")
        && !proxy_str.trim().is_empty() {
            let proxy = reqwest::Proxy::all(proxy_str)?;
            builder = builder.proxy(proxy);
        }
    
    Ok(builder.build()?)
}

fn check_dependencies() -> Result<(), Box<dyn Error>> {
    let binaries = ["yt-dlp", "ffmpeg", "fpcalc"];
    for bin in binaries {
        let output = if bin == "fpcalc" {
            Command::new(bin).arg("-version").output()
        } else {
            Command::new(bin).arg("--version").output()
        };

        if output.is_err() {
            return Err(format!("Required binary '{}' not found in PATH", bin).into());
        }
    }

    if env::var("ACOUSTID_CLIENT_ID").is_err() {
        return Err("ACOUSTID_CLIENT_ID environment variable not set (check config.toml or .env file)".into());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    load_config();

    // 1. Startup Validation
    if let Err(e) = check_dependencies() {
        eprintln!("Initialization Error: {}", e);
        std::process::exit(1);
    }

    // 2. Terminal Setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. App State & Event Loop
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app).await;

    // 4. Restore Terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn Error>>
where
    <B as Backend>::Error: 'static,
{
    // Create channels
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

    // Spawn thread to read crossterm events and send them to the channel
    let keys_tx = event_tx.clone();
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if keys_tx.blocking_send(AppEvent::Input(ev)).is_err() {
                break;
            }
        }
    });

    // Draw initial state
    terminal.draw(|f| ui::draw(f, app))?;

    while let Some(event) = event_rx.recv().await {
        match event {
            AppEvent::Input(ev) => {
                match ev {
                    Event::Key(key) => {
                        if key.kind == event::KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Esc => {
                                    match app.state {
                                        AppState::MainMenu => return Ok(()),
                                        AppState::Searching | AppState::PlaylistInput => {
                                            app.state = AppState::MainMenu;
                                        }
                                        AppState::Selecting => {
                                            app.state = AppState::Searching;
                                        }
                                        AppState::Processing => {
                                            app.state = AppState::Searching;
                                            app.logs.clear();
                                            app.log_scroll_y = 0;
                                            app.log_manual_scroll = false;
                                        }
                                        AppState::PlaylistConfirm => {
                                            app.state = AppState::PlaylistInput;
                                        }
                                        AppState::PlaylistProcessing => {
                                            app.state = AppState::PlaylistConfirm;
                                            app.log_scroll_y = 0;
                                            app.log_manual_scroll = false;
                                        }
                                    }
                                }
                                KeyCode::Char(c) => {
                                    match app.state {
                                        AppState::Searching => {
                                            if !app.is_searching {
                                                app.search_input.push(c);
                                            }
                                        }
                                        AppState::PlaylistInput => {
                                            if !app.playlist_is_loading {
                                                app.playlist_input.push(c);
                                            }
                                        }
                                        AppState::PlaylistConfirm => {
                                            if c == ' ' {
                                                app.toggle_track_selection();
                                            } else if c == 'a' || c == 'A' {
                                                if app.playlist_selected_indices.len() == app.playlist_tracks.len() {
                                                    app.playlist_selected_indices.clear();
                                                } else {
                                                    app.playlist_selected_indices = (0..app.playlist_tracks.len()).collect();
                                                }
                                            } else if c == 'd' || c == 'D' {
                                                start_playlist_processing(app, event_tx.clone()).await;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::Backspace => {
                                    match app.state {
                                        AppState::Searching => {
                                            if !app.is_searching {
                                                app.search_input.pop();
                                            }
                                        }
                                        AppState::PlaylistInput => {
                                            if !app.playlist_is_loading {
                                                app.playlist_input.pop();
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::Enter => {
                                    match app.state {
                                        AppState::MainMenu => {
                                            if app.menu_index == 0 {
                                                app.state = AppState::Searching;
                                            } else {
                                                app.state = AppState::PlaylistInput;
                                            }
                                        }
                                        AppState::Searching => {
                                            if !app.is_searching && !app.search_input.trim().is_empty() {
                                                let query = app.search_input.clone();
                                                let tx = event_tx.clone();
                                                tokio::spawn(async move {
                                                    let _ = tx.send(AppEvent::SearchStart).await;
                                                    match search_youtube(&query).await {
                                                        Ok(results) => {
                                                            let _ = tx.send(AppEvent::SearchSuccess(results)).await;
                                                        }
                                                        Err(e) => {
                                                            let _ = tx.send(AppEvent::SearchError(e.to_string())).await;
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        AppState::Selecting => {
                                            if let Some(i) = app.list_state.selected() {
                                                let selected = app.search_results[i].clone();
                                                app.selected_video = Some(selected.clone());
                                                app.state = AppState::Processing;
                                                app.logs.clear();
                                                app.logs.push(format!("[>] Initialized pipeline for: {}", selected.title));
 
                                                let tx = event_tx.clone();
                                                tokio::spawn(async move {
                                                    let start_time = std::time::Instant::now();
                                                    match run_pipeline(selected, tx.clone(), None).await {
                                                        Ok(dest_path) => {
                                                            let elapsed = start_time.elapsed();
                                                            let _ = tx.send(AppEvent::ProcessingFinished(Ok((dest_path, elapsed)))).await;
                                                        }
                                                        Err(e) => {
                                                            let _ = tx.send(AppEvent::ProcessingFinished(Err(e.to_string()))).await;
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        AppState::PlaylistInput => {
                                            if !app.playlist_is_loading && !app.playlist_input.trim().is_empty() {
                                                let url = app.playlist_input.clone();
                                                let tx = event_tx.clone();
                                                
                                                if !url.contains("youtube.com") && !url.contains("youtu.be") && !url.contains("list=") {
                                                    tokio::spawn(async move {
                                                        let _ = tx.send(AppEvent::PlaylistLoadStart).await;
                                                        let _ = tx.send(AppEvent::PlaylistLoadError("Only YouTube playlist links are supported. Please paste a YouTube playlist URL (containing list=).".to_string())).await;
                                                    });
                                                } else {
                                                    tokio::spawn(async move {
                                                        let _ = tx.send(AppEvent::PlaylistLoadStart).await;
                                                        match get_youtube_playlist_tracks(&url).await {
                                                            Ok(tracks) => {
                                                                let _ = tx.send(AppEvent::PlaylistLoadSuccess(tracks)).await;
                                                            }
                                                            Err(e) => {
                                                                let _ = tx.send(AppEvent::PlaylistLoadError(e.to_string())).await;
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                        AppState::PlaylistConfirm => {
                                            start_playlist_processing(app, event_tx.clone()).await;
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::Down => {
                                    match app.state {
                                        AppState::MainMenu => {
                                            if app.menu_index < 1 {
                                                app.menu_index += 1;
                                            } else {
                                                app.menu_index = 0;
                                            }
                                        }
                                        AppState::Selecting if !app.is_searching => {
                                            app.next();
                                        }
                                        AppState::PlaylistConfirm => {
                                            app.playlist_next();
                                        }
                                        AppState::Processing | AppState::PlaylistProcessing => {
                                            app.log_scroll_y = app.log_scroll_y.saturating_add(1);
                                            app.log_manual_scroll = true;
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::Up => {
                                    match app.state {
                                        AppState::MainMenu => {
                                            if app.menu_index > 0 {
                                                app.menu_index -= 1;
                                            } else {
                                                app.menu_index = 1;
                                            }
                                        }
                                        AppState::Selecting if !app.is_searching => {
                                            app.previous();
                                        }
                                        AppState::PlaylistConfirm => {
                                            app.playlist_previous();
                                        }
                                        AppState::Processing | AppState::PlaylistProcessing => {
                                            app.log_scroll_y = app.log_scroll_y.saturating_sub(1);
                                            app.log_manual_scroll = true;
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::PageUp => {
                                    if app.state == AppState::Processing || app.state == AppState::PlaylistProcessing {
                                        app.log_scroll_y = app.log_scroll_y.saturating_sub(10);
                                        app.log_manual_scroll = true;
                                    }
                                }
                                KeyCode::PageDown => {
                                    if app.state == AppState::Processing || app.state == AppState::PlaylistProcessing {
                                        app.log_scroll_y = app.log_scroll_y.saturating_add(10);
                                        app.log_manual_scroll = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        match mouse.kind {
                            MouseEventKind::Down(event::MouseButton::Left) => {
                                match app.state {
                                    AppState::MainMenu => {
                                        if mouse.row > app.menu_area.y && mouse.row < app.menu_area.y + app.menu_area.height - 1 {
                                            let rel_row = mouse.row - app.menu_area.y;
                                            if rel_row == 1 || rel_row == 2 {
                                                app.menu_index = 0;
                                                app.state = AppState::Searching;
                                            } else if rel_row == 3 || rel_row == 4 {
                                                app.menu_index = 1;
                                                app.state = AppState::PlaylistInput;
                                            }
                                        }
                                    }
                                    AppState::Selecting if !app.is_searching => {
                                        if let Some(selected) = app.on_click(mouse.row) {
                                            app.selected_video = Some(selected.clone());
                                            app.state = AppState::Processing;
                                            app.logs.clear();
                                            app.logs.push(format!("[>] Initialized pipeline for: {}", selected.title));

                                            let tx = event_tx.clone();
                                            tokio::spawn(async move {
                                                let start_time = std::time::Instant::now();
                                                match run_pipeline(selected, tx.clone(), None).await {
                                                    Ok(dest_path) => {
                                                        let elapsed = start_time.elapsed();
                                                        let _ = tx.send(AppEvent::ProcessingFinished(Ok((dest_path, elapsed)))).await;
                                                    }
                                                    Err(e) => {
                                                        let _ = tx.send(AppEvent::ProcessingFinished(Err(e.to_string()))).await;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    AppState::PlaylistConfirm => {
                                        if mouse.row > app.playlist_area.y && mouse.row < app.playlist_area.y + app.playlist_area.height - 1 {
                                            let clicked_idx = app.playlist_list_state.offset() + (mouse.row - app.playlist_area.y - 1) as usize;
                                            if clicked_idx < app.playlist_tracks.len() {
                                                app.playlist_list_state.select(Some(clicked_idx));
                                                app.toggle_track_selection();
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                match app.state {
                                    AppState::MainMenu => {
                                        if app.menu_index > 0 {
                                            app.menu_index -= 1;
                                        } else {
                                            app.menu_index = 1;
                                        }
                                    }
                                    AppState::Selecting if !app.is_searching => {
                                        app.previous();
                                    }
                                    AppState::PlaylistConfirm => {
                                        app.playlist_previous();
                                    }
                                    AppState::Processing | AppState::PlaylistProcessing => {
                                        app.log_scroll_y = app.log_scroll_y.saturating_sub(1);
                                        app.log_manual_scroll = true;
                                    }
                                    _ => {}
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                match app.state {
                                    AppState::MainMenu => {
                                        if app.menu_index < 1 {
                                            app.menu_index += 1;
                                        } else {
                                            app.menu_index = 0;
                                        }
                                    }
                                    AppState::Selecting if !app.is_searching => {
                                        app.next();
                                    }
                                    AppState::PlaylistConfirm => {
                                        app.playlist_next();
                                    }
                                    AppState::Processing | AppState::PlaylistProcessing => {
                                        app.log_scroll_y = app.log_scroll_y.saturating_add(1);
                                        app.log_manual_scroll = true;
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            AppEvent::SearchStart => {
                app.is_searching = true;
                app.search_results.clear();
                app.list_state.select(None);
            }
            AppEvent::SearchSuccess(results) => {
                app.is_searching = false;
                app.search_results = results;
                if !app.search_results.is_empty() {
                    app.state = AppState::Selecting;
                    app.list_state.select(Some(0));
                } else {
                    app.state = AppState::Searching;
                }
            }
            AppEvent::SearchError(err) => {
                app.is_searching = false;
                app.logs.push(format!("[!] Search failed: {}", err));
            }
            AppEvent::ProcessingLog(log) => {
                app.logs.push(log);
            }
            AppEvent::ProcessingFinished(res) => {
                match res {
                    Ok((dest_path, elapsed)) => {
                        let formatted_time = format_duration(elapsed);
                        app.logs.push(format!("[+] Processing completed successfully in {}!", formatted_time));
                        app.logs.push(format!("[+] Saved path: {}", dest_path));
                    }
                    Err(err) => {
                        app.logs.push(format!("[!] Processing failed: {}", err));
                    }
                }
            }
            AppEvent::PlaylistLoadStart => {
                app.playlist_is_loading = true;
                app.playlist_tracks.clear();
                app.playlist_selected_indices.clear();
                app.playlist_list_state.select(None);
                app.playlist_error = None;
            }
            AppEvent::PlaylistLoadSuccess(tracks) => {
                app.playlist_is_loading = false;
                app.playlist_tracks = tracks;
                app.playlist_selected_indices = (0..app.playlist_tracks.len()).collect();
                if !app.playlist_tracks.is_empty() {
                    app.state = AppState::PlaylistConfirm;
                    app.playlist_list_state.select(Some(0));
                } else {
                    app.state = AppState::PlaylistInput;
                }
            }
            AppEvent::PlaylistLoadError(err) => {
                app.playlist_is_loading = false;
                app.playlist_error = Some(err);
            }
            AppEvent::PlaylistTrackUpdate(idx, status) => {
                if idx < app.playlist_tracks.len() {
                    app.playlist_tracks[idx].status = status;
                }
                app.current_playlist_track_index = idx;
            }
            AppEvent::PlaylistFinished(elapsed) => {
                let formatted_time = format_duration(elapsed);
                app.logs.push(format!("[+] Playlist processing finished in {}!", formatted_time));
            }
        }
        terminal.draw(|f| ui::draw(f, app))?;
    }

    Ok(())
}

async fn start_playlist_processing(app: &mut App, tx: tokio::sync::mpsc::Sender<AppEvent>) {
    if app.playlist_selected_indices.is_empty() {
        return;
    }
    app.state = AppState::PlaylistProcessing;
    app.current_playlist_track_index = 0;
    app.logs.clear();
    app.logs.push("[>] Initialized playlist pipeline...".to_string());
    
    // Reset statuses of selected tracks
    for i in 0..app.playlist_tracks.len() {
        if app.playlist_selected_indices.contains(&i) {
            app.playlist_tracks[i].status = PlaylistTrackStatus::Pending;
        }
    }
    
    let tracks = app.playlist_tracks.clone();
    let selected_indices = app.playlist_selected_indices.clone();
    
    // Initialize Semaphore and ApiLimiter
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(3)); // max 3 concurrent downloads
    let api_limiter = std::sync::Arc::new(ApiLimiter::new());
    
    let playlist_start_time = std::time::Instant::now();

    tokio::spawn(async move {
        let mut tasks = Vec::new();
        
        for i in 0..tracks.len() {
            if !selected_indices.contains(&i) {
                continue;
            }
            
            let track = tracks[i].clone();
            let tx_clone = tx.clone();
            let sem_clone = semaphore.clone();
            let limiter_clone = api_limiter.clone();
            
            let task = tokio::spawn(async move {
                // Acquire permit before starting processing
                let permit = sem_clone.acquire_owned().await.unwrap();
                
                let _ = tx_clone.send(AppEvent::PlaylistTrackUpdate(i, PlaylistTrackStatus::Processing)).await;
                let _ = tx_clone.send(AppEvent::ProcessingLog(format!("[>] Starting track: {}", track.title))).await;
                
                let search_result = SearchResult {
                    title: track.title.clone(),
                    url: track.url.clone().unwrap_or_default(),
                    id: track.id.clone(),
                };
                
                let track_start_time = std::time::Instant::now();
                // Run the actual pipeline (passing the limiter for rate limiting)
                let res = run_pipeline(search_result, tx_clone.clone(), Some(limiter_clone)).await;
                let track_elapsed = track_start_time.elapsed();
                
                // Explicitly drop permit here to allow next task in line to start downloading
                drop(permit);
                
                match res {
                    Ok(dest_path) => {
                        let _ = tx_clone.send(AppEvent::PlaylistTrackUpdate(i, PlaylistTrackStatus::Success(dest_path))).await;
                        let formatted_time = format_duration(track_elapsed);
                        let _ = tx_clone.send(AppEvent::ProcessingLog(format!("[+] Track finished in {}: {}", formatted_time, track.title))).await;
                    }
                    Err(e) => {
                        let _ = tx_clone.send(AppEvent::PlaylistTrackUpdate(i, PlaylistTrackStatus::Failed(e.to_string()))).await;
                        let _ = tx_clone.send(AppEvent::ProcessingLog(format!("[!] Track failed: {}", track.title))).await;
                    }
                }
            });
            
            tasks.push(task);
            // Small pause between spawning to prevent instant bursts
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Wait for all spawned tasks to finish
        for task in tasks {
            let _ = task.await;
        }
        
        let total_elapsed = playlist_start_time.elapsed();
        let _ = tx.send(AppEvent::PlaylistFinished(total_elapsed)).await;
    });

}

async fn search_youtube(query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let limit = env::var("YOUTIDY_SEARCH_LIMIT").unwrap_or_else(|_| "10".to_string());
    
    let mut cmd = tokio::process::Command::new("yt-dlp");
    cmd.arg(format!("ytsearch{}:{}", limit, query))
       .arg("-J")
       .arg("--flat-playlist");

    if let Ok(proxy) = env::var("YOUTIDY_PROXY")
        && !proxy.trim().is_empty() {
            cmd.arg("--proxy").arg(&proxy);
        }

    if let Ok(ua) = env::var("YOUTIDY_USER_AGENT")
        && !ua.trim().is_empty() {
            cmd.arg("--user-agent").arg(&ua);
        }

    let output = cmd.output().await?;

    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp search failed: {}", err_str).into());
    }

    let response: YtdlSearchResponse = serde_json::from_slice(&output.stdout)?;
    let mut results = Vec::new();
    if let Some(entries) = response.entries {
        for entry in entries {
            let title = entry.title.unwrap_or_else(|| "Unknown Title".to_string());
            let url = if let Some(u) = entry.url {
                u
            } else {
                format!("https://www.youtube.com/watch?v={}", entry.id)
            };
            results.push(SearchResult {
                title,
                url,
                id: entry.id,
            });
        }
    }
    Ok(results)
}

async fn get_youtube_playlist_tracks(url: &str) -> Result<Vec<PlaylistTrack>, Box<dyn std::error::Error + Send + Sync>> {
    let mut cmd = tokio::process::Command::new("yt-dlp");
    cmd.arg("--flat-playlist")
       .arg("-J");
       
    // Only apply limit for auto-generated mix/radio playlists (contain list=RD, list=UL or start_radio=)
    let is_mix = url.contains("list=RD") || url.contains("list=UL") || url.contains("start_radio=");
    if is_mix {
        let limit = env::var("YOUTIDY_PLAYLIST_LIMIT").unwrap_or_else(|_| "50".to_string());
        if !limit.trim().is_empty() && limit != "0" {
            cmd.arg("--playlist-end").arg(&limit);
        }
    }
       
    // Apply proxy/ua if set in env
    if let Ok(proxy) = env::var("YOUTIDY_PROXY")
        && !proxy.trim().is_empty() {
            cmd.arg("--proxy").arg(&proxy);
        }
    if let Ok(ua) = env::var("YOUTIDY_USER_AGENT")
        && !ua.trim().is_empty() {
            cmd.arg("--user-agent").arg(&ua);
        }
        
    cmd.arg(url);
    
    let output = cmd.output().await?;
    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp failed to load playlist: {}", err_str).into());
    }
    
    let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    
    let entries = response.get("entries")
        .and_then(|v| v.as_array())
        .ok_or("No entries found in playlist")?;
        
    let mut tracks = Vec::new();
    for entry in entries {
        let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = entry.get("title").and_then(|v| v.as_str()).unwrap_or("Unknown Title").to_string();
        let uploader = entry.get("uploader").and_then(|v| v.as_str()).unwrap_or("Unknown Artist").to_string();
        
        let video_url = format!("https://www.youtube.com/watch?v={}", id);
        
        tracks.push(PlaylistTrack {
            title,
            artist: uploader,
            url: Some(video_url),
            id,
            status: PlaylistTrackStatus::Pending,
        });
    }
    
    Ok(tracks)
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CacheRecord {
    dest_path: String,
}

async fn run_pipeline(
    video: SearchResult,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
    api_limiter: Option<std::sync::Arc<ApiLimiter>>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let format = env::var("YOUTIDY_FORMAT").unwrap_or_else(|_| "mp3".to_string());
    let cache_dir = expand_tilde(&env::var("YOUTIDY_CACHE_DIR").unwrap_or_else(|_| get_default_cache_dir()));
    
    // Check cache first
    let cache_record_path = format!("{}/{}.json", cache_dir, video.id);
    let cache_path = std::path::Path::new(&cache_record_path);
    if cache_path.exists()
        && let Ok(cache_content) = std::fs::read_to_string(cache_path)
            && let Ok(record) = serde_json::from_str::<CacheRecord>(&cache_content) {
                let dest_path = std::path::Path::new(&record.dest_path);
                let lrc_path = dest_path.with_extension("lrc");
                if dest_path.exists() && lrc_path.exists() && dest_path.extension().and_then(|e| e.to_str()) == Some(&format) {
                    let _ = tx.send(AppEvent::ProcessingLog(format!(
                        "[>] Cache hit: song and lyrics already downloaded. Found at: {}",
                        record.dest_path
                    ))).await;
                    return Ok(record.dest_path);
                } else {
                    let _ = tx.send(AppEvent::ProcessingLog("[!] Cache found but audio or lyrics file is missing. Re-downloading...".to_string())).await;
                }
            }

    let temp_filename = format!("{}/temp_download_{}.{}", cache_dir, video.id, format);
    let temp_path = std::path::Path::new(&temp_filename);

    // 1. Download audio
    let _ = tx.send(AppEvent::ProcessingLog("[>] Starting download...".to_string())).await;
    std::fs::create_dir_all(&cache_dir)?;
    
    // Clean up any old temporary files for this video ID
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with(&format!("temp_download_{}", video.id)) {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
    }

    // Prioritize downloading a native stream matching the requested target format
    let preferred_format = match format.as_str() {
        "m4a" => "bestaudio[ext=m4a]/bestaudio/best",
        "opus" => "bestaudio[acodec=opus]/bestaudio[ext=webm]/bestaudio/best",
        _ => "bestaudio/best",
    };

    let mut cmd = tokio::process::Command::new("yt-dlp");
    cmd.arg("-f").arg(preferred_format);

    // Network options
    if let Ok(proxy) = env::var("YOUTIDY_PROXY")
        && !proxy.trim().is_empty() {
            cmd.arg("--proxy").arg(&proxy);
        }
    if let Ok(ua) = env::var("YOUTIDY_USER_AGENT")
        && !ua.trim().is_empty() {
            cmd.arg("--user-agent").arg(&ua);
        }

    cmd.arg("-o")
       .arg(format!("{}/temp_download_{}.%(ext)s", cache_dir, video.id))
       .arg(&video.url);

    let output = cmd.output().await?;

    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp download failed: {}", err_str).into());
    }
    let _ = tx.send(AppEvent::ProcessingLog("[>] Download complete.".to_string())).await;

    // Locate the downloaded file
    let mut downloaded_path = None;
    let expected_prefix = format!("temp_download_{}", video.id);
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(file_stem) = path.file_stem() {
                    if file_stem == expected_prefix.as_str() {
                        downloaded_path = Some(path.clone());
                        break;
                    }
                }
            }
        }
    }

    let downloaded_path = match downloaded_path {
        Some(path) => path,
        None => return Err("Downloaded temporary file not found".into()),
    };

    let downloaded_ext = downloaded_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Downloaded format: {}", downloaded_ext))).await;

    let skip_transcode = (format == "m4a" && downloaded_ext == "m4a") || (format == "opus" && downloaded_ext == "opus");
    let stream_copy_opus = format == "opus" && downloaded_ext == "webm";

    if skip_transcode {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Formats match. Skipping transcoding (instant copy).".to_string())).await;
        std::fs::rename(&downloaded_path, &temp_path)?;
    } else if stream_copy_opus {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Extracting Opus audio without transcoding (instant copy).".to_string())).await;
        let mut ffmpeg = tokio::process::Command::new("ffmpeg");
        ffmpeg.arg("-y")
              .arg("-i").arg(&downloaded_path)
              .arg("-vn")
              .arg("-c:a").arg("copy")
              .arg(&temp_filename);
        let out = ffmpeg.output().await?;
        if !out.status.success() {
            let err_str = String::from_utf8_lossy(&out.stderr);
            return Err(format!("ffmpeg stream copy failed: {}", err_str).into());
        }
        let _ = std::fs::remove_file(&downloaded_path);
    } else {
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Transcoding audio to {}...", format))).await;
        let mut ffmpeg = tokio::process::Command::new("ffmpeg");
        ffmpeg.arg("-y")
              .arg("-i").arg(&downloaded_path)
              .arg("-vn");

        match format.as_str() {
            "mp3" => {
                ffmpeg.arg("-c:a").arg("libmp3lame");
                if let Ok(bitrate) = env::var("YOUTIDY_BITRATE")
                    && !bitrate.trim().is_empty() {
                        let cleaned = bitrate.trim().to_uppercase();
                        if cleaned.chars().all(|c| c.is_ascii_digit()) {
                            ffmpeg.arg("-b:a").arg(format!("{}K", cleaned));
                        } else {
                            ffmpeg.arg("-b:a").arg(&bitrate);
                        }
                    } else if let Ok(quality) = env::var("YOUTIDY_AUDIO_QUALITY")
                        && !quality.trim().is_empty() {
                            ffmpeg.arg("-q:a").arg(&quality);
                        } else {
                            ffmpeg.arg("-q:a").arg("2");
                        }
            }
            "m4a" => {
                ffmpeg.arg("-c:a").arg("aac");
                if let Ok(bitrate) = env::var("YOUTIDY_BITRATE")
                    && !bitrate.trim().is_empty() {
                        ffmpeg.arg("-b:a").arg(&bitrate);
                    } else {
                        ffmpeg.arg("-b:a").arg("192k");
                    }
            }
            "opus" => {
                ffmpeg.arg("-c:a").arg("libopus");
                if let Ok(bitrate) = env::var("YOUTIDY_BITRATE")
                    && !bitrate.trim().is_empty() {
                        ffmpeg.arg("-b:a").arg(&bitrate);
                    } else {
                        ffmpeg.arg("-b:a").arg("160k");
                    }
            }
            "flac" => {
                ffmpeg.arg("-c:a").arg("flac");
            }
            _ => {}
        }
        ffmpeg.arg(&temp_filename);
        let out = ffmpeg.output().await?;
        if !out.status.success() {
            let err_str = String::from_utf8_lossy(&out.stderr);
            return Err(format!("ffmpeg transcoding failed: {}", err_str).into());
        }
        let _ = std::fs::remove_file(&downloaded_path);
    }
    let _ = tx.send(AppEvent::ProcessingLog("[>] Post-processing complete.".to_string())).await;

    // 2. fpcalc Fingerprint
    let _ = tx.send(AppEvent::ProcessingLog("[>] Calculating fingerprint...".to_string())).await;
    let output = tokio::process::Command::new("fpcalc")
        .arg("-json")
        .arg("-length")
        .arg("120")
        .arg(&temp_filename)
        .output()
        .await?;

    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("fpcalc failed: {}", err_str).into());
    }

    let fpcalc_res: FpcalcOutput = serde_json::from_slice(&output.stdout)?;
    let duration = fpcalc_res.duration;
    let fingerprint = fpcalc_res.fingerprint;
    let _ = tx.send(AppEvent::ProcessingLog(format!(
        "[>] Fingerprint calculated (duration: {:.1}s).",
        duration
    ))).await;

    // 3. AcoustID Lookup
    if let Some(limiter) = &api_limiter {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Waiting for API rate limit slot...".to_string())).await;
        limiter.acquire().await;
    }
    let _ = tx.send(AppEvent::ProcessingLog("[>] Querying AcoustID API...".to_string())).await;
    let client_id = env::var("ACOUSTID_CLIENT_ID")
        .map_err(|_| "ACOUSTID_CLIENT_ID environment variable not set")?;

    let client = build_http_client()?;
    let response = client
        .post("https://api.acoustid.org/v2/lookup")
        .form(&[
            ("client", client_id.as_str()),
            ("duration", &format!("{:.0}", duration)),
            ("fingerprint", &fingerprint),
            ("meta", "recordings"),
            ("format", "json"),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        let _ = tx.send(AppEvent::ProcessingLog(format!(
            "[!] AcoustID lookup status error: {}. Falling back.",
            response.status()
        ))).await;
    }

    let mut recording_mbid = None;
    if response.status().is_success()
        && let Ok(acoustid_res) = response.json::<AcoustIdResponse>().await
            && acoustid_res.status == "ok" {
                for result in acoustid_res.results {
                    if let Some(recordings) = result.recordings
                        && let Some(first_rec) = recordings.first() {
                            recording_mbid = Some(first_rec.id.clone());
                            break;
                        }
                }
            }

    // 4. MusicBrainz Query & Heuristic Filter
    let mut resolved_metadata = None;
    if let Some(mbid) = recording_mbid {
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] AcoustID match found (MBID: {}).", mbid))).await;
        match query_musicbrainz(&mbid, tx.clone()).await {
            Ok(Some((art, alb, tit, rel_mbid))) => {
                resolved_metadata = Some((art, alb, tit, Some(rel_mbid), None));
            }
            Ok(None) => {
                let _ = tx.send(AppEvent::ProcessingLog("[!] MusicBrainz query returned no releases matching heuristic. Trying Spotify fallback...".to_string())).await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::ProcessingLog(format!("[!] MusicBrainz query failed: {}. Trying Spotify fallback...", e))).await;
            }
        }
    } else {
        let _ = tx.send(AppEvent::ProcessingLog("[!] No AcoustID match found. Trying Spotify fallback...".to_string())).await;
    }

    let (artist, album, title, mbid, spotify_cover_url) = if let Some(meta) = resolved_metadata {
        meta
    } else {
        let clean_title = clean_youtube_title(&video.title);
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Querying Spotify fallback for \"{}\"...", clean_title))).await;
        match search_spotify_fallback(&clean_title).await {
            Some(spotify_meta) => {
                let _ = tx.send(AppEvent::ProcessingLog(format!(
                    "[+] Spotify match found: \"{}\" - \"{}\" (Album: \"{}\").",
                    spotify_meta.artist, spotify_meta.title, spotify_meta.album
                ))).await;
                (spotify_meta.artist, spotify_meta.album, spotify_meta.title, None, spotify_meta.cover_url)
            }
            None => {
                let _ = tx.send(AppEvent::ProcessingLog("[!] Spotify fallback failed or returned no matches. Falling back to Unknown.".to_string())).await;
                ("Unknown Artist".to_string(), "Unknown Album".to_string(), clean_title, None, None)
            }
        }
    };

    // Download album art/thumbnail
    let _ = tx.send(AppEvent::ProcessingLog("[>] Fetching album art...".to_string())).await;
    let mut art_data = None;

    if let Some(ref release_mbid) = mbid {
        let cover_url = format!("https://coverartarchive.org/release/{}/front", release_mbid);
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Trying Cover Art Archive for release MBID: {}...", release_mbid))).await;
        match download_url(&client, &cover_url).await {
            Ok((bytes, mime)) => {
                let _ = tx.send(AppEvent::ProcessingLog("[+] Album art fetched from Cover Art Archive.".to_string())).await;
                art_data = Some((bytes, mime));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Cover Art Archive failed: {}. Falling back...", e))).await;
            }
        }
    }

    if art_data.is_none()
        && let Some(ref cover_url) = spotify_cover_url {
            let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Fetching Spotify album art from URL: {}...", cover_url))).await;
            match download_url(&client, cover_url).await {
                Ok((bytes, mime)) => {
                    let _ = tx.send(AppEvent::ProcessingLog("[+] Album art fetched from Spotify.".to_string())).await;
                    art_data = Some((bytes, mime));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Spotify cover fetch failed: {}. Falling back to YouTube thumbnail...", e))).await;
                }
            }
        }

    if art_data.is_none() {
        // Fallback to YouTube thumbnail
        let maxres_url = format!("https://img.youtube.com/vi/{}/maxresdefault.jpg", video.id);
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Fetching YouTube maxres thumbnail for video ID: {}...", video.id))).await;
        match download_url(&client, &maxres_url).await {
            Ok((bytes, mime)) => {
                let _ = tx.send(AppEvent::ProcessingLog("[+] YouTube maxres thumbnail fetched.".to_string())).await;
                art_data = Some((bytes, mime));
            }
            Err(_) => {
                let hq_url = format!("https://img.youtube.com/vi/{}/hqdefault.jpg", video.id);
                let _ = tx.send(AppEvent::ProcessingLog("[>] Maxres thumbnail failed. Trying YouTube hqdefault thumbnail...".to_string())).await;
                match download_url(&client, &hq_url).await {
                    Ok((bytes, mime)) => {
                        let _ = tx.send(AppEvent::ProcessingLog("[+] YouTube hqdefault thumbnail fetched.".to_string())).await;
                        art_data = Some((bytes, mime));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::ProcessingLog(format!("[!] YouTube thumbnail failed: {}.", e))).await;
                    }
                }
            }
        }
    }

    // Compute destination folder and filename
    let clean_artist = sanitize_path_segment(&artist);
    let clean_album = sanitize_path_segment(&album);
    let clean_title = sanitize_path_segment(&title);

    let folder_structure = env::var("YOUTIDY_FOLDER_STRUCTURE")
        .unwrap_or_else(|_| "{artist}/{album}".to_string());
    let filename_template = env::var("YOUTIDY_FILENAME_TEMPLATE")
        .unwrap_or_else(|_| "{title}".to_string());

    let folder_relative = folder_structure
        .replace("{artist}", &clean_artist)
        .replace("{album}", &clean_album)
        .replace("{title}", &clean_title);

    let filename = filename_template
        .replace("{artist}", &clean_artist)
        .replace("{album}", &clean_album)
        .replace("{title}", &clean_title);

    let base_music_dir = expand_tilde(&env::var("YOUTIDY_MUSIC_DIR").unwrap_or_else(|_| "Music".to_string()));
    let dest_dir = format!("{}/{}", base_music_dir, folder_relative);
    std::fs::create_dir_all(&dest_dir)?;

    let save_cover = env::var("YOUTIDY_SAVE_COVER_FILE")
        .map(|v| v.parse::<bool>().unwrap_or(true))
        .unwrap_or(true);

    if save_cover {
        if let Some((ref bytes, ref mime)) = art_data {
            let ext = mime.ext().unwrap_or("jpg");
            let cover_path = format!("{}/cover.{}", dest_dir, ext);
            if let Err(e) = std::fs::write(&cover_path, bytes) {
                let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Warning: failed to save cover art file: {}", e))).await;
            } else {
                let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Saved cover art to: {}", cover_path))).await;
            }
        }
    } else {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Skipping saving standalone cover art file (embed only).".to_string())).await;
    }

    let dest_file = format!("{}/{}.{}", dest_dir, filename, format);

    // 5. Fetch and save lyrics
    let mut lyrics_content = None;
    if let Err(e) = lyrics::fetch_and_save_lyrics(
        &client,
        &artist,
        &title,
        &album,
        duration,
        &dest_dir,
        &filename,
        &video.title,
        tx.clone(),
    ).await {
        let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Lyrics lookup skipped/failed: {}", e))).await;
    } else {
        let embed_lyrics = env::var("YOUTIDY_EMBED_LYRICS")
            .map(|v| v.parse::<bool>().unwrap_or(false))
            .unwrap_or(false);
        if embed_lyrics {
            let lrc_path = format!("{}/{}.lrc", dest_dir, filename);
            if let Ok(lrc) = std::fs::read_to_string(&lrc_path) {
                lyrics_content = Some(lrc);
            }
        }
    }

    // 6. Tag with lofty (ID3v2)
    let _ = tx.send(AppEvent::ProcessingLog(format!(
        "[>] Writing tags: Artist='{}', Album='{}', Title='{}'...",
        artist, album, title
    ))).await;

    let mut tagged_file = lofty::read_from_path(&temp_filename)?;
    
    // Get primary tag mutably or create one
    let tag = match tagged_file.primary_tag_mut() {
        Some(t) => t,
        None => {
            let new_tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
            tagged_file.insert_tag(new_tag);
            tagged_file.primary_tag_mut().unwrap()
        }
    };
    
    tag.set_title(title.clone());
    tag.set_artist(artist.clone());
    tag.set_album(album.clone());

    if let Some(lrc) = lyrics_content {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Embedding lyrics into file tags...".to_string())).await;
        tag.insert_text(lofty::tag::ItemKey::Lyrics, lrc);
    }

    if let Some((ref bytes, ref mime)) = art_data {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Embedding album art into file tags...".to_string())).await;
        tag.remove_picture_type(PictureType::CoverFront);
        let picture = Picture::unchecked(bytes.clone())
            .pic_type(PictureType::CoverFront)
            .mime_type(mime.clone())
            .build();
        tag.push_picture(picture);
    }
    
    tagged_file.save_to_path(&temp_filename, lofty::config::WriteOptions::default())?;
    let _ = tx.send(AppEvent::ProcessingLog("[>] Tagging complete.".to_string())).await;

    // 7. Move to final destination
    std::fs::copy(&temp_filename, &dest_file)?;
    std::fs::remove_file(&temp_filename)?;
    
    let _ = tx.send(AppEvent::ProcessingLog(format!("[>] File saved to destination: {}", dest_file))).await;

    // Auto add to mpc queue if enabled
    if env::var("YOUTIDY_MPC_ADD").map(|v| v.parse::<bool>().unwrap_or(false)).unwrap_or(false) {
        let relative_path = if folder_relative.trim().is_empty() {
            format!("{}.{}", filename, format)
        } else {
            format!("{}/{}.{}", folder_relative.trim_matches('/'), filename, format)
        };
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Adding to mpc queue: {}", relative_path))).await;
        
        let update_status = tokio::process::Command::new("mpc")
            .arg("update")
            .arg("--wait")
            .arg(&relative_path)
            .status()
            .await;
            
        match update_status {
            Ok(status) if status.success() => {
                let add_status = tokio::process::Command::new("mpc")
                    .arg("add")
                    .arg(&relative_path)
                    .status()
                    .await;
                    
                match add_status {
                    Ok(status) if status.success() => {
                        let _ = tx.send(AppEvent::ProcessingLog("[>] Successfully added to mpc queue.".to_string())).await;
                    }
                    Ok(status) => {
                        let _ = tx.send(AppEvent::ProcessingLog(format!("[!] mpc add failed with exit code: {:?}", status.code()))).await;
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Failed to run mpc add: {}", e))).await;
                    }
                }
            }
            Ok(status) => {
                let _ = tx.send(AppEvent::ProcessingLog(format!("[!] mpc update --wait failed, trying direct add: {:?}", status.code()))).await;
                let _ = tokio::process::Command::new("mpc")
                    .arg("add")
                    .arg(&relative_path)
                    .status()
                    .await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Failed to run mpc update: {}", e))).await;
            }
        }
    }

    // Save cache record
    let record = CacheRecord {
        dest_path: dest_file.clone(),
    };
    if let Ok(serialized) = serde_json::to_string(&record) {
        let cache_record_path = format!("{}/{}.json", cache_dir, video.id);
        if let Err(e) = std::fs::write(&cache_record_path, serialized) {
            let _ = tx.send(AppEvent::ProcessingLog(format!("[!] Warning: failed to save cache record: {}", e))).await;
        }
    }

    Ok(dest_file)
}

#[derive(Debug, Deserialize)]
struct SpotifyMetadata {
    artist: String,
    album: String,
    title: String,
    cover_url: Option<String>,
    #[allow(dead_code)]
    release_date: Option<String>,
}

async fn search_spotify_fallback(query: &str) -> Option<SpotifyMetadata> {
    let output = tokio::process::Command::new(".venv/bin/python3")
        .arg("src/spotify_fallback.py")
        .arg(query)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    if output.status.success() {
        serde_json::from_slice(&output.stdout).ok()
    } else {
        None
    }
}

async fn query_musicbrainz(
    recording_mbid: &str,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) -> Result<Option<(String, String, String, String)>, Box<dyn Error + Send + Sync>> {
    // Respect the 1 request/second rate limit
    let _ = tx.send(AppEvent::ProcessingLog("[>] Waiting for MusicBrainz rate limit (1.1s)...".to_string())).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Querying MusicBrainz for MBID: {}...", recording_mbid))).await;

    let client = build_http_client()?;

    let url = format!(
        "https://musicbrainz.org/ws/2/recording/{}?inc=releases+release-groups+artists&fmt=json",
        recording_mbid
    );

    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(format!("MusicBrainz API status: {}", response.status()).into());
    }

    let recording: MusicBrainzRecording = response.json().await?;
    let artist_name = if let Some(credits) = &recording.artist_credit {
        format_artist_credit(credits)
    } else {
        "Unknown Artist".to_string()
    };

    let mut candidates = Vec::new();
    if let Some(releases) = recording.releases {
        for release in releases {
            if let Some(rg) = &release.release_group {
                // Heuristic: primary-type must be "Album" or "Single"
                let primary_ok = if let Some(p_type) = &rg.primary_type {
                    let p_lower = p_type.to_lowercase();
                    p_lower == "album" || p_lower == "single"
                } else {
                    false
                };

                if !primary_ok {
                    continue;
                }

                // Heuristic: secondary-types must NOT contain Compilation, Soundtrack, Live, Remix
                let mut secondary_ok = true;
                for s_type in &rg.secondary_types {
                    let s_lower = s_type.to_lowercase();
                    if s_lower == "compilation"
                        || s_lower == "soundtrack"
                        || s_lower == "live"
                        || s_lower == "remix"
                    {
                        secondary_ok = false;
                        break;
                    }
                }

                if !secondary_ok {
                    continue;
                }

                candidates.push((release.clone(), rg.clone()));
            }
        }
    }

    if candidates.is_empty() {
        return Ok(None);
    }

    // Sort by oldest first-release-date
    candidates.sort_by(|a, b| {
        let date_a = a.1.first_release_date.as_deref().unwrap_or("9999-12-31");
        let date_b = b.1.first_release_date.as_deref().unwrap_or("9999-12-31");
        
        let key_a = parse_date_to_sort_key(date_a);
        let key_b = parse_date_to_sort_key(date_b);
        
        key_a.cmp(&key_b)
    });

    let (best_release, best_rg) = &candidates[0];
    let album_name = best_release.title.clone();
    let title = recording.title.clone();

    let _ = tx.send(AppEvent::ProcessingLog(format!(
        "[>] Best match: Artist='{}', Album='{}', Title='{}', Date='{}'",
        artist_name,
        album_name,
        title,
        best_rg.first_release_date.as_deref().unwrap_or("Unknown")
    ))).await;

    Ok(Some((artist_name, album_name, title, best_release.id.clone())))
}

async fn download_url(
    client: &reqwest::Client,
    url: &str,
) -> Result<(Vec<u8>, MimeType), Box<dyn Error + Send + Sync>> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(format!("Failed to download {}, status: {}", url, response.status()).into());
    }
    let mime_str = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg");
    let mime = MimeType::from_str(mime_str);
    let bytes = response.bytes().await?.to_vec();
    Ok((bytes, mime))
}

fn format_artist_credit(credits: &[ArtistCredit]) -> String {
    if let Some(first_credit) = credits.first() {
        first_credit.name.trim().to_string()
    } else {
        "Unknown Artist".to_string()
    }
}

fn parse_date_to_sort_key(date_str: &str) -> String {
    let parts: Vec<&str> = date_str.split('-').collect();
    match parts.len() {
        1 if parts[0].len() == 4 => format!("{}-01-01", parts[0]),
        2 if parts[0].len() == 4 && parts[1].len() == 2 => format!("{}-{}-01", parts[0], parts[1]),
        _ => date_str.to_string(),
    }
}

fn sanitize_path_segment(s: &str) -> String {
    let clean = s.replace(['/', '\\'], "");
    let clean = clean.trim();
    if clean.is_empty() {
        "Unknown".to_string()
    } else {
        clean.to_string()
    }
}

fn clean_youtube_title(title: &str) -> String {
    let mut clean = title.to_string();
    
    let suffixes = [
        "(official video)",
        "[official video]",
        "(official audio)",
        "[official audio]",
        "(official music video)",
        "[official music video]",
        "(lyric video)",
        "(lyrics)",
        "[lyrics]",
        "(hd)",
        "(hq)",
    ];
    
    let mut lower = clean.to_lowercase();
    for suffix in suffixes {
        if let Some(pos) = lower.find(suffix) {
            clean.drain(pos..(pos + suffix.len()));
            lower = clean.to_lowercase();
        }
    }
    
    clean = clean.replace("  ", " ").trim().to_string();
    if clean.is_empty() {
        title.to_string()
    } else {
        clean
    }
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home_dir) = env::var("HOME") {
            return path.replacen('~', &home_dir, 1);
        }
    } else if path == "~" {
        if let Ok(home_dir) = env::var("HOME") {
            return home_dir;
        }
    }
    path.to_string()
}

fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

fn get_default_cache_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
            let mut path = std::path::PathBuf::from(local_appdata);
            path.push("youtidy");
            path.push("cache");
            if let Some(s) = path.to_str() {
                return s.to_string();
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            let mut path = std::path::PathBuf::from(home);
            path.push("Library");
            path.push("Caches");
            path.push("youtidy");
            if let Some(s) = path.to_str() {
                return s.to_string();
            }
        }
    }

    // Default for Linux and Unix-like systems
    if let Ok(xdg_cache) = env::var("XDG_CACHE_HOME") {
        if !xdg_cache.trim().is_empty() {
            let mut path = std::path::PathBuf::from(xdg_cache);
            path.push("youtidy");
            if let Some(s) = path.to_str() {
                return s.to_string();
            }
        }
    }

    if let Ok(home) = env::var("HOME") {
        let mut path = std::path::PathBuf::from(home);
        path.push(".cache");
        path.push("youtidy");
        if let Some(s) = path.to_str() {
            return s.to_string();
        }
    }

    "cache".to_string()
}

