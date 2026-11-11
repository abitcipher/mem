import Config

config :mem_hub, start_server: false
config :mem_hub, MemHub.Repo, database: Path.expand("../mem_hub_test.db", __DIR__), pool_size: 1
config :logger, level: :warning
config :mem_hub, allow_loopback_push: true
config :mem_hub, wake_interval_ms: 1_000
