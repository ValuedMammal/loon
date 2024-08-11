# Loon Wallet
Coordination at a distance

<div align="center">
    <img src="./doc/logo.jpg?raw=true">
    <!-- <img src="./doc/logo.jpg" width="220" /> -->
</div>


## Quick Start
The app uses BDK, nostr-sdk, clap, and tokio. Right now it's a simple CLI made for individual power users and not necessarily for mass consumption. Note carefully the following requirements.

### Requirements
- A local bitcoind configured with `-blockfilterindex` and `-peerblockfilters`
- These environment variables must be set
    - `RPC_COOKIE` - path to bitcoind cookie file for communicating over RPC, e.g. `/home/satoshi/.bitcoin/.cookie`
    - `NOSTR_NSEC` - to sign/publish nostr events
- Sqlite data store, i.e. `loon.db`. See the [schema](./schema.sql).

### Limitations
We use [a fork of BDK](https://github.com/ValuedMammal/bdk/tree/feat/bitcoind-rpc-filter) that supports compact block filter (CBF) sync via Bitcoin Core RPC. While multiple efforts are underway for BDK to officially support CBF, it's not certain whether the implementation used by Loon will ever be adopted/merged.

## Features
|Status|Task|
|:----:|--------|
|✅ | Finalize [v1 spec](./doc/specification.md) |
|_ | Demo user flow |
|_ | Design a policy/quorum builder |
|✅ | Get descriptor info |
|✅ | Import descriptors |
|✅ | Emit SQL via CLI |
|✅ | Publish notes |
|✅ | Fetch notes from quorum participants |
|✅ | nip44 encrypt/decrypt |
|✅ | Generate nostr keys |
|✅ | Use multipath descriptors |
|✅ | Wallet sync |
|✅ | Get addresses |
|✅ | Create PSBTs |
|✅ | List wallet transactions |
|✅ | Support mainnet, signet |
|_ | Send raw transaction |
