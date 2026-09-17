// SPDX-License-Identifier: GPL-3.0-only

//! Telegram build-artifact notifications for Hybrid Mount.
//! Without secrets, `maybe_send_output_dir_notification` skips silently after printing a notice.

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

    #[must_use]
    pub fn with_topic_id(mut self, topic_id: Option<i64>) -> Self {
        self.topic_id = topic_id;
        self
    }
}

/// Skips and returns `Ok(false)` when secrets are absent, so local builds behave as before.
pub fn maybe_send_output_dir_notification(request: &NotifyRequest) -> Result<bool> {
    let has_token = env::var("TELEGRAM_BOT_TOKEN").is_ok_and(|value| !value.trim().is_empty());
    let has_chat = env::var("TELEGRAM_CHAT_ID").is_ok_and(|value| !value.trim().is_empty());

    if !has_token || !has_chat {
        println!("Telegram secrets not configured, skipping notification.");
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
    let commit_link = escape_html(&format!("{server_url}/{repo}/commit/{commit_hash}"));

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
            context.send_artifact_group(chunk).await?;
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
            "Dispatching yield to Telegram: {} ({:.2} MB)",
            artifact.file_name,
            bytes_to_mib(artifact.size_bytes)
        );

        self.bot
            .execute(self.build_single_artifact_action(artifact, index).await?)
            .await?;
        Ok(())
    }

    async fn build_single_artifact_action(
        &self,
        artifact: &Artifact,
        index: usize,
    ) -> Result<SendDocument> {
        let caption = self.caption_for_artifact(artifact, index);
        let mut action = SendDocument::new(
            self.chat_id.to_owned(),
            InputFile::path(artifact.path.clone()).await?,
        );

        if let Some(topic_id) = self.request.topic_id {
            action = action.with_message_thread_id(topic_id);
        }

        Ok(action
            .with_caption_parse_mode(ParseMode::Html)
            .with_caption(caption))
    }

    async fn send_artifact_group(&self, artifacts: &[Artifact]) -> Result<()> {
        let file_names = artifacts
            .iter()
            .map(|artifact| artifact.file_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!("Dispatching yield bundle to Telegram: {file_names}");

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

        let (last, leading) = artifacts
            .split_last()
            .context("cannot build an empty Telegram media group")?;

        for artifact in leading {
            let file = InputFile::path(artifact.path.clone()).await?;
            let document = InputMediaDocument::default().with_disable_content_type_detection(true);
            items.push(MediaGroupItem::for_document(file, document));
        }

        items.push(MediaGroupItem::for_document(
            InputFile::path(last.path.clone()).await?,
            InputMediaDocument::default()
                .with_disable_content_type_detection(true)
                .with_caption_parse_mode(ParseMode::Html)
                .with_caption(caption.to_owned()),
        ));
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

    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to read an entry in {}", output_dir.display()))?;
        let path = entry.path();
        if is_zip_path(&path) {
            let metadata = entry
                .metadata()
                .with_context(|| format!("failed to inspect artifact {}", path.display()))?;
            if !metadata.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            artifacts.push(Artifact {
                path,
                file_name,
                size_bytes: metadata.len(),
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
        "🌾 <b>Hybrid Mount: {}</b>\n\n\
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
        "🌾 <b>Hybrid Mount: {}</b>\n\n\
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
    if caption.chars().count() <= 1024 {
        caption.to_string()
    } else {
        fallback_link.to_string()
    }
}

fn truncate_caption_extra(caption: &str, file_name: &str) -> String {
    if caption.chars().count() <= 1024 {
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
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn minimal_tokio_features_support_the_notify_runtime() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let value = runtime.block_on(async { 42 });
        assert_eq!(value, 42);
    }

    #[test]
    fn find_zip_files_returns_all_zips_sorted() -> Result<()> {
        let output_dir = make_temp_output_dir()?;
        File::create(output_dir.join("Hybrid-Mount-6.0.0-1.zip"))?;
        File::create(output_dir.join("Hybrid-Mount-6.0.0-2.zip"))?;
        File::create(output_dir.join("notes.txt"))?;

        let artifacts = find_zip_files(&output_dir)?;
        let names: Vec<_> = artifacts
            .iter()
            .map(|artifact| artifact.file_name.as_str())
            .collect();

        assert_eq!(
            names,
            vec!["Hybrid-Mount-6.0.0-1.zip", "Hybrid-Mount-6.0.0-2.zip"]
        );

        fs::remove_dir_all(output_dir)?;
        Ok(())
    }

    #[test]
    fn find_zip_files_errors_on_empty_output() {
        let output_dir = make_temp_output_dir().unwrap();
        let err = find_zip_files(&output_dir).unwrap_err();
        assert!(err.to_string().contains("no zip files"));
        fs::remove_dir_all(output_dir).ok();
    }

    #[test]
    fn captions_escape_and_brand_correctly() {
        let escaped = escape_html("<script>\"&'");
        assert_eq!(escaped, "&lt;script&gt;&quot;&amp;&#39;");

        let caption = build_extra_caption(
            &NotifyRequest::new("output", "日常耕作 🌱"),
            2,
            3,
            &Artifact {
                path: PathBuf::from("x.zip"),
                file_name: "Hybrid-Mount-6.0.0-1.zip".to_string(),
                size_bytes: 1024 * 1024,
            },
        );
        assert!(caption.contains("Hybrid Mount"));
        assert!(caption.contains("2/3"));
    }

    fn make_temp_output_dir() -> Result<PathBuf> {
        // The clock alone is not enough: it can return the same value for two
        // calls in a row, which let the parallel tests in this module share one
        // directory and delete each other's fixtures.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let seq = NEXT.fetch_add(1, Ordering::Relaxed);
        let output_dir = env::temp_dir().join(format!("notify-test-{nanos}-{seq}"));
        fs::create_dir_all(&output_dir)?;
        Ok(output_dir)
    }

    #[test]
    fn temp_output_dirs_are_unique_across_concurrent_calls() {
        // The collision this guards happened between parallel tests, so the
        // calls must race: a sequential loop advances the clock far enough
        // between iterations to hide a missing discriminator.
        let threads = 8;
        let per_thread = 200;
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                std::thread::spawn(move || {
                    (0..per_thread)
                        .map(|_| make_temp_output_dir().expect("temp dir"))
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        for handle in handles {
            for dir in handle.join().expect("join") {
                assert!(seen.insert(dir.clone()), "duplicate temp dir: {dir:?}");
                fs::remove_dir_all(dir).ok();
            }
        }
        assert_eq!(seen.len(), threads * per_thread);
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use std::{io::Read, thread, time::Duration};

    fn capture_upload(count: usize) -> Result<String> {
        let server =
            tiny_http::Server::http("127.0.0.1:0").map_err(|err| anyhow::anyhow!("{err}"))?;
        let host = format!("http://{}", server.server_addr());
        let receiver = thread::spawn(move || -> Result<String> {
            let mut request = server
                .recv_timeout(Duration::from_secs(10))?
                .context("upload not received")?;
            let mut body = String::new();
            request
                .as_reader()
                .take(1_000_000)
                .read_to_string(&mut body)?;
            let message = r#"{"message_id":1,"date":0,"chat":{"id":123,"type":"private","first_name":"Test"}}"#;
            let result = if count == 1 {
                message.to_owned()
            } else {
                format!("[{message},{message}]")
            };
            request.respond(tiny_http::Response::from_string(format!(
                r#"{{"ok":true,"result":{result}}}"#
            )))?;
            Ok(body)
        });
        let dir = env::temp_dir().join(format!("notify-wire-{}-{count}", std::process::id()));
        fs::create_dir_all(&dir)?;
        for index in 0..count {
            fs::write(dir.join(format!("artifact-{index}.zip")), b"zip fixture")?;
        }
        let artifacts = find_zip_files(&dir)?;
        let request = NotifyRequest::new(&dir, "wire test").with_topic_id(Some(37));
        let sent = tokio::runtime::Runtime::new()?.block_on(async {
            let http = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()?;
            let bot = Client::with_http_client(http, "test-token")
                .with_host(host)
                .with_max_retries(0);
            let context = NotificationContext {
                bot: &bot,
                chat_id: "123",
                request: &request,
                branch_name: "dev",
                artifact_count: count,
                safe_commit_msg: "fix &amp; verify",
                commit_link: "https://example.com/commit/test",
            };
            if count == 1 {
                context.send_single_artifact(&artifacts[0], 0).await
            } else {
                context.send_artifact_group(&artifacts).await
            }
        });
        let received = receiver
            .join()
            .map_err(|_| anyhow::anyhow!("mock server panicked"))?;
        fs::remove_dir_all(dir)?;
        sent?;
        received
    }

    fn field<'a>(body: &'a str, name: &str) -> &'a str {
        let marker = format!("name=\"{name}\"");
        body.split("\r\n--")
            .filter_map(|part| part.split_once("\r\n\r\n"))
            .find(|(headers, _)| headers.contains(&marker))
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("missing multipart field {name}"))
    }

    #[test]
    fn single_upload_sends_plain_html_parse_mode() -> Result<()> {
        let body = capture_upload(1)?;
        assert_eq!(field(&body, "parse_mode"), "HTML");
        assert_eq!(field(&body, "message_thread_id"), "37");
        assert_eq!(field(&body, "chat_id"), "123");
        let expected = build_primary_caption(
            &NotifyRequest::new("unused", "wire test"),
            "dev",
            1,
            "fix &amp; verify",
            "https://example.com/commit/test",
        );
        assert_eq!(field(&body, "caption"), expected);
        assert!(body.contains("filename=\"artifact-0.zip\""));
        assert!(body.contains("zip fixture"));
        Ok(())
    }

    #[test]
    fn group_upload_sends_html_mode_inside_media_json() -> Result<()> {
        let body = capture_upload(2)?;
        let media: serde_json::Value = serde_json::from_str(field(&body, "media"))?;
        assert_eq!(media.as_array().unwrap().len(), 2);
        assert!(media[0].get("caption").is_none());
        assert_eq!(media[1]["parse_mode"], "HTML");
        assert!(
            media[1]["caption"]
                .as_str()
                .unwrap()
                .contains("fix &amp; verify")
        );
        assert_eq!(field(&body, "message_thread_id"), "37");
        assert_eq!(field(&body, "chat_id"), "123");
        Ok(())
    }
}
