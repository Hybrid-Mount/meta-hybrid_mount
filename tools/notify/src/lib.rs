// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use tgbot::{
    api::Client,
    types::{
        InputFile, InputMediaDocument, MediaGroup, MediaGroupItem, ParseMode, SendDocument,
        SendMediaGroup,
    },
};

const MAX_MEDIA_GROUP_ATTACHMENTS: usize = 10;

#[derive(Debug, Clone)]
pub struct NotifyRequest {
    pub output_dir: PathBuf,
    pub topic_id: Option<i64>,
    pub event_label: String,
}

impl NotifyRequest {
    pub fn new(output_dir: impl Into<PathBuf>, event_label: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
            topic_id: None,
            event_label: event_label.into(),
        }
    }

    pub fn with_topic_id(mut self, topic_id: Option<i64>) -> Self {
        self.topic_id = topic_id;
        self
    }
}

pub fn maybe_send_output_dir_notification(request: &NotifyRequest) -> Result<bool> {
    if env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .filter(|v| !v.is_empty())
        .is_none()
        || env::var("TELEGRAM_CHAT_ID")
            .ok()
            .filter(|v| !v.is_empty())
            .is_none()
    {
        return Ok(false);
    }

    send_output_dir_notification(request)?;
    Ok(true)
}

pub fn send_output_dir_notification(request: &NotifyRequest) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create Tokio runtime")?;
    runtime.block_on(send_output_dir_notification_async(request))
}

async fn send_output_dir_notification_async(request: &NotifyRequest) -> Result<()> {
    let bot_token = env::var("TELEGRAM_BOT_TOKEN").context("TELEGRAM_BOT_TOKEN not set")?;
    let chat_id = env::var("TELEGRAM_CHAT_ID").context("TELEGRAM_CHAT_ID not set")?;

    let repo = env::var("GITHUB_REPOSITORY").unwrap_or_default();
    let server_url =
        env::var("GITHUB_SERVER_URL").unwrap_or_else(|_| "https://github.com".to_string());
    let branch_name = env::var("GITHUB_REF_NAME").unwrap_or_else(|_| get_git_branch());

    let artifacts = find_zip_files(&request.output_dir)?;

    let (commit_msg, commit_hash) = get_git_commit();
    let safe_commit_msg = escape_html(&commit_msg);
    let commit_link = format!("{}/{}/commit/{}", server_url, repo, commit_hash);

    println!("Selecting {} yield(s)", artifacts.len());

    let bot = Client::new(bot_token)?;
    let context = NotificationContext {
        bot: &bot,
        chat_id: &chat_id,
        request,
        branch_name: &branch_name,
        artifact_count: artifacts.len(),
        safe_commit_msg: &safe_commit_msg,
        commit_link: &commit_link,
    };

    for (batch_index, chunk) in artifacts.chunks(MAX_MEDIA_GROUP_ATTACHMENTS).enumerate() {
        let start_index = batch_index * MAX_MEDIA_GROUP_ATTACHMENTS;
        if chunk.len() == 1 {
            context.send_single_artifact(&chunk[0], start_index).await?;
        } else {
            context.send_artifact_group(chunk, start_index).await?;
        }
    }

    Ok(())
}

struct NotificationContext<'a> {
    bot: &'a Client,
    chat_id: &'a str,
    request: &'a NotifyRequest,
    branch_name: &'a str,
    artifact_count: usize,
    safe_commit_msg: &'a str,
    commit_link: &'a str,
}

impl NotificationContext<'_> {
    async fn send_single_artifact(&self, artifact: &Artifact, index: usize) -> Result<()> {
        println!(
            "Dispatching yield to Granary (Telegram): {} ({:.2} MB)",
            artifact.file_name,
            bytes_to_mib(artifact.size_bytes)
        );

        let caption = self.caption_for_artifact(artifact, index);
        let mut action = SendDocument::new(
            self.chat_id.to_owned(),
            InputFile::path(artifact.path.clone()).await?,
        )
        .with_caption_parse_mode(ParseMode::Html);

        if let Some(topic_id) = self.request.topic_id {
            action = action.with_message_thread_id(topic_id);
        }

        let action = action.with_caption(&caption);
        self.bot.execute(action).await?;

        Ok(())
    }

    async fn send_artifact_group(&self, artifacts: &[Artifact], _start_index: usize) -> Result<()> {
        let file_names = artifacts
            .iter()
            .map(|artifact| artifact.file_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        println!(
            "Dispatching yield bundle to Granary (Telegram): {}",
            file_names,
        );

        let raw_caption = build_primary_caption(
            self.request,
            self.branch_name,
            self.artifact_count,
            self.safe_commit_msg,
            self.commit_link,
        );
        let caption = truncate_caption(&raw_caption, self.commit_link);
        let media = MediaGroup::new(self.build_media_group_items(artifacts, &caption).await?)?;
        let mut action = SendMediaGroup::new(self.chat_id.to_owned(), media);
        if let Some(topic_id) = self.request.topic_id {
            action = action.with_message_thread_id(topic_id);
        }

        self.bot.execute(action).await?;

        Ok(())
    }

    async fn build_media_group_items(
        &self,
        artifacts: &[Artifact],
        caption: &str,
    ) -> Result<Vec<MediaGroupItem>> {
        let mut items = Vec::with_capacity(artifacts.len());

        for (i, artifact) in artifacts.iter().enumerate() {
            let file = InputFile::path(artifact.path.clone()).await?;
            let mut info = InputMediaDocument::default().with_disable_content_type_detection(true);
            if i == 0 {
                info = info
                    .with_caption_parse_mode(ParseMode::Html)
                    .with_caption(caption.to_owned());
            }
            items.push(MediaGroupItem::for_document(file, info));
        }

        Ok(items)
    }

    fn caption_for_artifact(&self, artifact: &Artifact, index: usize) -> String {
        let caption = if index == 0 {
            build_primary_caption(
                self.request,
                self.branch_name,
                self.artifact_count,
                self.safe_commit_msg,
                self.commit_link,
            )
        } else {
            build_extra_caption(self.request, index + 1, self.artifact_count, artifact)
        };

        if index == 0 {
            truncate_caption(&caption, self.commit_link)
        } else {
            truncate_caption_extra(&caption, &artifact.file_name)
        }
    }
}

