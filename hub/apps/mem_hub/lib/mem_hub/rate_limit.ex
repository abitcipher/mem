defmodule MemHub.RateLimit do
  @moduledoc """
  Per-address counters in ETS: uploads per minute and open sockets. Addresses
  are kept only as counter keys for the current minute; nothing is logged.
  """
  use GenServer

  @table __MODULE__

  def start_link(_), do: GenServer.start_link(__MODULE__, nil, name: __MODULE__)

  @impl true
  def init(nil) do
    :ets.new(@table, [:named_table, :public, :set, write_concurrency: true])
    :timer.send_interval(60_000, :prune)
    {:ok, nil}
  end

  @impl true
  def handle_info(:prune, s) do
    minute = minute()
    :ets.select_delete(@table, [{{{:put, :_, :"$1"}, :_}, [{:<, :"$1", minute}], [true]}])
    {:noreply, s}
  end

  defp minute, do: div(System.os_time(:second), 60)

  @doc "Counts an upload; `false` once over the limit this minute."
  def put?(ip) do
    limit = Application.fetch_env!(:mem_hub, :limits).puts_per_minute
    :ets.update_counter(@table, {:put, ip, minute()}, 1, {{:put, ip, minute()}, 0}) <= limit
  end

  @doc "Takes a socket slot; `false` if the address has too many open."
  def open?(ip) do
    limit = Application.fetch_env!(:mem_hub, :limits).sockets_per_ip

    if :ets.update_counter(@table, {:sockets, ip}, 1, {{:sockets, ip}, 0}) > limit do
      close(ip)
      false
    else
      true
    end
  end

  @doc "Releases a socket slot."
  def close(ip), do: :ets.update_counter(@table, {:sockets, ip}, -1, {{:sockets, ip}, 1})
end
