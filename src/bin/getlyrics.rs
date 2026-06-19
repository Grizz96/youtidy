use std::env;
use std::error::Error;
use tokio::sync::mpsc;

#[path = "../lyrics.rs"]
mod lyrics;

#[derive(Debug, Clone)]
pub enum AppEvent {
    ProcessingLog(String),
}

fn load_config() {
    let mut config_path = std::path::PathBuf::from("config.toml");
    if !config_path.exists()
        && let Ok(home_dir) = env::var("HOME") {
            let mut home = std::path::PathBuf::from(home_dir);
            home.push(".config");
            home.push("youtidy");
            home.push("config.toml");
            config_path = home;
        }

    if config_path.exists()
        && let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(config) = content.parse::<toml::Table>()
                && let Some(network) = config.get("network").and_then(|v| v.as_table()) {
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

fn build_http_client() -> Result<reqwest::Client, Box<dyn Error>> {
    let mut builder = reqwest::Client::builder();
    
    let user_agent = env::var("YOUTIDY_USER_AGENT")
        .unwrap_or_else(|_| "youtidy-lyrics-extractor/1.0.0".to_string());
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load .env file if present
    dotenvy::dotenv().ok();
    load_config();

    let args: Vec<String> = env::args().collect();
    if args.len() < 6 {
        eprintln!("Usage: getlyrics <artist> <title> <album> <duration_seconds> <dest_dir> [clean_title]");
        std::process::exit(1);
    }

    let artist = &args[1];
    let title = &args[2];
    let album = &args[3];
    
    let duration: f64 = args[4].parse().unwrap_or_else(|_| {
        eprintln!("Warning: Failed to parse duration '{}', using 0.0", args[4]);
        0.0
    });
    
    let dest_dir = &args[5];
    
    let clean_title = if args.len() > 6 {
        args[6].clone()
    } else {
        title.clone()
    };

    let client = build_http_client()?;

    let (tx, mut rx) = mpsc::channel(100);

    // Spawn logging receiver to output progress to stdout
    let log_handler = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                AppEvent::ProcessingLog(msg) => {
                    println!("{}", msg);
                }
            }
        }
    });

    println!("[>] Starting standalone lyrics retrieval for: {} - {}", artist, title);
    
    match lyrics::fetch_and_save_lyrics(
        &client,
        artist,
        title,
        album,
        duration,
        dest_dir,
        &clean_title,
        tx,
    ).await {
        Ok(_) => {
            println!("[+] Successfully saved lyrics to: {}/{}.lrc", dest_dir, clean_title);
        }
        Err(e) => {
            eprintln!("[!] Error fetching lyrics: {}", e);
            std::process::exit(1);
        }
    }

    // Wait for logger to finish flushing logs
    let _ = log_handler.await;

    Ok(())
}
