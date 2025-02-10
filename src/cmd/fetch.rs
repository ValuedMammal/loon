use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time;

use loon::CallTy;
use loon::ChatEntry;
use loon::Coordinator;

use super::nostr::{EventId, Filter, Kind, PublicKey, Timestamp};
use super::Result;

/// How far to look back in time when polling the relay, currently one fortnight.
const DEFAULT_LOOKBACK: u64 = 14 * 24 * 60 * 60;

/// Encrypted content and author keyed by `EventId`.
type RawEntries = HashMap<EventId, (PublicKey, String)>;

/// Fetch latest notes by quorum parties, printing results to stdout.
pub async fn fetch_and_decrypt(coordinator: &Coordinator) -> Result<()> {
    let raw_entries = fetch_raw_entries(coordinator).await?;
    let entries = decrypt_raw_entries(coordinator, raw_entries.values().cloned()).await?;
    for entry in entries {
        println!("{}: {}", entry.alias, entry.message);
    }
    Ok(())
}

/// Fetch events from quorum participants.
async fn fetch_raw_entries(coordinator: &Coordinator) -> Result<RawEntries> {
    let client = coordinator.client().expect("must have client");
    client.connect().await;
    let mut entries = RawEntries::new();

    let subs: Vec<Filter> = coordinator
        .participants()
        .map(|(_id, p)| {
            Filter::new()
                .author(p.pk)
                .since((Timestamp::now().as_u64() - DEFAULT_LOOKBACK).into())
        })
        .collect();

    let events = client.fetch_events(subs, super::TIMEOUT).await?;

    for event in events {
        if let Kind::TextNote = event.kind {
            entries.insert(event.id, (event.pubkey, event.content));
        }
    }

    Ok(entries)
}

/// Decrypt nip44.
async fn decrypt_raw_entries(
    coordinator: &Coordinator,
    messages: impl IntoIterator<Item = (PublicKey, String)>,
) -> Result<Vec<ChatEntry>> {
    let signer = coordinator.signer().await?;
    let mut ret = vec![];

    // If we see HRP, we read the message fingerprint and check if it matches the current
    // quorum's FP. When a match is found, we derive the participant from the parsed PID.
    // If the derived participant's PK matches the current user's PK, we reconstruct the
    // conversation key according to nip44 and decrypt.
    for (pk, message) in messages {
        let alias = coordinator
            .participants()
            .filter(|(_id, p)| p.pk == pk)
            .map(|(_, p)| p.alias.clone())
            .next()
            .expect("we subscribed to the pk")
            .unwrap_or_default();
        if !message.starts_with(loon::HRP) {
            ret.push(ChatEntry { alias, message });
            continue;
        }

        // parse quorum FP
        let quorum_fp = &message[5..13];
        if quorum_fp == coordinator.quorum_fingerprint() {
            // parse two-digit pid, e.g. '02'
            let quid: u32 = message[13..15].parse()?;

            // derive recipient p from quorum id
            // we should get one because the message fp matches the active quorum
            let participant = coordinator
                .participants()
                .find(|(pid, _p)| pid.as_u32() == quid);

            if let Some((_pid, participant)) = participant {
                // parse payload for the intended recipient
                if participant.pk == signer.get_public_key().await? {
                    assert!(message.len() > 15);
                    let payload = &message[15..];
                    let res = match payload {
                        "0" => CallTy::Nack,
                        "1" => CallTy::Ack,
                        _ => {
                            let decoded = signer.nip44_decrypt(&pk, payload).await?;
                            CallTy::Note(decoded)
                        }
                    };
                    ret.push(ChatEntry {
                        alias,
                        message: res.to_string(),
                    });
                }
            }
        }
    }

    Ok(ret)
}

/// Listens for incoming calls, and writes to a log file.
// or write to database?
pub async fn listen(coordinator: &Coordinator) -> Result<()> {
    let cargo_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/chat.log", cargo_dir);
    let mut f = fs::File::options().append(true).open(&path).await?;

    // keep track of events seen
    let mut event_ids = HashSet::<EventId>::new();

    loop {
        let raw_entries = fetch_raw_entries(coordinator).await?;

        // only log new events
        if !raw_entries.is_empty() {
            let raw_entries_iter = raw_entries.into_iter().filter_map(|(event, entry)| {
                if event_ids.contains(&event) {
                    None
                } else {
                    event_ids.insert(event);
                    Some(entry)
                }
            });
            let chat_entries = decrypt_raw_entries(coordinator, raw_entries_iter).await?;
            for entry in chat_entries {
                let content = match entry.message.as_bytes() {
                    b if b.len() < 1024 => format!("{}: {}\n", entry.alias, entry.message),
                    _ => "message too long, skipping\n".to_string(),
                };
                let _ = f.write(content.as_bytes()).await?;
            }
        }

        // refresh on 10s interval
        time::sleep(Duration::from_secs(10)).await;
    }
}

/// Fetch events from quorum participants.
#[allow(unused)]
pub async fn fetch(coordinator: Coordinator) -> Result<()> {
    let client = coordinator.client().expect("must have client");
    client.connect().await;

    let subs = Filter::new()
        .authors(coordinator.participants().map(|(_id, p)| p.pk))
        .since((Timestamp::now().as_u64() - DEFAULT_LOOKBACK).into());
    let events = client.fetch_events(vec![subs], super::TIMEOUT).await?;
    for event in events {
        if let Kind::TextNote = event.kind {
            println!("{}", event.content);
        }
    }

    Ok(())
}
