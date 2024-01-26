# App Specification

## Architecture
- serde
- sqlite
- bitcoincore-rpc
- bdk, bitcoin, miniscript
- clap
- nostr-sdk

## Schematic
<img src="./schematic.jpg?raw=true">

## Requirements
In general, the implementation should be as high level as possible, relying on various working components noted in the architecture section above. In addition, we introduce the following new interfaces:

### Coordinator

The main structure is the `Coordinator` who's responsible for managing a collection of watch-only wallets. The coordinator provides a container for a bdk wallet, an spv client, and a messaging client. Each wallet/account has a public descriptor and spending policy that may be single- or multi-sig. These can be stored on disk in an SQL table like so:
```
CREATE TABLE account (
    id INTEGER PRIMARY KEY,
    nick TEXT,
    descriptor BLOB
);
```

| id | nick | descriptor |
|--|--|--|
| 01 | my_wallet | `wpkh([deadbeef/84'/0'/0']xpub.../<0;1>/*)` |

Importantly, the coordinator doesn't store or generate signing keys. It only has knowledge of public data from which it may derive addresses. Serving as an abstraction over a bdk `Wallet`, the coordinator can perform common tasks like querying a chain source for block data, crunching balances, and crafting new PSBTs to be exported for processing by a signer.

A typical user flow would look like:

1. User imports public descriptors
2. Wallet scans a chain source for tx data
3. User selects inputs to a new transaction
4. Wallet creates a transaction to be signed
5. Wallet broadcasts signed transactions

### Messenger
    
The app facilitates communication between participants in a multisig quorum, with communication handled over nostr. The first candidate for a message client is [nostr-sdk](https://docs.rs/nostr-sdk) crate. The user brings their own nostr keypair. As is typical of any message client, functions include creating posts and subscribing to the feed of other accounts. In our context, messages are generally brief and serve primarily as anti-phishing protection. The application need only be aware of correspondence between parties in a quorum defined by an account being watched by the coordinator. All communication is private by default, which can be accomplished through one of a number of encryption schemes such as PGP, ECDH, etc[^1].

Messages, referred to as `loon_call`s, are specified as follows:

1. The message is prefixed with a **human-readable part (hrp)** that identifies it as a Loon call, e.g. `loon1`.
2. After the prefix is a **fixed-length fingerprint** that encodes the identity of the wallet quorum and the recipient. So, to identify participant `01` as the recipient of a message in a quorum with (hex) fingerprint `deadbeef`, this part will thus be comprised of 5 bytes.
3. Finally **the payload** is appended to the end, appearing in cipher text which may be of variable length. The resulting format thus appears like so:  
`loon1deadbeef01<cipher-text>`

To illustrate the messaging flow, consider two participants Alice and Bob:  

1. We assume each party has knowledge of the other's pubkey.
2. Now Alice wants to send a message to Bob. For instance she may have a partially-signed transaction and is requesting interaction on the part of Bob. She encrypts the data with Bob's pubkey, constructs the call as specified above, and posts to **her own nostr feed**. 
    - Alice may optionally decide to sign the fully constructed message with her own key to prove authenticity. (I believe this is the default behavior in the case of nostr).
3. The message thus appears as an encrypted blob on the server, and since the coordinator is aware of how messages are constructed (specifically, by looking at the combination of the hrp + wallet fingerprint), it can easily recognize posts that are salient given the details of a watched wallet.
4. Bob's coordinator periodically fetches posts from known participants, and when the coordinator sees a message destined for Bob, the message is stored locally and Bob receives an alert in his inbox. He decrypts the blob with his own key using his preferred method.
    - In the case the message is accompanied by a signature from Alice, Bob takes the additional step of verifying the signature against Alice's pubkey.
5. When Bob is satisfied that the message is authentic, he can either respond with a call of his own (mirroring Alice's flow), or he can save the message for later and finally discard it when finished.

Open questions:  
1. Is the normal messaging flow the same mechanism used for passing PSBTs? sure, why not.
2. Does the message and coordination flow allow for constructing new wallets with new participants, or must this be handled "out of band"? 
3. How does the app handle the notion of time when it comes to messages? That is,
    - Does it matter in which order messages are sent and received?
    - How do we know when a message should be considered stale or invalid? 
    - Does a sequence of messages make up an append-only log, or are they treated as timeless and ephemeral?

[^1]: methods of encryption  
  Easy: encrypted DM nip04  
  Normal: nip44 versioned encryption  
  Paranoid: gpg encrypted file upload  

## Should haves
- Nostr rust relay for development purposes, may also consider monetizing a hosted relay.
- Testnet/signet capable
- Extra facilities
    - import descriptors to Bitcoin Core
    - sign/verify messages using Core's built in functionality
- [Loon logo](./logo.jpg)

## Wishlist
- Consider what it would take to integrate a simpleX client. What advantages or limitations does it pose as a message client over nostr?
- Consider desktop/mobile/web UI. Don't assume users will want to use the cli.
