defmodule MemHub.Sweeper do
  @moduledoc "Deletes expired bundles and tombstones every minute."
  use GenServer

  def start_link(_), do: GenServer.start_link(__MODULE__, nil, name: __MODULE__)

  @impl true
  def init(nil) do
    :timer.send_interval(60_000, :sweep)
    {:ok, nil}
  end

  @impl true
  def handle_info(:sweep, s) do
    MemHub.Store.sweep()
    MemHub.Wake.sweep()
    {:noreply, s}
  end
end
