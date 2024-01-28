use std::fmt;

mod coordinator;
pub mod db;

pub use coordinator::Coordinator;
pub use coordinator::CallTy;
pub use coordinator::Call;
pub use coordinator::Participant;
pub use coordinator::HRP;
pub use nostr_sdk::prelude as nostr;

/// Crate errors.
#[derive(Debug)]
pub enum Error {
    /// Builder
    Builder,
    /// Coordinator
    Coordinator(String),
    /// Nostr client
    Nostr(nostr_sdk::client::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builder => write!(f, "not all required fields present"),
            Self::Coordinator(e) => e.fmt(f),
            Self::Nostr(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod test {
    //use super::*;
    use nostr_sdk::prelude::*;

    #[test]
    fn encode_decode() {
        // call --encrypt --dryrun --to <recipient> --note <content>
        // $ cargo run -- call -e -d -t 'chicken' -m 'hello world'
        // A -> B
        let m = "loon14795dc9101AuKpleIcx2+uVvC2SAIXndsGWRMQ8ISqMoUcmM2MfqAVDYiwjYv50mGgHOQxmLctGYoo0/GAqJTcC6HwSOCmvjmhqFFJa1tgYQ9F373eMr/Ds+p7IIKCdUWoZYMt0t6KdymO".to_string();
        let k = Keys::from_sk_str(&std::env::var("NOSTR_NSEC_A").unwrap()).unwrap();
        let sk1 = k.secret_key().unwrap();
        let pk1 = k.public_key();
        let k = Keys::from_sk_str(&std::env::var("NOSTR_NSEC_B").unwrap()).unwrap();
        let sk2 = k.secret_key().unwrap();
        let pk2 = k.public_key();
        let _conv = nip44::v2::ConversationKey::derive(&sk1, &pk2);

        let hrp = &m[..5];
        assert_eq!(hrp, "loon1");

        let qfp = &m[5..13];
        assert_eq!(qfp, "4795dc91");

        let quid = &m[13..15];
        assert_eq!(quid, "01");

        let payload = &m[15..];
        let res = nip44::decrypt(&sk2, &pk1, payload).unwrap();
        assert_eq!(res, "hello world");
    }
}
