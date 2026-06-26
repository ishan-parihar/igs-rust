use crate::tools::types::*;
use tokio::process::Command;

pub async fn youtube_search(input: YoutubeSearchInput) -> Result<YoutubeSearchOutput, String> {
    let limit = input.limit.unwrap_or(10).min(50);
    let search_term = format!("ytsearch{}:{}", limit, input.query);

    let output = Command::new("yt-dlp")
        .args([&search_term, "--flat-playlist", "--print", "%(id)s|||%(title)s|||%(channel)s|||%(duration_string)s"])
        .output()
        .await
        .map_err(|e| format!("yt-dlp not found or failed to execute: {}. Install with: pip install yt-dlp", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("ERROR") {
            return Err(format!("yt-dlp error: {}", stderr.trim()));
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut videos = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        let parts: Vec<&str> = line.splitn(4, "|||").collect();
        if parts.len() < 3 { continue; }

        videos.push(YoutubeVideo {
            id: parts[0].to_string(),
            title: parts[1].to_string(),
            url: format!("https://www.youtube.com/watch?v={}", parts[0]),
            channel: parts[2].to_string(),
            duration: if parts.len() > 3 && !parts[3].is_empty() {
                Some(parts[3].to_string())
            } else {
                None
            },
        });
    }

    let count = videos.len();
    Ok(YoutubeSearchOutput { videos, count })
}

pub async fn youtube_metadata(input: YoutubeMetadataInput) -> Result<YoutubeMetadataOutput, String> {
    let output = Command::new("yt-dlp")
        .args(["--dump-json", "--no-download", "--no-playlist"])
        .arg(&input.url)
        .output()
        .await
        .map_err(|e| format!("yt-dlp not found or failed to execute: {}. Install with: pip install yt-dlp", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse yt-dlp JSON output: {}", e))?;

    Ok(YoutubeMetadataOutput {
        title: json["title"].as_str().unwrap_or("").to_string(),
        description: json["description"].as_str().unwrap_or("").to_string(),
        channel: json["channel"].as_str().unwrap_or("").to_string(),
        duration: json["duration_string"].as_str().map(|s| s.to_string()),
        views: json["view_count"].as_u64(),
        likes: json["like_count"].as_u64(),
        upload_date: json["upload_date"].as_str().map(|s| s.to_string()),
    })
}

pub async fn youtube_subtitles(input: YoutubeSubtitlesInput) -> Result<YoutubeSubtitlesOutput, String> {
    let lang = input.language.unwrap_or_else(|| "en".to_string());

    let list_output = Command::new("yt-dlp")
        .args(["--list-subs", "--no-download", "--no-playlist", "--skip-download"])
        .arg(&input.url)
        .output()
        .await
        .map_err(|e| format!("yt-dlp not found or failed to execute: {}. Install with: pip install yt-dlp", e))?;

    let list_stdout = String::from_utf8_lossy(&list_output.stdout);

    let actual_lang = if list_stdout.contains(&format!("{} ", lang)) {
        lang.clone()
    } else if list_stdout.contains("en ") {
        "en".to_string()
    } else {
        let auto_output = Command::new("yt-dlp")
            .args(["--list-subs", "--no-download", "--no-playlist", "--skip-download", "--write-auto-sub"])
            .arg(&input.url)
            .output()
            .await
            .map_err(|e| format!("yt-dlp failed to list auto-subs: {}", e))?;

        let auto_stdout = String::from_utf8_lossy(&auto_output.stdout);
        if auto_stdout.contains(&format!("{} ", lang)) {
            lang.clone()
        } else if auto_stdout.contains("en ") {
            "en".to_string()
        } else {
            lang.clone()
        }
    };

    let temp_dir = std::env::temp_dir();
    let output_template = temp_dir.join("igs_sub_%(id)s.%(ext)s");

    let dl_output = Command::new("yt-dlp")
        .args([
            "--write-sub", "--write-auto-sub",
            "--sub-format", "vtt",
            "--sub-langs", &actual_lang,
            "--skip-download", "--no-playlist",
            "-o", &output_template.to_string_lossy(),
        ])
        .arg(&input.url)
        .output()
        .await
        .map_err(|e| format!("Failed to download subtitles: {}", e))?;

    if !dl_output.status.success() {
        let stderr = String::from_utf8_lossy(&dl_output.stderr);
        return Err(format!("yt-dlp subtitle download failed: {}", stderr.trim()));
    }

    let subtitle_text = if let Some(entries) = std::fs::read_dir(&temp_dir).ok() {
        let mut found = String::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("igs_sub_") && name_str.ends_with(".vtt") {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    found = parse_vtt_to_text(&content);
                    let _ = std::fs::remove_file(entry.path());
                }
                break;
            }
        }
        found
    } else {
        String::new()
    };

    if subtitle_text.is_empty() {
        return Err(format!(
            "No subtitles found for language '{}'. The video may not have subtitles available.",
            actual_lang
        ));
    }

    Ok(YoutubeSubtitlesOutput {
        subtitles: subtitle_text,
        language: actual_lang,
    })
}

fn parse_vtt_to_text(vtt: &str) -> String {
    let mut lines = Vec::new();
    let mut skip_header = true;

    for line in vtt.lines() {
        let trimmed = line.trim();

        if skip_header {
            if trimmed.starts_with("WEBVTT") || trimmed.starts_with("Kind:") || trimmed.starts_with("Language:") {
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            skip_header = false;
        }

        if trimmed.parse::<u64>().is_ok() {
            continue;
        }

        if trimmed.contains("-->") {
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("NOTE") || trimmed.starts_with("STYLE") || trimmed.starts_with("::") {
            continue;
        }

        let cleaned = remove_html_tags(trimmed);
        if !cleaned.is_empty() {
            lines.push(cleaned);
        }
    }

    lines.join("\n")
}

fn remove_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
}
