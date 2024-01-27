use loon::CallTy;
use loon::Coordinator;

use super::nostr::{ClientSigner, Filter, Kind, Timestamp, XOnlyPublicKey};
use super::nostr::nip44;
use super::Result;
use super::bail;
use crate::cli::CallOpt;
use crate::cli::Recipient;

/// How far to look back in time when polling the relay, currently one fortnight.
const DEFAULT_LOOKBACK: u64 = 14 * 24 * 60 * 60;

/// Post a nostr note.
pub async fn post(coordinator: Coordinator<'_>, params: CallOpt) -> Result<()> {
    let CallOpt {
        recipient,
        encrypt,
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
            if encrypt {
                // nip44 encrypt
                let signer = coordinator
                    .messenger()
                    .expect("msg client must be init")
                    .signer()
                    .await?;
                let my_sec = match signer {
                    ClientSigner::Keys(k) => k.secret_key()?,
                    _ => panic!("only keys signers allowed"),
                };

                let conversation_key = nip44::v2::ConversationKey::derive(&my_sec, &p.pk);
                nip44::v2::encrypt(&conversation_key, note)?
            } else {
                // plain text
                note
            }
        }
    };

    // send it
    let call = coordinator.call_new_with_recipient_and_payload(p.quorum_id, &payload);
    let client = coordinator.messenger().expect("messenger must be init");
    client.connect().await;

    if params.dryrun {
        println!("Preview: {:#?}", &call);
    } else {
        let event_id = client.publish_text_note(call.to_string(), None).await?;
        println!("Sent: {}", event_id);
    }

    Ok(())
}

/// Fetch events from quorum participants.
pub async fn fetch_and_decrypt(coordinator: Coordinator<'_>) -> Result<()> {
    let client = coordinator.messenger().unwrap();
    let k = match client.signer().await? {
        ClientSigner::Keys(k) => k,
        _ => panic!("only keys signers allowed"),
    };
    let my_sec = k.secret_key()?;

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
    for (pk, message) in messages {
        if !message.starts_with(loon::HRP) {
            // TODO: get alias from author pk
            println!("{}", message);
            return Ok(());
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
                // proceed if we are the intended recipient
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

                    println!("{}", res);
                }
            }
        }
    }

    Ok(())
}

/// Fetch events from quorum participants.
#[allow(dead_code)]
pub async fn fetch(coordinator: Coordinator<'_>) -> Result<()> {
    let client = coordinator.messenger().unwrap();
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
