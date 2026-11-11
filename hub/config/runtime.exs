import Config

if config_env() == :prod do
  config :mem_hub, port: String.to_integer(System.get_env("MEM_HUB_PORT", "4000"))

  config :mem_hub, MemHub.Repo,
    database: System.get_env("MEM_HUB_DB", "/var/lib/mem_hub/mem_hub.db")

  # Reverse proxies allowed to name the client in `x-real-ip` (comma-separated); none by default.
  trusted =
    case MemHub.ClientIp.parse_list(System.get_env("MEM_HUB_TRUSTED_PROXIES", "")) do
      {:ok, ips} -> ips
      {:error, {:bad_address, entry}} -> raise "MEM_HUB_TRUSTED_PROXIES: bad address #{entry}"
    end

  config :mem_hub, trusted_proxies: trusted
end
