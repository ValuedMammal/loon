use loon::CallTy;
use loon::Coordinator;

use nostr_sdk::{EventBuilder, Kind};

use super::bail;
use super::Result;
use crate::cli::CallOpt;
use crate::cli::CallSubCmd;
use crate::cli::Recipient;

/// Push notes.
pub async fn push(coordinator: &Coordinator, cmd: CallSubCmd) -> Result<()> {
    match cmd {
        // Push a plain text note.
        CallSubCmd::Push { note } => {
            let client = coordinator.client();
            client.connect().await;
            let event = client
                .send_event_builder(EventBuilder::new(Kind::TextNote, note))
                .await?;
            println!("Sent: {}", event.id());
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
                bail!("no recipient found");
            }

            let p = if let Some(id) = id {
                coordinator
                    .participants
                    .get(&id.into())
                    .ok_or(anyhow::anyhow!("unknown participant id {}", id))?
            } else {
                let alias = alias.expect("must have alias");
                coordinator
                    .participants()
                    .find(|(_, p)| p.alias.as_ref() == Some(&alias))
                    .map(|(_, p)| p)
                    .ok_or(anyhow::anyhow!("unknown participant {}", alias))?
            };

            // parse params into a payload
            let payload = if nack {
                CallTy::Nack.id().to_string()
            } else if ack {
                CallTy::Ack.id().to_string()
            } else {
                // text note
                match note {
                    Some(s) if !s.trim().is_empty() => {
                        // nip44 encrypt
                        coordinator.signer().await?.nip44_encrypt(&p.pk, &s).await?
                    }
                    _ => bail!("no message provided"),
                }
            };

            let call = coordinator.call_new_with_recipient_and_payload(p.quorum_id, &payload);

            // Send it
            if params.dryrun {
                println!("Preview: {:#?}", &call);
            } else {
                let client = coordinator.client();
                client.connect().await;
                let event = client
                    .send_event_builder(EventBuilder::new(Kind::TextNote, call.to_string()))
                    .await?;
                println!("Sent: {}", event.id());
            }
        }
    }

    Ok(())
}
