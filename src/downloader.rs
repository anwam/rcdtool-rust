use anyhow::Result;
use grammers_client::Client;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::config::AppConfig;
use crate::telegram;
use crate::utils::ChannelId;

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub channel_id: ChannelId,
    pub message_id: i32,
    pub output_filename: String,
    pub infer_extension: bool,
    pub discussion_message_id: Option<i32>,
}

pub struct Downloader {
    dry_mode: bool,
    client: Option<Client>,
}

impl Downloader {
    /// Connect (and interactively authorize) the Telegram client, unless running
    /// in dry mode where no network access is required.
    pub async fn connect(config: AppConfig, dry_mode: bool) -> Result<Self> {
        let client = if dry_mode {
            None
        } else {
            Some(telegram::connect(&config).await?)
        };
        Ok(Self { dry_mode, client })
    }

    pub async fn download_media(&self, request: DownloadRequest) -> Result<Option<String>> {
        debug!(
            ?request.channel_id,
            message_id = request.message_id,
            output = %request.output_filename,
            infer_extension = request.infer_extension,
            ?request.discussion_message_id,
            "processing download target"
        );

        if self.dry_mode {
            info!("dry running target");
            return Ok(Some(request.output_filename));
        }

        let client = self
            .client
            .as_ref()
            .expect("client is connected when not in dry mode");

        ensure_parent_dir(&request.output_filename)?;

        let peer = telegram::resolve_peer(client, &request.channel_id).await?;
        let downloaded = telegram::download(
            client,
            peer,
            request.message_id,
            &request.output_filename,
            request.discussion_message_id,
        )
        .await?;

        if !downloaded {
            warn!(message_id = request.message_id, "no media found");
            return Ok(None);
        }
        info!(output = %request.output_filename, "downloaded");

        if request.infer_extension {
            if let Some(renamed) = telegram::apply_inferred_extension(&request.output_filename)? {
                debug!(output = %renamed, "renamed with inferred extension");
                return Ok(Some(renamed));
            }
        }

        Ok(Some(request.output_filename))
    }
}

fn ensure_parent_dir(output_filename: &str) -> Result<()> {
    if let Some(parent) = Path::new(output_filename).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
