use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelId {
    Numeric(i64),
    Username(String),
}

pub fn parse_channel_id(raw: &str) -> Result<ChannelId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("channel id is required")
    }

    let stripped = trimmed.trim_start_matches('-');
    if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
        let parsed = trimmed
            .parse::<i64>()
            .with_context(|| format!("invalid channel id: {trimmed}"))?;
        if parsed > 0 {
            let with_prefix = format!("-100{parsed}");
            let normalized = with_prefix
                .parse::<i64>()
                .with_context(|| "failed to normalize private channel id")?;
            Ok(ChannelId::Numeric(normalized))
        } else {
            Ok(ChannelId::Numeric(parsed))
        }
    } else {
        Ok(ChannelId::Username(
            trimmed.trim_start_matches('@').to_string(),
        ))
    }
}

pub fn parse_message_id(raw: &str) -> Result<i32> {
    raw.trim()
        .parse::<i32>()
        .with_context(|| format!("invalid message id: {raw}"))
}

pub fn parse_ranges(input: &str) -> Result<Vec<(i32, i32)>> {
    let regex = Regex::new(r"(\d+)(?:\.{2}(\d+))?")?;
    let mut ranges = Vec::new();

    for part in input.split(',').map(str::trim).filter(|v| !v.is_empty()) {
        let captures = regex
            .captures(part)
            .ok_or_else(|| anyhow!("invalid message range segment: {part}"))?;
        let start = parse_message_id(&captures[1])?;
        let end = captures
            .get(2)
            .map(|m| parse_message_id(m.as_str()))
            .transpose()?
            .unwrap_or(start);

        if end < start {
            bail!("range end must be greater than or equal to start: {part}");
        }
        ranges.push((start, end));
    }

    Ok(ranges)
}

pub fn parse_target_spec_from_link(link: &str) -> Result<(String, String, Option<String>)> {
    let cleaned = link.trim().trim_end_matches('/');
    if cleaned.is_empty() {
        bail!("empty link")
    }

    let without_scheme = cleaned
        .strip_prefix("https://")
        .or_else(|| cleaned.strip_prefix("http://"))
        .unwrap_or(cleaned);

    let path_and_query = without_scheme
        .split_once('/')
        .map(|(_, tail)| tail)
        .ok_or_else(|| anyhow!("invalid message link: {cleaned}"))?;

    let (path_part, query_part) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query, None));

    let path_part = path_part.trim_end_matches('/');
    let parts = path_part
        .split('/')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();

    let discussion_expr = query_part.and_then(parse_discussion_message_expr);

    if parts.first() == Some(&"c") {
        if parts.len() < 3 {
            bail!("invalid channel link: {cleaned}")
        }

        let channel_id = parts[1].to_string();
        let message_id = parts
            .last()
            .ok_or_else(|| anyhow!("missing message id in link: {cleaned}"))?
            .to_string();

        return Ok((channel_id, message_id, discussion_expr));
    }

    if cleaned.contains("/c/") && parts.len() >= 3 {
        let index = parts
            .iter()
            .position(|v| *v == "c")
            .ok_or_else(|| anyhow!("invalid channel link: {cleaned}"))?;

        let channel_id = parts
            .get(index + 1)
            .ok_or_else(|| anyhow!("missing channel id in link: {cleaned}"))?;
        let message_id = parts
            .last()
            .ok_or_else(|| anyhow!("missing message id in link: {cleaned}"))?;

        return Ok((
            (*channel_id).to_string(),
            (*message_id).to_string(),
            discussion_expr,
        ));
    }

    if parts.len() < 2 {
        bail!("invalid message link: {cleaned}")
    }
    let channel_id = parts[parts.len() - 2].to_string();
    let message_id = parts[parts.len() - 1].to_string();
    Ok((channel_id, message_id, discussion_expr))
}

fn parse_discussion_message_expr(query: &str) -> Option<String> {
    query.split('&').map(str::trim).find_map(|segment| {
        let (key, value) = segment.split_once('=')?;
        if key == "comment" && !value.trim().is_empty() {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

pub fn default_output_path(
    channel: &str,
    message_id: i32,
    discussion_message_id: Option<i32>,
    batch_id: &str,
) -> String {
    match discussion_message_id {
        Some(dm_id) => format!("download/{channel}/{message_id}/{dm_id}"),
        None => format!("download/{channel}/{batch_id}/{message_id}"),
    }
}

pub fn generate_unique_filename(
    filepath: &str,
    is_detailed: bool,
    detail: Option<String>,
    exclude_names: &[String],
) -> String {
    let path = Path::new(filepath);
    let directory = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    let mut name = stem;
    if is_detailed {
        let suffix = detail.unwrap_or_else(|| "-detail".to_string());
        name.push_str(&suffix);
    }

    let mut candidate = directory.join(format!("{name}{ext}"));
    let mut counter: usize = 1;

    while candidate.exists() || exclude_names.iter().any(|v| Path::new(v) == candidate) {
        candidate = directory.join(format!("{name}-{counter}{ext}"));
        counter += 1;
    }

    pathbuf_to_string(candidate)
}

fn pathbuf_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

use anyhow::Context;

#[cfg(test)]
mod tests {
    use super::{
        ChannelId, default_output_path, parse_channel_id, parse_message_id, parse_ranges,
        parse_target_spec_from_link,
    };

    #[test]
    fn parse_channel_id_from_number() {
        let result = parse_channel_id("300020001000").expect("parse failed");
        assert_eq!(result, ChannelId::Numeric(-100300020001000));
    }

    #[test]
    fn parse_channel_id_from_negative_number() {
        let result = parse_channel_id("-10001000").expect("parse failed");
        assert_eq!(result, ChannelId::Numeric(-10001000));
    }

    #[test]
    fn parse_channel_id_from_public_link() {
        let result = parse_channel_id("@qwerty").expect("parse failed");
        assert_eq!(result, ChannelId::Username("qwerty".to_string()));
    }

    #[test]
    fn parse_message_id_from_number_string() {
        let result = parse_message_id("1000").expect("parse failed");
        assert_eq!(result, 1000);
    }

    #[test]
    fn parse_ranges_complex() {
        let result = parse_ranges("1638,1639..1641,1650..1650").expect("parse failed");
        assert_eq!(result, vec![(1638, 1638), (1639, 1641), (1650, 1650)]);
    }

    #[test]
    fn parse_link_with_discussion_comment() {
        let result = parse_target_spec_from_link("https://t.me/1234567890/25?comment=101")
            .expect("parse failed");
        assert_eq!(
            result,
            (
                "1234567890".to_string(),
                "25".to_string(),
                Some("101".to_string())
            )
        );
    }

    #[test]
    fn parse_link_with_discussion_comment_range() {
        let result = parse_target_spec_from_link("https://t.me/1234567890/25?comment=101..105")
            .expect("parse failed");
        assert_eq!(
            result,
            (
                "1234567890".to_string(),
                "25".to_string(),
                Some("101..105".to_string())
            )
        );
    }

    #[test]
    fn default_output_path_uses_discussion_when_present() {
        let result = default_output_path("1234567890", 25, Some(101), "ignored");
        assert_eq!(result, "download/1234567890/25/101");
    }

    #[test]
    fn default_output_path_uses_batch_id_when_no_discussion() {
        let result = default_output_path("chan", 42, None, "a1b2c3d4");
        assert_eq!(result, "download/chan/a1b2c3d4/42");
    }
}
