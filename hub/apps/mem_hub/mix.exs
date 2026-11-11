defmodule MemHub.MixProject do
  use Mix.Project

  def project do
    [
      app: :mem_hub,
      version: "0.1.0",
      build_path: "../../_build",
      config_path: "../../config/config.exs",
      deps_path: "../../deps",
      lockfile: "../../mix.lock",
      elixir: "~> 1.20",
      start_permanent: Mix.env() == :prod,
      elixirc_paths: elixirc_paths(Mix.env()),
      deps: deps()
    ]
  end

  # Run "mix help compile.app" to learn about applications.
  def application do
    [
      extra_applications: [:logger, :inets, :ssl, :public_key],
      mod: {MemHub.Application, []}
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  # Each dependency is permissively licensed (MIT or Apache-2.0); see hub/README.md.
  defp deps do
    [
      # HTTP and the plain binary WebSocket (no Phoenix Channels: their frames are JSON).
      {:bandit, "~> 1.13"},
      {:websock_adapter, "~> 0.5"},
      # Storage: SQLite by default, so a community hub is one container.
      {:ecto_sql, "~> 3.13"},
      {:ecto_sqlite3, "~> 0.22"},
      # mem-proto and mem-crypto through a NIF: one decoder for apps and hub.
      {:rustler, "~> 0.37", runtime: false},
      {:stream_data, "~> 1.2", only: :test},
      # Hub-to-hub sessions and the tests' client.
      {:mint_web_socket, "~> 1.0"}
    ]
  end
end
