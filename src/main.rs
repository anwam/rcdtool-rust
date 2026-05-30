mod cli;
mod config;
mod downloader;
mod telegram;
mod utils;

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use tracing::{debug, info, warn};

use cli::Arguments;
use config::AppConfig;
use downloader::{DownloadRequest, Downloader};
use utils::{
    default_output_path, parse_channel_id, parse_message_id, parse_ranges,
    parse_target_spec_from_link,
};

#[derive(Debug, Clone)]
struct RawTarget {
    channel_raw: String,
    message_raw: String,
    discussion_message_raw: Option<String>,
    batch_id: String,
}

#[tokio::main]
async fn main() {
    init_logging();

    if let Err(err) = run().await {
        tracing::error!(error = %err, "rcdtool-rust failed");
        std::process::exit(1);
    }
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,rcdtool_rust=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

async fn run() -> Result<()> {
    let args = Arguments::parse_compat();
    let app_config = AppConfig::from_file(&args.config_filename)?;
    let downloader = Downloader::connect(app_config, args.dry_run).await?;

    let raw_targets = collect_targets(&args)?;
    if raw_targets.is_empty() {
        warn!("no valid targets found");
        return Ok(());
    }

    let mut requests = Vec::with_capacity(raw_targets.len());
    let mut allocated_names: Vec<String> = Vec::with_capacity(raw_targets.len());

    for raw_target in raw_targets {
        let channel_id = parse_channel_id(&raw_target.channel_raw)?;
        let message_id = parse_message_id(&raw_target.message_raw)?;
        let discussion_message_id = raw_target
            .discussion_message_raw
            .as_deref()
            .map(parse_message_id)
            .transpose()?;

        let base_output = args.output_filename.clone().unwrap_or_else(|| {
            default_output_path(
                &raw_target.channel_raw,
                message_id,
                discussion_message_id,
                &raw_target.batch_id,
            )
        });

        let output_filename = utils::generate_unique_filename(
            &base_output,
            args.detailed_name,
            Some(format!(
                "-{}-{}",
                raw_target.channel_raw, raw_target.message_raw
            )),
            &allocated_names,
        );
        allocated_names.push(output_filename.clone());

        let request = DownloadRequest {
            channel_id,
            message_id,
            output_filename,
            infer_extension: args.infer_extension,
            discussion_message_id,
        };

        requests.push(request);
    }

    debug!(
        targets = requests.len(),
        concurrency = args.concurrency,
        "prepared download requests"
    );
    let start = Instant::now();
    let files = stream::iter(
        requests
            .into_iter()
            .map(|request| downloader.download_media(request)),
    )
    .buffer_unordered(args.concurrency)
    .collect::<Vec<_>>()
    .await;

    info!(
        elapsed_seconds = start.elapsed().as_secs_f64(),
        "total download time"
    );

    for output in files {
        match output {
            Ok(Some(path)) => println!("{path}"),
            Ok(None) => {}
            Err(err) => warn!(error = %err, "download target failed"),
        }
    }

    Ok(())
}

fn collect_targets(args: &Arguments) -> Result<Vec<RawTarget>> {
    let mut targets = Vec::new();

    let links = collect_links(args)?;

    if !links.is_empty() {
        for link in links {
            let batch_id = compute_batch_id(&link);
            let (channel_id, message_expr, link_discussion_expr) =
                parse_target_spec_from_link(&link)?;
            let discussion_expr = if let Some(cli_dm) = &args.discussion_message_id {
                cli_dm.clone()
            } else if let Some(from_link) = link_discussion_expr {
                from_link
            } else {
                String::new()
            };

            let discussion_ranges = if discussion_expr.is_empty() {
                vec![(0, 0)]
            } else {
                parse_ranges(&discussion_expr)?
            };

            for (start, end) in parse_ranges(&message_expr)? {
                for message_id in start..=end {
                    if discussion_expr.is_empty() {
                        targets.push(RawTarget {
                            channel_raw: channel_id.clone(),
                            message_raw: message_id.to_string(),
                            discussion_message_raw: None,
                            batch_id: batch_id.clone(),
                        });
                        continue;
                    }

                    for (dm_start, dm_end) in &discussion_ranges {
                        for discussion_message_id in *dm_start..=*dm_end {
                            targets.push(RawTarget {
                                channel_raw: channel_id.clone(),
                                message_raw: message_id.to_string(),
                                discussion_message_raw: Some(discussion_message_id.to_string()),
                                batch_id: batch_id.clone(),
                            });
                        }
                    }
                }
            }
        }
    } else {
        let channel_id = args.channel_id.clone().unwrap_or_default();
        if !channel_id.is_empty() && !channel_id.chars().all(|c| c.is_ascii_digit()) {
            warn!(channel_id = %channel_id, "channel id is not a digit (username mode may be intended)");
        }

        let message_expr = args.message_id.clone().unwrap_or_default();
        let batch_id = compute_batch_id(&format!("{}:{}", channel_id, message_expr));
        let discussion_expr = args.discussion_message_id.clone().unwrap_or_default();
        let discussion_ranges = if discussion_expr.is_empty() {
            vec![(0, 0)]
        } else {
            parse_ranges(&discussion_expr)?
        };

        for (start, end) in parse_ranges(&message_expr)? {
            for message_id in start..=end {
                if discussion_expr.is_empty() {
                    targets.push(RawTarget {
                        channel_raw: channel_id.clone(),
                        message_raw: message_id.to_string(),
                        discussion_message_raw: None,
                        batch_id: batch_id.clone(),
                    });
                    continue;
                }
                for (dm_start, dm_end) in &discussion_ranges {
                    for discussion_message_id in *dm_start..=*dm_end {
                        targets.push(RawTarget {
                            channel_raw: channel_id.clone(),
                            message_raw: message_id.to_string(),
                            discussion_message_raw: Some(discussion_message_id.to_string()),
                            batch_id: batch_id.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(targets)
}

fn collect_links(args: &Arguments) -> Result<Vec<String>> {
    let mut links = Vec::new();

    if let Some(link_input) = &args.link {
        if !link_input.trim().is_empty() {
            links.extend(expand_link_inputs(link_input)?);
        }
    }

    for file in &args.link_files {
        links.extend(read_links_from_file(file)?);
    }

    Ok(links)
}

fn compute_batch_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:08x}", hasher.finish() & 0xffff_ffff)
}

fn expand_link_values(link_input: &str) -> Vec<String> {
    link_input
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
}

fn read_links_from_file(path: &str) -> Result<Vec<String>> {
    let source_path = Path::new(path);
    let content = std::fs::read_to_string(source_path)?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('#'))
        .map(ToString::to_string)
        .collect::<Vec<_>>())
}

fn expand_link_inputs(link_input: &str) -> Result<Vec<String>> {
    let mut links = Vec::new();

    for source in expand_link_values(link_input) {
        let source_path = Path::new(&source);
        let is_text_file = source_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));

        if is_text_file && source_path.is_file() {
            links.extend(read_links_from_file(&source)?);
        } else {
            links.push(source);
        }
    }

    Ok(links)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{collect_links, compute_batch_id, expand_link_inputs, read_links_from_file};
    use crate::cli::Arguments;

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn make_temp_file(content: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX_EPOCH")
            .as_nanos();
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rcdtool-rust-link-test-{}-{}-{}.txt",
            std::process::id(),
            nanos,
            counter
        ));
        fs::write(&path, content).expect("failed to write temp link file");
        path
    }

    #[test]
    fn read_links_from_file_skips_empty_and_comments() {
        let path = make_temp_file(
            "\n# comment\nhttps://t.me/a/1\n\n  https://t.me/b/2?comment=7  \n# tail\n",
        );

        let result =
            read_links_from_file(path.to_str().expect("invalid utf-8 path")).expect("read failed");
        fs::remove_file(path).expect("failed to cleanup temp file");

        assert_eq!(
            result,
            vec![
                "https://t.me/a/1".to_string(),
                "https://t.me/b/2?comment=7".to_string()
            ]
        );
    }

    #[test]
    fn collect_links_combines_link_and_link_file_inputs() {
        let file_path = make_temp_file("https://t.me/file/10\nhttps://t.me/file/11\n");
        let file_path_str = file_path.to_str().expect("invalid utf-8 path").to_string();

        let args = Arguments {
            link: Some("https://t.me/direct/1".to_string()),
            link_files: vec![file_path_str],
            config_filename: "config.ini".to_string(),
            channel_id: None,
            message_id: None,
            discussion_message_id: None,
            output_filename: None,
            infer_extension: false,
            detailed_name: false,
            concurrency: 2,
            dry_run: true,
        };

        let result = collect_links(&args).expect("collect failed");
        fs::remove_file(file_path).expect("failed to cleanup temp file");

        assert_eq!(
            result,
            vec![
                "https://t.me/direct/1".to_string(),
                "https://t.me/file/10".to_string(),
                "https://t.me/file/11".to_string()
            ]
        );
    }

    #[test]
    fn compute_batch_id_is_stable_and_differs_across_inputs() {
        let id_a = compute_batch_id("https://t.me/chan/100");
        let id_b = compute_batch_id("https://t.me/chan/100");
        let id_c = compute_batch_id("https://t.me/chan/200");
        assert_eq!(id_a, id_b, "same input must produce same batch_id");
        assert_ne!(
            id_a, id_c,
            "different inputs must produce different batch_ids"
        );
        // Must be exactly 8 lowercase hex chars
        assert_eq!(id_a.len(), 8);
        assert!(id_a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn expand_link_inputs_supports_txt_entries_in_link_value() {
        let path = make_temp_file("https://t.me/fromtxt/25?comment=101..103\n");
        let link_input = format!("{};https://t.me/direct/2", path.display());

        let result = expand_link_inputs(&link_input).expect("expand failed");
        fs::remove_file(path).expect("failed to cleanup temp file");

        assert_eq!(
            result,
            vec![
                "https://t.me/fromtxt/25?comment=101..103".to_string(),
                "https://t.me/direct/2".to_string()
            ]
        );
    }
}
