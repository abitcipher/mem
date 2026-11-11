import Config

# Defaults; an operator can change any of them per environment.
config :mem_hub,
  ecto_repos: [MemHub.Repo],
  port: 4000,
  start_server: true,
  limits: %{
    # Largest bundle.
    max_bundle: 65_536,
    # Per destination tag (as couriers).
    per_tag: 512 * 1024,
    # Intro bundles: per intro tag and in total.
    intro_per_tag: 64 * 1024,
    intro_total: 8 * 1024 * 1024,
    # Everything stored.
    total: 1024 * 1024 * 1024,
    # Of that, bundles from partner operators' hubs.
    foreign_total: 256 * 1024 * 1024,
    # Uploads per minute and open sockets per client address.
    puts_per_minute: 60,
    sockets_per_ip: 16,
    # Bytes sent for one fetch's backlog.
    fetch_bytes: 4 * 1024 * 1024
  }

config :mem_hub, MemHub.Repo,
  database: Path.expand("../mem_hub.db", __DIR__),
  pool_size: 5,
  journal_mode: :wal

# Logs never carry tags, prefixes, tokens or endpoints.
config :logger, level: :info

import_config "#{config_env()}.exs"
