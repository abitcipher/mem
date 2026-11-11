defmodule MemHub.Application do
  @moduledoc false
  use Application

  @impl true
  def start(_type, _args) do
    MemHub.Wake.init_table()

    children =
      [
        MemHub.Repo,
        {Ecto.Migrator, repos: [MemHub.Repo]},
        {Registry, keys: :duplicate, name: MemHub.Live},
        # The hub network: peer sessions waiting for log entries, and pullers.
        {Registry, keys: :duplicate, name: MemHub.Repl},
        {Registry, keys: :unique, name: MemHub.PeerNames},
        {DynamicSupervisor, name: MemHub.Peers, strategy: :one_for_one},
        MemHub.Network,
        {Task.Supervisor, name: MemHub.WakeTasks},
        MemHub.RateLimit,
        MemHub.Sweeper
      ] ++ server()

    Supervisor.start_link(children, strategy: :one_for_one, name: MemHub.Supervisor)
  end

  defp server do
    if Application.get_env(:mem_hub, :start_server, true) do
      [{Bandit, plug: MemHub.Router, port: Application.fetch_env!(:mem_hub, :port)}]
    else
      []
    end
  end
end
