use std::env;
use std::error::Error;
use tokio::sync::mpsc;

#[path = "../lyrics.rs"]
mod lyrics;

#[derive(Debug, Clone)]
pub enum AppEvent {
    ProcessingLog(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load .env file if present
    dotenvy::dotenv().ok();

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

    let client = reqwest::Client::builder()
        .user_agent("youtidy-lyrics-extractor/1.0.0")
        .build()?;

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
