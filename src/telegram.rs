use std::io::{self, BufRead, Write as _};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use grammers_client::{Client, SenderPool, SignInError, message::Message, tl};
use grammers_session::storages::SqliteSession;
use grammers_session::types::PeerRef;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::utils::ChannelId;

/// Connect to Telegram, spawn the network runner, and ensure the session is
/// authorized (running the interactive login flow only when needed).
///
/// Grammers stores its session in its own SQLite format, incompatible with the
/// Python tool's Telethon session, so we keep a separate
/// `<name>.grammers.session` file to avoid clobbering an existing one.
pub async fn connect(config: &AppConfig) -> Result<Client> {
    let session_path = format!("{}.grammers.session", config.access.session);
    let session = Arc::new(
        SqliteSession::open(&session_path)
            .await
            .map_err(|e| anyhow!("opening session '{session_path}': {e}"))?,
    );

    let SenderPool { runner, handle, .. } =
        SenderPool::new(Arc::clone(&session), config.access.api_id);
    let client = Client::new(handle);
    // The runner drives the connection; it stops once all client handles drop.
    tokio::spawn(runner.run());

    authorize(&client, &config.access.api_hash).await?;
    Ok(client)
}

async fn authorize(client: &Client, api_hash: &str) -> Result<()> {
    if client
        .is_authorized()
        .await
        .context("checking authorization")?
    {
        return Ok(());
    }

    let phone = prompt("Enter your phone number (international format, e.g. +15551234567): ")?;
    let token = client
        .request_login_code(phone.trim(), api_hash)
        .await
        .context("requesting login code")?;
    let code = prompt("Enter the login code you received: ")?;

    match client.sign_in(&token, code.trim()).await {
        Ok(user) => info!(user = user.first_name().unwrap_or("?"), "signed in"),
        Err(SignInError::PasswordRequired(password_token)) => {
            let message = match password_token.hint() {
                Some(hint) if !hint.is_empty() => {
                    format!("Enter your 2FA password (hint: {hint}): ")
                }
                _ => "Enter your 2FA password: ".to_string(),
            };
            let password = prompt(&message)?;
            client
                .check_password(password_token, password.trim())
                .await
                .context("checking 2FA password")?;
            info!("signed in with 2FA");
        }
        Err(err) => return Err(anyhow!("sign-in failed: {err}")),
    }

    Ok(())
}

/// Resolve a channel reference to a usable peer reference.
///
/// Usernames resolve directly; numeric (private) ids are matched against the
/// dialog list, since Grammers can only address private channels you are a
/// member of (it needs the cached access hash).
pub async fn resolve_peer(client: &Client, channel: &ChannelId) -> Result<PeerRef> {
    match channel {
        ChannelId::Username(username) => {
            let peer = client
                .resolve_username(username)
                .await
                .with_context(|| format!("resolving username '{username}'"))?
                .ok_or_else(|| anyhow!("no chat found for username '{username}'"))?;
            peer.to_ref()
                .await
                .ok_or_else(|| anyhow!("username '{username}' is not usable"))
        }
        ChannelId::Numeric(marked) => find_peer_by_id(client, *marked).await,
    }
}

async fn find_peer_by_id(client: &Client, marked: i64) -> Result<PeerRef> {
    let mut dialogs = client.iter_dialogs();
    while let Some(dialog) = dialogs.next().await.context("iterating dialogs")? {
        let peer = dialog.peer();
        if peer.id().bot_api_dialog_id() == marked {
            return peer
                .to_ref()
                .await
                .ok_or_else(|| anyhow!("channel {marked} is not usable"));
        }
    }
    Err(anyhow!(
        "channel id {marked} not found in your dialogs (the account must be a member of it)"
    ))
}

