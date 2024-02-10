use std::time::Duration;
use tokio::time;

use loon::CallTy;
use loon::Coordinator;

use super::nostr::{Filter, Kind, Timestamp, XOnlyPublicKey};
use super::nostr::nip44;
use super::Result;
use super::bail;
use crate::cli::CallOpt;
use crate::cli::Recipient;

/// How far to look back in time when polling the relay, currently one fortnight.
const DEFAULT_LOOKBACK: u64 = 14 * 24 * 60 * 60;

/// Push an encrypted payload to a desginated recipient.
pub async fn push_with_options(coordinator: Coordinator<'_>, params: CallOpt) -> Result<()> {
    let CallOpt {
        recipient,
        note,
        ack,
        nack,
        ..
    } = params;

    // Get the recipient
    let Recipient { id, alias } = recipient;
    if let (None, None) = (id, alias.as_ref()) {
        bail!("no recipient found")
    };

    let p = if let Some(id) = id {
        coordinator.get(id).unwrap()
    } else {
        coordinator
            .participants()
            .find(|(&_pid, p)| p.alias == alias)
            .map(|(_, p)| p)
            .unwrap()
    };

    // parse params into a payload
    let payload = {
        if nack {
            CallTy::Nack.id().to_string()
        } else if ack {
            CallTy::Ack.id().to_string()
        } else {
            // text note
            let note = match note {
                Some(n) if !n.trim().is_empty() => n,
                _ => bail!("no message provided"),
            };

            // nip44 encrypt
            let my_sec = coordinator.keys().await?.secret_key()?;
            let conversation_key = nip44::v2::ConversationKey::derive(&my_sec, &p.pk);
            nip44::v2::encrypt(&conversation_key, note)?
        }
    };

    // send it
    let call = coordinator.call_new_with_recipient_and_payload(p.quorum_id, &payload);
    let client = coordinator.messenger();
    client.connect().await;

    if params.dryrun {
        println!("Preview: {:#?}", &call);
    } else {
        let event_id = client.publish_text_note(call.to_string(), None).await?;
        println!("Sent: {}", event_id);
    }

    Ok(())
}

/// Push a plain text note.
pub async fn push(coordinator: Coordinator<'_>, note: &str) -> Result<()> {
    let client = coordinator.messenger();
    client.connect().await;
    let event_id = client.publish_text_note(note, None).await?;
    println!("Sent: {}", event_id);
    Ok(())
}

/// Fetch events from quorum participants.
pub async fn fetch_and_decrypt(coordinator: &Coordinator<'_>) -> Result<()> {
    let client = coordinator.messenger();
    client.connect().await;

    let subs = Filter::new()
        .authors(coordinator.participants().map(|(_id, p)| p.pk))
        .since((Timestamp::now().as_u64() - DEFAULT_LOOKBACK).into());
    let events = client
        .get_events_of(vec![subs], Some(super::TIMEOUT))
        .await?;
    let messages: Vec<(XOnlyPublicKey, String)> = events
        .iter()
        .filter(|event| matches!(event.kind, Kind::TextNote))
        .map(|e| {
            let author = e.author();
            let content = e.content().to_owned();
            (author, content)
        })
        .collect();

    // Decrypt nip44
    // If we see HRP, we read the message fingerprint and check if it matches the current
    // quorum's fp. When a match is found, we derive the participant from the parsed pid.
    // If the derived participant's pk matches the current user's pk, we reconstruct the
    // conversation key according to nip44 and decrypt.
    let k = coordinator.keys().await?;
    let my_sec = k.secret_key()?;

    for (pk, message) in messages {
        let alias = coordinator
            .participants()
            .filter(|(_id, p)| p.pk == pk)
            .map(|(_, p)| p.alias.clone())
            .next()
            .expect("we subscribed to the pk")
            .unwrap_or_default();
        if !message.starts_with(loon::HRP) {
            println!("{}: {}", alias, message);
            continue;
        }

        // parse quorum fp
        let qfp = &message[5..13];
        if qfp == coordinator.quorum_fingerprint().as_str() {
            // parse two-digit pid, e.g. '02'
            let quid: u32 = message[13..15].parse()?;

            // derive recipient p from quorum id
            // we should get one because the message fp matches the active quorum
            let p = coordinator
                .participants()
                .find(|(pid, _p)| pid.as_u32() == quid);

            if let Some((_pid, p)) = p {
                // parse payload for the intended recipient
                if p.pk == k.public_key() {
                    assert!(message.len() > 15);
                    let payload = &message[15..];
                    let res = match payload {
                        "0" => CallTy::Nack,
                        "1" => CallTy::Ack,
                        _ => {
                            // reconstruct the conversation key
                            // `nip44::v2::decrypt` requires that we base64 decode the payload
                            //let conv = nip44::v2::ConversationKey::derive(&my_sec, &pk);
                            //let data: Vec<u8> = general_purpose::STANDARD.decode(payload)?;
                            //let res = nip44::v2::decrypt(&conv, data)?;
                            // alternatively,
                            let m = nip44::decrypt(&my_sec, &pk, payload)?;
                            CallTy::Note(m)
                        }
                    };

                    println!("{}: {}", alias, res);
                }
            }
        }
    }

    Ok(())
}

/// Listens for incoming calls, and writes to a log file (or database?).
pub async fn listen(coordinator: &Coordinator<'_>) -> Result<()> {
    loop {
        fetch_and_decrypt(coordinator).await?;

        // TODO filter already seen event ids
        time::sleep(Duration::from_secs(7)).await;
    }
}

/// Fetch events from quorum participants.
#[allow(dead_code)]
pub async fn fetch(coordinator: Coordinator<'_>) -> Result<()> {
    let client = coordinator.messenger();
    client.connect().await;

    let subs = Filter::new()
        .authors(coordinator.participants().map(|(_id, p)| p.pk))
        .since((Timestamp::now().as_u64() - DEFAULT_LOOKBACK).into());
    let events = client
        .get_events_of(vec![subs], Some(super::TIMEOUT))
        .await?;
    for event in events {
        if matches!(event.kind, Kind::TextNote) {
            println!("{}", event.content);
        }
    }

    Ok(())
}