#[derive(Debug, Clone)]
struct Artifact {
    path: PathBuf,
    file_name: String,
    size_bytes: u64,
}

fn find_zip_files(output_dir: &Path) -> Result<Vec<Artifact>> {
    let entries = fs::read_dir(output_dir)
        .with_context(|| format!("failed to read output directory {}", output_dir.display()))?;
    let mut artifacts = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if is_zip_path(&path) {
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size_bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            artifacts.push(Artifact {
                path,
                file_name,
                size_bytes,
            });
        }
    }

    artifacts.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    if artifacts.is_empty() {
        bail!("no zip files found in {}", output_dir.display());
    }

    Ok(artifacts)
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

fn build_primary_caption(
    request: &NotifyRequest,
    branch_name: &str,
    artifact_count: usize,
    safe_commit_msg: &str,
    commit_link: &str,
) -> String {
    format!(
        "🌾 <b>Hybrid-Mount: {}</b>\n\n\
        🌿 <b>分支 (Branch):</b> {}\n\n\
        📦 <b>产物 (Artifacts):</b> {}\n\n\
        📝 <b>新性状 (Commit):</b>\n\
        <pre>{}</pre>\n\n\
        🚜 <a href='{}'>查看日志 (View Log)</a>",
        escape_html(&request.event_label),
        escape_html(branch_name),
        artifact_count,
        safe_commit_msg,
        commit_link
    )
}

fn build_extra_caption(
    request: &NotifyRequest,
    index: usize,
    artifact_count: usize,
    artifact: &Artifact,
) -> String {
    format!(
        "🌾 <b>Hybrid-Mount: {}</b>\n\n\
        📦 <b>产物 (Artifact):</b> {}/{}\n\n\
        <pre>{}</pre>\n\n\
        ⚖️ <b>重量 (Weight):</b> {:.2} MB",
        escape_html(&request.event_label),
        index,
        artifact_count,
        escape_html(&artifact.file_name),
        bytes_to_mib(artifact.size_bytes)
    )
}

fn truncate_caption(caption: &str, fallback_link: &str) -> String {
    if caption.chars().count() < 1024 {
        caption.to_string()
    } else {
        fallback_link.to_string()
    }
}

fn truncate_caption_extra(caption: &str, file_name: &str) -> String {
    if caption.chars().count() < 1024 {
        caption.to_string()
    } else {
        escape_html(file_name)
    }
}

fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

fn get_git_commit() -> (String, String) {
    let msg = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|msg| !msg.is_empty())
        .unwrap_or_else(|| "No commit message available.".to_string());

    let hash = Command::new("git")
        .args(["log", "-1", "--pretty=%H"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "000000".to_string());

    (msg, hash)
}

fn get_git_branch() -> String {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn find_zip_files_returns_all_zips_sorted() -> Result<()> {
        let output_dir = make_temp_output_dir()?;
        File::create(output_dir.join("Hybrid-Mount-lite.zip"))?;
        File::create(output_dir.join("Hybrid-Mount.zip"))?;
        File::create(output_dir.join("notes.txt"))?;

        let artifacts = find_zip_files(&output_dir)?;
        let names: Vec<_> = artifacts
            .iter()
            .map(|artifact| artifact.file_name.as_str())
            .collect();

        assert_eq!(names, vec!["Hybrid-Mount-lite.zip", "Hybrid-Mount.zip"]);

        fs::remove_dir_all(output_dir)?;
        Ok(())
    }

    fn make_temp_output_dir() -> Result<PathBuf> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let output_dir = env::temp_dir().join(format!("notify-test-{nanos}"));
        fs::create_dir_all(&output_dir)?;
        Ok(output_dir)
    }
}
