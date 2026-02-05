// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    env, fs,
    path::PathBuf,
    process::{Command, exit},
};

use anyhow::Result;
use tgbot::{
    api::Client,
    types::{InputFile, SendDocument},
};

#[tokio::main]
async fn main() -> Result<()> {
    let bot_token = env::var("TELEGRAM_BOT_TOKEN").expect("Error: TELEGRAM_BOT_TOKEN not set");
    let chat_id = env::var("TELEGRAM_CHAT_ID").expect("Error: TELEGRAM_CHAT_ID not set");

    let args: Vec<String> = env::args().collect();
    let topic_id = if args.len() > 1 { Some(&args[1]) } else { None };
    let event_label = if args.len() > 2 {
        &args[2]
    } else {
        "New Yield (新产物)"
    };

    let repo = env::var("GITHUB_REPOSITORY").unwrap_or_default();
    let run_id = env::var("GITHUB_RUN_ID").unwrap_or_default();
    let server_url = env::var("GITHUB_SERVER_URL").unwrap_or("https://github.com".to_string());
    let run_url = format!("{}/{}/actions/runs/{}", server_url, repo, run_id);

    let output_dir = PathBuf::from("output");
    let mut zip_file: Option<PathBuf> = None;

    if let Ok(entries) = fs::read_dir(&output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension()
                && ext == "zip"
            {
                zip_file = Some(path);
                break;
            }
        }
    }

    let file_path = match zip_file {
        Some(p) => p,
        None => {
            eprintln!("Error: No grain sacks (zip files) found in output/.");
            exit(1);
        }
    };

    let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();
    let file_size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0) as f64 / 1024.0 / 1024.0;

    println!("Selecting yield: {} ({:.2} MB)", file_name, file_size);

    let commit_msg = get_git_commit_message();
    let safe_commit_msg = escape_html(&commit_msg);

    let caption = format!(
        "🌾 <b>Meta-Hybrid: {}</b>\n\n\
        ⚖️ <b>重量 (Weight):</b> {:.2} MB\n\n\
        📝 <b>新性状 (Commit):</b>\n\
        <pre>{}</pre>\n\n\
        🚜 <a href='{}'>查看日志 (View Log)</a>",
        event_label, file_size, safe_commit_msg, run_url
    );

    println!("Dispatching yield to Granary (Telegram)...");

    let bot = Client::new(bot_token)?;

    let mut action = SendDocument::new(chat_id, InputFile::path(file_path).await?)
        .with_caption_parse_mode(tgbot::types::ParseMode::Html);

    if let Some(topic_id) = topic_id {
        action = action.with_message_thread_id(topic_id.parse::<i64>()?);
    }
    let action = if caption.len() < 1024 {
        action.with_caption(&caption)
    } else {
        action.with_caption(&run_url)
    };
    bot.execute(action).await?;

    Ok(())
}

fn get_git_commit_message() -> String {
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "No commit message available.".to_string(),
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
