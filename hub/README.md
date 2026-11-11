# mem_hub — the MEM hub

A dumb, replaceable P2 mailbox. It stores sealed envelopes by
destination tag. It hands them to whoever fetches their 2-byte tag prefix. It deletes them only
when shown their purge token or when they expire. It has no accounts and never sees content.
Anyone can run one.

## What it is made of

- **Bandit + WebSock** serve one endpoint, `GET /v1`, a WebSocket of binary frames. Each frame
  holds one CBOR hub message (`spec/hub.cddl`). It does not use Phoenix Channels, because their
  frames are JSON. `GET /health` answers `ok`.
- **Rustler NIF** (`native/mem_hub_nif`) runs `mem-proto` and `mem-crypto`. These are the same
  bounded, fuzzed decoders and proof-of-work check the apps use. Proof of work runs on a dirty CPU
  scheduler.
- **SQLite** through Ecto (`ecto_sqlite3`), so a community hub is one process and one file.
  Postgres can be added as another Ecto adapter.
- **Quotas** (`config/config.exs`): 64 KiB per bundle, 512 KiB per tag, intro bundles 64 KiB
  per tag and 8 MiB in total, a total store limit, 60 uploads per minute and 16 sockets per
  client address, and 4 MiB per fetch backlog.
- **Logs** never contain tags, prefixes, tokens or push endpoints.

## Run it

```sh
mix deps.get
mix test
MIX_ENV=prod MEM_HUB_PORT=4000 MEM_HUB_DB=/var/lib/mem_hub/mem_hub.db mix run --no-halt
```

Put TLS in front of it (Caddy, nginx, or a load balancer). Apps connect only to `wss://`
addresses; plain `ws://` is accepted only for a hub on the same device (development).

The cross-language test runs the Rust nodes against this hub:

```sh
MIX_ENV=dev MEM_HUB_PORT=4111 MEM_HUB_DB=/tmp/hub.db mix run --no-halt &
MEM_HUB_URL=ws://127.0.0.1:4111/v1 cargo test -p mem-runtime two_nodes_deliver_through_the_hub
```

## Dependencies and licences

All dependencies are MIT or Apache-2.0. `exqlite` bundles SQLite, which is public domain. By
default `exqlite` downloads a precompiled NIF. Release builds should set
`EXQLITE_FORCE_BUILD=true` to compile it from source.

## Push wake-ups

Clients may register a UnifiedPush endpoint. When an upload lands under one of the endpoint's
prefixes, the hub POSTs a fixed one-byte body (`w`) to it, at most once a minute. The hub only calls `https`
endpoints whose host resolves to a public IPv4 address, and checks again before every POST.
Because the hub calls addresses that clients chose, operators should also block private and
link-local ranges for its outbound traffic (DNS rebinding between the check and the request).

## A network of hubs

Several hubs run by one operator act as one mailbox: each pulls every other hub's log of new
bundles and purges, so a phone can use any of them and nothing is lost when one goes down.

1. The operator makes the operator key once, offline, and keeps it there:
   `cargo run -p mem-hublist -- keygen operator.seed` (prints the public key; it goes into the
   app's `OPERATOR_KEYS` in `crates/mem-runtime/src/node.rs`).
2. Each hub gets its own key: `mem-hublist keygen hub.seed`.
3. The operator signs the list:
   `mem-hublist sign operator.seed <serial> <days> hubs.cbor wss://hub-a.example/v1=<key>@eu ...`
   (a higher serial for every new list; at most 365 days).
4. Each hub is configured with its seed, the list and the operator's public key:

   ```elixir
   config :mem_hub, network: %{key_file: "/data/hub.seed", list_file: "/data/hubs.cbor",
                               operator_keys: ["<operator key hex>"]}
   ```

   The hub reads the list again every hour. To add or remove a hub, copy a new signed list to
   every hub. A hub whose list is missing, expired or does not name it runs alone.

A hub's database belongs to its key: a hub that loses its database must get a new key (and a
new list), or its peers would skip what it logs next.

## Partner operators

Two operators can exchange mail between their networks. Each names the other's hubs as
partners in its own signed list:

```
mem-hublist sign operator.seed <serial> <days> hubs.cbor wss://hub-a.example/v1=<key> ... \
  "partner:wss://hub.other.example/v1=<their hub key>@Other operator"
```

A partner hub authenticates like an own hub but gets only bundles whose sender let them cross
(the app allows it when the contact's privacy floor permits P3 and the user has not chosen
"Only my operator's hubs"), never bundles that came from a partner themselves, and every purge
token. What a hub receives from a partner is limited by `foreign_total` (a quarter of `total`
by default) and is marked for clients, whose app shows it as P3. Both operators must list each
other; either can stop by issuing a list without the other.

`hub/scripts/network-test.sh` runs three hubs on this machine and checks delivery between
them and catch-up of a hub started late, then a fourth hub of a second operator and delivery
across the two operators.

## Not yet

- MLS delivery service, the iroh relay, and Privacy Pass rate limiting.
