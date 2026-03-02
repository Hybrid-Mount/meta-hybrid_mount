// Copyright 2026 Hybrid Mount Developers
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
    let _run_id = env::var("GITHUB_RUN_ID").unwrap_or_default();
    let server_url = env::var("GITHUB_SERVER_URL").unwrap_or("https://github.com".to_string());

    let branch_name = env::var("GITHUB_REF_NAME").unwrap_or_else(|_| get_git_branch());

    let output_dir = PathBuf::from("output");
    let mut zip_file: Option<PathBuf> = None;

    if let Ok(entries) = fs::read_dir(&output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "zip" {
                    zip_file = Some(path);
                    break;
                }
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

    let (commit_msg, commit_hash) = get_git_commit();
    let safe_commit_msg = escape_html(&commit_msg);

    let caption = format!(
        "🌾 <b>Hybrid-Mount: {}</b>\n\n\
        🌿 <b>分支 (Branch):</b> {}\n\n\
        ⚖️ <b>重量 (Weight):</b> {:.2} MB\n\n\
        📝 <b>新性状 (Commit):</b>\n\
        <pre>{}</pre>\n\n\
        🚜 <a href='{}/{}/commit/{}'>查看日志 (View Log)</a>",
        event_label, branch_name, file_size, safe_commit_msg, server_url, repo, commit_hash
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
        action.with_caption(format!("{}/{}/commit/{}", server_url, repo, commit_hash))
    };
    bot.execute(action).await?;

    Ok(())
}

fn get_git_commit() -> (String, String) {
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .output();

    let msg = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "No commit message available.".to_string(),
    };

    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%H"])
        .output();

    let hash = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "000000".to_string(),
    };

    (msg, hash)
}

fn get_git_branch() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "Unknown".to_string(),
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
