// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use reqwest::blocking::{Client, multipart};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, exit};
use std::thread;
use std::time::Duration;

fn main() {
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

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("Failed to build HTTP client");

    let max_retries = 2;
    for attempt in 0..max_retries {
        let mut form = multipart::Form::new()
            .text("chat_id", chat_id.clone())
            .text("caption", caption.clone())
            .text("parse_mode", "HTML");

        if let Some(tid) = topic_id {
            if !tid.trim().is_empty() && tid != "0" {
                form = form.text("message_thread_id", tid.to_string());
                if attempt == 0 {
                    println!("Targeting Topic ID: {}", tid);
                }
            }
        }

        match form.file("document", &file_path) {
            Ok(f) => form = f,
            Err(e) => {
                eprintln!("❌ Critical Error reading file: {}", e);
                exit(1);
            }
        };

        let url = format!("https://api.telegram.org/bot{}/sendDocument", bot_token);

        match client.post(&url).multipart(form).send() {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().unwrap_or_default();

                if status.is_success() {
                    println!("✅ Yield stored successfully!");
                    return;
                }

                if text.contains("TOPIC_CLOSED") {
                    if attempt < max_retries - 1 {
                        if let Some(tid) = topic_id {
                            if reopen_topic(&client, &bot_token, &chat_id, tid) {
                                println!("🔄 Retrying upload in 2 seconds...");
                                thread::sleep(Duration::from_secs(2));
                                continue;
                            } else {
                                eprintln!("❌ Could not reopen topic. Aborting.");
                                exit(1);
                            }
                        }
                    } else {
                        eprintln!("❌ Retries exhausted.");
                    }
                }

                eprintln!(
                    "❌ Storage failed (Attempt {}/{}): Status {} - {}",
                    attempt + 1,
                    max_retries,
                    status,
                    text
                );
            }
            Err(e) => {
                eprintln!(
                    "❌ Network error (Attempt {}/{}): {}",
                    attempt + 1,
                    max_retries,
                    e
                );
            }
        }

        if attempt == max_retries - 1 {
            exit(1);
        }
        thread::sleep(Duration::from_secs(2));
    }
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

fn reopen_topic(client: &Client, bot_token: &str, chat_id: &str, topic_id: &str) -> bool {
    println!("⚠️ Topic {} is closed. Attempting to reopen...", topic_id);
    let url = format!("https://api.telegram.org/bot{}/reopenForumTopic", bot_token);
    let json_body = format!(
        r#"{{"chat_id": "{}", "message_thread_id": {}}}"#,
        chat_id, topic_id
    );

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(json_body)
        .send()
    {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✅ Topic {} successfully reopened!", topic_id);
                true
            } else {
                eprintln!(
                    "❌ Failed to reopen topic: {}",
                    resp.text().unwrap_or_default()
                );
                false
            }
        }
        Err(e) => {
            eprintln!("❌ Network error reopening topic: {}", e);
            false
        }
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
