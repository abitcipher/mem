import Config

# A local hub for development and the cross-language test (MEM_HUB_URL in mem-runtime).
if port = System.get_env("MEM_HUB_PORT"), do: config(:mem_hub, port: String.to_integer(port))
if db = System.get_env("MEM_HUB_DB"), do: config(:mem_hub, MemHub.Repo, database: db)

# Push wake-ups to a distributor on this machine (development only; never in prod).
if System.get_env("MEM_HUB_ALLOW_LOOPBACK_PUSH") == "1",
  do: config(:mem_hub, allow_loopback_push: true)

# The hub network, for local multi-hub tests.
if key = System.get_env("MEM_HUB_KEY"),
  do:
    config(:mem_hub,
      network: %{
        key_file: key,
        list_file: System.fetch_env!("MEM_HUB_LIST"),
        operator_keys: String.split(System.fetch_env!("MEM_HUB_OPERATOR_KEYS"), ",")
      }
    )