/// Fetch a message and download its media to `output`.
///
/// When `discussion_message_id` is `Some`, follows the discussion-group link:
/// calls `GetDiscussionMessage` on the channel post to find the linked
/// discussion group, then fetches `discussion_message_id` from that group.
///
/// Returns `false` when the target message has no downloadable media.
pub async fn download(
    client: &Client,
    peer: PeerRef,
    message_id: i32,
    output: &str,
    discussion_message_id: Option<i32>,
) -> Result<bool> {
    let messages = client
        .get_messages_by_id(peer.clone(), &[message_id])
        .await
        .with_context(|| format!("fetching message {message_id}"))?;
    let channel_message = messages
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| anyhow!("message {message_id} not found"))?;

    let target_message = if let Some(dm_id) = discussion_message_id {
        resolve_discussion_message(client, peer, &channel_message, dm_id).await?
    } else {
        channel_message
    };

    let Some(media) = target_message.media() else {
        return Ok(false);
    };

    client
        .download_media(&media, output)
        .await
        .with_context(|| format!("downloading media to '{output}'"))?;
    Ok(true)
}

/// Given a channel post that has a linked discussion group, fetches
/// `discussion_message_id` from that discussion group and returns it.
async fn resolve_discussion_message(
    client: &Client,
    peer: PeerRef,
    channel_message: &Message,
    discussion_message_id: i32,
) -> Result<Message> {
    // Check that comments are enabled on this post (warn but still try).
    let has_comments = if let tl::enums::Message::Message(ref raw) = channel_message.raw {
        raw.replies
            .as_ref()
            .map(|r| match r {
                tl::enums::MessageReplies::Replies(mr) => mr.comments,
            })
            .unwrap_or(false)
    } else {
        false
    };
    if !has_comments {
        warn!(
            message_id = channel_message.id(),
            "message has no comments enabled; GetDiscussionMessage may fail"
        );
    }

    let input_peer: tl::enums::InputPeer = (&peer).into();
    let discussion = client
        .invoke(&tl::functions::messages::GetDiscussionMessage {
            peer: input_peer,
            msg_id: channel_message.id(),
        })
        .await
        .context("calling GetDiscussionMessage")?;

    let discussion: tl::types::messages::DiscussionMessage = discussion.into();

    // messages[0] is the linked post inside the discussion group; its peer_id
    // identifies the discussion channel.
    let linked_msg = discussion
        .messages
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("GetDiscussionMessage returned no messages"))?;

    let discussion_channel_id = match &linked_msg {
        tl::enums::Message::Message(m) => match &m.peer_id {
            tl::enums::Peer::Channel(pc) => pc.channel_id,
            other => {
                return Err(anyhow!(
                    "unexpected peer type in discussion message: {other:?}"
                ));
            }
        },
        other => {
            return Err(anyhow!(
                "unexpected message type from discussion: {other:?}"
            ));
        }
    };

    // Find the access_hash for that channel from the chats list.
    let access_hash = discussion
        .chats
        .iter()
        .find_map(|chat| {
            if let tl::enums::Chat::Channel(ch) = chat {
                if ch.id == discussion_channel_id {
                    return ch.access_hash;
                }
            }
            None
        })
        .ok_or_else(|| {
            anyhow!("discussion channel {discussion_channel_id} not found in chats list")
        })?;

    let discussion_peer =
        PeerRef::from(tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
            channel_id: discussion_channel_id,
            access_hash,
        }));

    let msgs = client
        .get_messages_by_id(discussion_peer, &[discussion_message_id])
        .await
        .with_context(|| format!("fetching discussion message {discussion_message_id}"))?;

    msgs.into_iter()
        .next()
        .flatten()
        .ok_or_else(|| anyhow!("discussion message {discussion_message_id} not found"))
}

/// Infer the file type from the freshly downloaded file and, if recognized,
/// rename it with the inferred extension appended (mirroring the Python tool).
/// Returns the new path when a rename happened.
pub fn apply_inferred_extension(path: &str) -> Result<Option<String>> {
    let kind = infer::get_from_path(path)
        .with_context(|| format!("reading '{path}' for type inference"))?;
    match kind {
        Some(kind) => {
            let new_path = format!("{path}.{}", kind.extension());
            std::fs::rename(path, &new_path)
                .with_context(|| format!("renaming '{path}' to '{new_path}'"))?;
            Ok(Some(new_path))
        }
        None => Ok(None),
    }
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush().context("flushing stdout")?;
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .context("reading from stdin")?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}
