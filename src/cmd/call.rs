use loon::CallTy;
use loon::Coordinator;

use super::nostr::nip44;
use super::Result;
use super::bail;
use crate::cli::CallOpt;
use crate::cli::CallSubCmd;
use crate::cli::Recipient;

/// Push notes.
pub async fn push(coordinator: Coordinator, cmd: CallSubCmd) -> Result<()> {
    match cmd {
        // Push a plain text note.
        CallSubCmd::Push { note } => {
            let client = coordinator.messenger();
            client.connect().await;
            let event_id = client.publish_text_note(note, None).await?;
            println!("Sent: {}", event_id);
        }
        // Push an encrypted payload to a desginated recipient.
        CallSubCmd::New(params) => {
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
        }
    }

    Ok(())
}
