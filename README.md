# Loon Wallet
Coordination at a distance

<div align="center">
    <img src="./doc/logo.jpg?raw=true">
    <!-- <img src="./doc/logo.jpg" width="220" /> -->
</div>


## Getting started

This is a simple CLI application using bdk, nostr-sdk, clap and tokio. Note carefully the following requirements.

### Requirements

- A local bitcoind configured with `-blockfilterindex`.
- These environment variables must be set
    - `RPC_COOKIE` - Path to bitcoind cookie file for communicating over RPC, e.g. `/home/satoshi/.bitcoin/.cookie`
    - `NOSTR_NSEC` - To sign nostr events
- Sqlite database, i.e. `loon.db`. See the [schema](./schema.sql).

## Features

* `nostr-sdk`: (optional) Used to send and receive notes via a nostr relay.

## Example

```sh
Usage: loon [OPTIONS] <COMMAND>

Commands:
  call      Push notes
  db        Database operations
  desc      Descriptors operations
  fetch     Fetch notes from quorum participants
  hash      Get best block hash
  generate  Generate a keypair
  wallet    Wallet operations
  help      Print this message or the help of the given subcommand(s)

Options:
  -a, --account-id <ACCOUNT_ID>  Account id
  -h, --help                     Print help
  -V, --version                  Print version
```
