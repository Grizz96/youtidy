use crate::AppEvent;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibResponse {
    duration: Option<f64>,
    synced_lyrics: Option<String>,
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|c| matches!(c as u32, 0x3040..=0x309F | 0x30A0..=0x30FF | 0x4E00..=0x9FFF))
}

fn romanize_text_kakasi(text: &str) -> String {
    kakasi::convert(text).romaji
}

struct PythonRomanizer {
    _child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
}

impl PythonRomanizer {
    async fn new() -> Option<Self> {
        let mut child = Command::new(".venv/bin/python3")
            .arg("src/romanize.py")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .ok()?;
            
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let reader = BufReader::new(stdout);
        
        Some(Self {
            _child: child,
            stdin,
            reader,
        })
    }
    
    async fn romanize(&mut self, text: &str) -> Option<String> {
        let clean_text = text.replace('\n', " ").replace('\r', "");
        let line = format!("{}\n", clean_text);
        
        self.stdin.write_all(line.as_bytes()).await.ok()?;
        self.stdin.flush().await.ok()?;
        
        let mut response = String::new();
        self.reader.read_line(&mut response).await.ok()?;
        
        let trimmed = response.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

async fn normalize_text(text: &str, romanizer: &mut Option<PythonRomanizer>) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut normalized = if contains_japanese(text) {
        let mut romaji = None;
        if let Some(r) = romanizer {
            romaji = r.romanize(text).await;
        }
        romaji.unwrap_or_else(|| romanize_text_kakasi(text))
    } else {
        text.to_string()
    };
    normalized = normalized.to_lowercase();
    let mut cleaned = String::new();
    for c in normalized.chars() {
        if c.is_alphanumeric() || c.is_whitespace() {
            cleaned.push(c);
        }
    }
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Aligner {
    reference_lines: Vec<String>,
    reference_norms: Vec<String>,
}

impl Aligner {
    fn new(reference_lines: Vec<String>, reference_norms: Vec<String>) -> Self {
        Self {
            reference_lines,
            reference_norms,
        }
    }
    
    fn find_best_match(&self, lrc_norm: &str, current_idx: usize) -> (Option<String>, usize) {
        if lrc_norm.is_empty() {
            return (None, current_idx);
        }
        
        let mut best_match = None;
        let mut best_idx = current_idx;
        let mut max_score = -1.0;
        
        let current_i = current_idx as i32;
        let search_start = std::cmp::max(0, current_i - 1) as usize;
        let search_end = std::cmp::min(self.reference_lines.len(), (current_i + 8) as usize);
        
        for i in search_start..search_end {
            let g_line = &self.reference_lines[i];
            let g_norm = &self.reference_norms[i];
            
            // Look-ahead double-line merge match
            if i + 1 < self.reference_lines.len() {
                let combined_line = format!("{} {}", g_line, self.reference_lines[i + 1]);
                let combined_norm = format!("{} {}", g_norm, self.reference_norms[i + 1]);
                let mut combined_score = strsim::sorensen_dice(lrc_norm, &combined_norm);
                
                if combined_norm.contains(lrc_norm) || lrc_norm.contains(&combined_norm) {
                    let len_ratio = (std::cmp::min(lrc_norm.len(), combined_norm.len()) as f64)
                        / (std::cmp::max(lrc_norm.len(), combined_norm.len()) as f64);
                    combined_score = combined_score.max(0.5 + (0.5 * len_ratio));
                }
                
                if combined_score > 0.85 {
                    let dist_penalty = ((i as i32 - current_i).abs() as f64) * 0.05;
                    if (combined_score - dist_penalty) > max_score {
                        max_score = combined_score - dist_penalty;
                        best_match = Some(combined_line);
                        best_idx = i + 1;
                        continue;
                    }
                }
            }
            
            // Single line match
            let mut score = strsim::sorensen_dice(lrc_norm, g_norm);
            
            if g_norm.contains(lrc_norm) || lrc_norm.contains(g_norm) {
                let len_ratio = (std::cmp::min(lrc_norm.len(), g_norm.len()) as f64)
                    / (std::cmp::max(lrc_norm.len(), g_norm.len()) as f64);
                score = score.max(0.5 + (0.5 * len_ratio));
            }
            
            let dist_penalty = ((i as i32 - current_i).abs() as f64) * 0.05;
            let effective_score = score - dist_penalty;
            
            if effective_score > max_score {
                max_score = effective_score;
                best_match = Some(g_line.clone());
                best_idx = i;
            }
        }
        
        if max_score > 0.55 {
            (best_match, best_idx)
        } else {
            (None, current_idx)
        }
    }
}

fn adjust_timestamp(ts_str: &str, offset: f64) -> String {
    let re = regex::Regex::new(r"^\[(\d+):(\d+\.?\d*)\]").unwrap();
    if let Some(caps) = re.captures(ts_str) {
        let m = caps[1].parse::<f64>().unwrap_or(0.0);
        let s = caps[2].parse::<f64>().unwrap_or(0.0);
        let mut total = m * 60.0 + s + offset;
        if total < 0.0 {
            total = 0.0;
        }
        let minutes = (total / 60.0).floor() as u32;
        let seconds = total % 60.0;
        format!("[{:02}:{:05.2}]", minutes, seconds)
    } else {
        ts_str.to_string()
    }
}

fn shift_lrc_timestamps(lrc: &str, offset: f64) -> String {
    let mut output = String::new();
    let re_timestamp = regex::Regex::new(r"^(\[[0-9:.]+\])(.*)").unwrap();
    
    for line in lrc.lines() {
        if let Some(caps) = re_timestamp.captures(line) {
            let timestamp = &caps[1];
            let text = &caps[2];
            let new_timestamp = adjust_timestamp(timestamp, offset);
            output.push_str(&format!("{} {}\n", new_timestamp, text.trim()));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

async fn get_lrclib_synced_lyrics(
    client: &reqwest::Client,
    artist: &str,
    track: &str,
    album: &str,
    duration: f64,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) -> Result<Option<(String, f64)>, Box<dyn std::error::Error + Send + Sync>> {
    let duration_str = format!("{:.0}", duration);
    
    // 1. Direct GET
    let mut url = reqwest::Url::parse("https://lrclib.net/api/get")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("artist_name", artist)
             .append_pair("track_name", track)
             .append_pair("album_name", album)
             .append_pair("duration", &duration_str);
    }
    
    let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Querying LRCLIB for: {} - {}", artist, track))).await;
    
    let response = client.get(url.clone())
        .header("Lrclib-Client", "youtidy/1.0.0")
        .send()
        .await?;
        
    if response.status().is_success()
        && let Ok(res) = response.json::<LrclibResponse>().await
            && let Some(synced) = res.synced_lyrics {
                let lrc_dur = res.duration.unwrap_or(duration);
                return Ok(Some((synced, lrc_dur)));
            }
    
    // 2. Swapped GET without album name
    let mut url_no_album = reqwest::Url::parse("https://lrclib.net/api/get")?;
    {
        let mut query = url_no_album.query_pairs_mut();
        query.append_pair("artist_name", artist)
             .append_pair("track_name", track)
             .append_pair("duration", &duration_str);
    }
    let response = client.get(url_no_album)
        .header("Lrclib-Client", "youtidy/1.0.0")
        .send()
        .await?;
        
    if response.status().is_success()
        && let Ok(res) = response.json::<LrclibResponse>().await
            && let Some(synced) = res.synced_lyrics {
                let lrc_dur = res.duration.unwrap_or(duration);
                return Ok(Some((synced, lrc_dur)));
            }
    
    // 3. Search Fallback
    let mut search_url = reqwest::Url::parse("https://lrclib.net/api/search")?;
    {
        let search_query = format!("{} {}", artist, track);
        search_url.query_pairs_mut().append_pair("q", &search_query);
    }
    
    let response = client.get(search_url)
        .header("Lrclib-Client", "youtidy/1.0.0")
        .send()
        .await?;
        
    if response.status().is_success()
        && let Ok(results) = response.json::<Vec<LrclibResponse>>().await {
            let mut candidates: Vec<LrclibResponse> = results.into_iter()
                .filter(|r| r.synced_lyrics.is_some() && r.duration.is_some())
                .collect();
                
            if !candidates.is_empty() {
                candidates.sort_by(|a, b| {
                    let diff_a = (a.duration.unwrap() - duration).abs();
                    let diff_b = (b.duration.unwrap() - duration).abs();
                    diff_a.partial_cmp(&diff_b).unwrap_or(std::cmp::Ordering::Equal)
                });
                
                let best = &candidates[0];
                return Ok(Some((best.synced_lyrics.clone().unwrap(), best.duration.unwrap())));
            }
        }
    
    Ok(None)
}

async fn query_genius_api(
    client: &reqwest::Client,
    query: &str,
    token: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut api_url = reqwest::Url::parse("https://api.genius.com/search")?;
    api_url.query_pairs_mut().append_pair("q", query);
    
    let response = client.get(api_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;
        
    if response.status().is_success() {
        let json: serde_json::Value = response.json().await?;
        if let Some(hits) = json["response"]["hits"].as_array()
            && !hits.is_empty()
                && let Some(url) = hits[0]["result"]["url"].as_str() {
                    return Ok(Some(url.to_string()));
                }
    }
    Ok(None)
}

fn clean_html_lyrics(html: &str) -> String {
    let mut lyrics = String::new();
    let re_container = regex::Regex::new(r#"(?s)<div[^>]*data-lyrics-container="true"[^>]*>(.*?)</div>"#).unwrap();
    let re_tags = regex::Regex::new(r"<[^>]*>").unwrap();
    
    for cap in re_container.captures_iter(html) {
        let mut content = cap[1].to_string();
        
        content = content.replace("<br/>", "\n")
                         .replace("<br>", "\n")
                         .replace("<br />", "\n");
                          
        content = content.replace("</p>", "\n");
        
        content = re_tags.replace_all(&content, "").to_string();
        
        content = content.replace("&amp;", "&")
                         .replace("&quot;", "\"")
                         .replace("&#x27;", "'")
                         .replace("&apos;", "'")
                         .replace("&nbsp;", " ")
                         .replace("&lt;", "<")
                         .replace("&gt;", ">");
                         
        lyrics.push_str(&content);
        lyrics.push('\n');
    }
    
    lyrics.trim().to_string()
}

async fn fetch_genius_lyrics(
    client: &reqwest::Client,
    artist: &str,
    track: &str,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let token = match std::env::var("GENIUS_ACCESS_TOKEN") {
        Ok(t) if !t.trim().is_empty() => t,
        _ => {
            let _ = tx.send(AppEvent::ProcessingLog("[!] GENIUS_ACCESS_TOKEN not set in .env. Skipping Rust Genius search.".to_string())).await;
            return Ok(None);
        }
    };
    let query_romanized = format!("{} {} Romanized", artist, track);
    let mut url = None;
    
    let _ = tx.send(AppEvent::ProcessingLog("[>] Searching Genius (Romanized)...".to_string())).await;
    if let Some(found_url) = query_genius_api(client, &query_romanized, &token).await? {
        url = Some(found_url);
    } else {
        let query_standard = format!("{} {}", artist, track);
        let _ = tx.send(AppEvent::ProcessingLog("[>] Romanized not found. Searching Genius (Standard)...".to_string())).await;
        if let Some(found_url) = query_genius_api(client, &query_standard, &token).await? {
            url = Some(found_url);
        }
    }
    
    if let Some(lyrics_url) = url {
        let _ = tx.send(AppEvent::ProcessingLog(format!("[>] Scraping lyrics from Genius: {}...", lyrics_url))).await;
        let response = client.get(&lyrics_url).send().await?;
        if response.status().is_success() {
            let html = response.text().await?;
            let lyrics = clean_html_lyrics(&html);
            if !lyrics.is_empty() {
                return Ok(Some(lyrics));
            }
        }
    }
    
    Ok(None)
}

async fn run_python_lyrics_worker(
    mode: &str,
    offset: f64,
    artist: &str,
    title: &str,
    stdin_content: Option<&str>,
) -> Option<String> {
    // 1. Resolve python3 executable
    let mut python_cmd = "python3".to_string();
    if std::path::Path::new(".venv/bin/python3").exists() {
        python_cmd = ".venv/bin/python3".to_string();
    } else if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let venv_py = exe_dir.join(".venv/bin/python3");
            if venv_py.exists() {
                python_cmd = venv_py.to_string_lossy().to_string();
            } else {
                let parent_venv = exe_dir.parent().map(|p| p.join(".venv/bin/python3"));
                if let Some(ref p) = parent_venv {
                    if p.exists() {
                        python_cmd = p.to_string_lossy().to_string();
                    }
                }
            }
        }
    }

    // 2. Resolve lyrics_worker.py script path
    let mut script_path = "src/lyrics_worker.py".to_string();
    if std::path::Path::new("src/lyrics_worker.py").exists() {
        // use default
    } else if std::path::Path::new("lyrics_worker.py").exists() {
        script_path = "lyrics_worker.py".to_string();
    } else if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let exe_script = exe_dir.join("lyrics_worker.py");
            if exe_script.exists() {
                script_path = exe_script.to_string_lossy().to_string();
            } else {
                let exe_src_script = exe_dir.join("src/lyrics_worker.py");
                if exe_src_script.exists() {
                    script_path = exe_src_script.to_string_lossy().to_string();
                }
            }
        }
    }

    let mut child = Command::new(python_cmd)
        .arg(script_path)
        .arg(mode)
        .arg(offset.to_string())
        .arg(artist)
        .arg(title)
        .stdin(if stdin_content.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    if let Some(content) = stdin_content
        && let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(content.as_bytes()).await;
            let _ = stdin.flush().await;
        }

    let output = child.wait_with_output().await.ok()?;
    if output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        if !stdout_str.trim().is_empty() {
            return Some(stdout_str);
        }
    }
    None
}

async fn romanize_lrc(
    synced_lrc: &str,
    artist: &str,
    track: &str,
    offset: f64,
    client: &reqwest::Client,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) -> String {
    let reference_lyrics = match fetch_genius_lyrics(client, artist, track, tx.clone()).await {
        Ok(Some(ref_lyr)) => Some(ref_lyr),
        _ => None,
    };
    
    let mut romanizer = PythonRomanizer::new().await;
    if romanizer.is_some() {
        let _ = tx.send(AppEvent::ProcessingLog("[>] Using Python cutlet romanizer worker...".to_string())).await;
    } else {
        let _ = tx.send(AppEvent::ProcessingLog("[!] Python romanizer worker not available. Using kakasi fallback...".to_string())).await;
    }

    let mut reference_lines = Vec::new();
    let mut reference_norms = Vec::new();
    
    if let Some(ref_lyr) = reference_lyrics {
        for line in ref_lyr.lines() {
            let line = line.trim();
            if !(line.is_empty() || line.starts_with('[') && line.ends_with(']')) {
                let cleaned_line = line.replace('\u{2005}', " ").replace('\u{200b}', "");
                let norm = normalize_text(&cleaned_line, &mut romanizer).await;
                reference_norms.push(norm);
                reference_lines.push(cleaned_line);
            }
        }
    }
    
    let aligner = Aligner::new(reference_lines, reference_norms);
    let mut current_ptr = 0;
    let mut output = String::new();
    
    let re_timestamp = regex::Regex::new(r"^(\[[0-9:.]+\])(.*)").unwrap();
    
    for line in synced_lrc.lines() {
        if let Some(caps) = re_timestamp.captures(line) {
            let timestamp = caps[1].to_string();
            let text = caps[2].trim().to_string();
            
            let new_timestamp = adjust_timestamp(&timestamp, offset);
            
            if text.is_empty() {
                output.push_str(&new_timestamp);
                output.push('\n');
                continue;
            }
            
            let norm_lrc = normalize_text(&text, &mut romanizer).await;
            let (best_match, found_idx) = aligner.find_best_match(&norm_lrc, current_ptr);
            
            if let Some(matched_text) = best_match {
                output.push_str(&format!("{} {}\n", new_timestamp, matched_text));
                current_ptr = found_idx + 1;
            } else {
                if contains_japanese(&text) {
                    let romaji = if let Some(ref mut r) = romanizer {
                        r.romanize(&text).await.unwrap_or_else(|| romanize_text_kakasi(&text))
                    } else {
                        romanize_text_kakasi(&text)
                    };
                    output.push_str(&format!("{} {}\n", new_timestamp, romaji));
                } else {
                    output.push_str(&format!("{} {}\n", new_timestamp, text));
                }
            }
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    
    output
}

fn clean_query_name(name: &str) -> String {
    let mut cleaned = name
        .replace('　', " ")
        .replace(['「', '」', '『', '』', '【', '】'], "\"");

    // Remove common metadata flags like (Official Video), (Music Video), [Lyrics], etc.
    if let Ok(re_brackets_meta) = regex::Regex::new(r"(?i)\s*[\(\[][^\]\)]*(official|lyrics?|lirik|video|music|audio|hd|4k|high quality|remastered|feat|ft|prod|opening|ending|theme|op|ed|ost|tv|full|special|movie|anime|eng|rom|kanji|mv|pv|karaoke|cover|テーマ)[^\]\)]*[\)\]]") {
        cleaned = re_brackets_meta.replace_all(&cleaned, "").to_string();
    }

    // Remove general brackets if any are left
    if let Ok(re_brackets_empty) = regex::Regex::new(r"\s*[\(\[（].*[\)\]）]") {
        cleaned = re_brackets_empty.replace_all(&cleaned, "").to_string();
    }

    // Remove isolated keywords
    if let Ok(re_keywords) = regex::Regex::new(r"(?i)\b(official|lyrics?|lirik|video|music|audio|hd|4k|high quality|remastered|opening|ending|theme|op|ed|ost|full|special|movie|anime|mv|pv|karaoke|cover)\b") {
        cleaned = re_keywords.replace_all(&cleaned, "").to_string();
    }

    // Clean up spaces
    if let Ok(re_spaces) = regex::Regex::new(r"\s+") {
        cleaned = re_spaces.replace_all(&cleaned, " ").to_string();
    }

    cleaned.trim().to_string()
}

#[allow(clippy::too_many_arguments)]
pub async fn fetch_and_save_lyrics(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    album: &str,
    duration: f64,
    dest_dir: &str,
    clean_title: &str,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = tx.send(AppEvent::ProcessingLog("[>] Starting lyrics search...".to_string())).await;
    
    let clean_artist = clean_query_name(artist);
    let clean_track = clean_query_name(title);
    
    let synced_lyrics = match get_lrclib_synced_lyrics(client, &clean_artist, &clean_track, album, duration, tx.clone()).await {
        Ok(Some(lrc)) => Some(lrc),
        _ => {
            let _ = tx.send(AppEvent::ProcessingLog("[>] Swapping artist and title search...".to_string())).await;
            match get_lrclib_synced_lyrics(client, &clean_track, &clean_artist, album, duration, tx.clone()).await {
                Ok(Some(lrc)) => Some(lrc),
                _ => None,
            }
        }
    };
    
    let lrc_content = if let Some((synced, lrc_duration)) = synced_lyrics {
        let _ = tx.send(AppEvent::ProcessingLog("[+] Synced lyrics found on LRCLIB.".to_string())).await;
        let offset = duration - lrc_duration;
        
        if contains_japanese(&synced) {
            let _ = tx.send(AppEvent::ProcessingLog("[>] Japanese lyrics detected. Romanizing...".to_string())).await;
            if let Some(res) = run_python_lyrics_worker("process", offset, &clean_artist, &clean_track, Some(&synced)).await {
                let _ = tx.send(AppEvent::ProcessingLog("[+] Processed lyrics using Python lyrics worker.".to_string())).await;
                res
            } else {
                let _ = tx.send(AppEvent::ProcessingLog("[!] Python worker not available or failed. Using Rust fallback...".to_string())).await;
                romanize_lrc(&synced, &clean_artist, &clean_track, offset, client, tx.clone()).await
            }
        } else {
            let _ = tx.send(AppEvent::ProcessingLog("[>] Shifting timestamps to match audio...".to_string())).await;
            shift_lrc_timestamps(&synced, offset)
        }
    } else {
        let _ = tx.send(AppEvent::ProcessingLog("[!] Synced lyrics not found on LRCLIB. Trying Genius fallback...".to_string())).await;
        if let Some(res) = run_python_lyrics_worker("fetch", 0.0, &clean_artist, &clean_track, None).await {
            let _ = tx.send(AppEvent::ProcessingLog("[+] Found lyrics on Genius via Python worker.".to_string())).await;
            format!("[ar:{}]\n[al:{}]\n[ti:{}]\n{}", artist, album, title, res)
        } else {
            let _ = tx.send(AppEvent::ProcessingLog("[!] Python worker fallback failed or not found. Trying Rust Genius search...".to_string())).await;
            match fetch_genius_lyrics(client, &clean_artist, &clean_track, tx.clone()).await {
                Ok(Some(genius_lyrics)) => {
                    let _ = tx.send(AppEvent::ProcessingLog("[+] Plain lyrics found on Genius.".to_string())).await;
                    format!("[ar:{}]\n[al:{}]\n[ti:{}]\n{}", artist, album, title, genius_lyrics)
                }
                _ => {
                    let _ = tx.send(AppEvent::ProcessingLog("[!] No lyrics found on Genius. Skipping.".to_string())).await;
                    return Ok(());
                }
            }
        }
    };
    
    let lrc_path = format!("{}/{}.lrc", dest_dir, clean_title);
    std::fs::write(&lrc_path, lrc_content)?;
    let _ = tx.send(AppEvent::ProcessingLog(format!("[+] Lyrics saved to: {}", lrc_path))).await;
    
    Ok(())
}
