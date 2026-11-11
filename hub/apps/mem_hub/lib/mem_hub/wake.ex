defmodule MemHub.Wake do
  @moduledoc """
  Push wake-ups. A client registers a UnifiedPush endpoint with the padded prefixes it
  fetches. When an upload lands under one of them, the hub POSTs a fixed one-byte body to the
  endpoint: "fetch now", nothing else. It sends at most one wake-up per endpoint per minute:
  uploads within that minute are merged into one wake-up at its end, never dropped (a dropped
  one would leave the message waiting until something else woke the phone). It forgets a
  registration after 48 h without renewal.

  The hub POSTs to addresses that clients chose, so it refuses anything that could reach a private
  network (SSRF). It accepts only `https`, to hosts that resolve to public addresses (loopback
  only when `allow_loopback_push` is set, for tests). It drops endpoints that answer 404 or 410.
  It never logs endpoints.
  """
  import Ecto.Query

  alias MemHub.Repo

  @ttl_ms 48 * 60 * 60 * 1000
  # The same byte for every wake-up: it says nothing but "fetch now".
  @body "w"
  @max_endpoints_per_socket 4
  @table __MODULE__

  def max_endpoints_per_socket, do: @max_endpoints_per_socket

  # One minute; shorter only in tests.
  defp min_interval_ms, do: Application.get_env(:mem_hub, :wake_interval_ms, 60_000)

  @doc "Creates the table of last wake times; called once at start."
  def init_table do
    :ets.new(@table, [:named_table, :public, :set])
    :ok
  end

  @doc "Registers `endpoint` for `prefixes`, replacing its earlier prefixes."
  def register(endpoint, prefixes, now \\ MemHub.Store.now_ms()) do
    if acceptable?(endpoint) do
      Repo.transaction(fn ->
        Repo.delete_all(from(w in "wakes", where: w.endpoint == ^endpoint))

        rows =
          for p <- Enum.uniq(prefixes),
              do: %{endpoint: endpoint, prefix: p, expires_at_ms: now + @ttl_ms}

        Repo.insert_all("wakes", rows)
      end)

      :ok
    else
      {:error, :endpoint}
    end
  end

  @doc "Withdraws a registration."
  def unregister(endpoint) do
    Repo.delete_all(from(w in "wakes", where: w.endpoint == ^endpoint))
    :ok
  end

  @doc "Wakes every endpoint registered for the prefix of `tag` (asynchronously)."
  def notify(tag, now \\ MemHub.Store.now_ms()) do
    prefix = binary_part(tag, 0, 2)

    endpoints =
      Repo.all(
        from(w in "wakes",
          where: w.prefix == ^prefix and w.expires_at_ms > ^now,
          select: w.endpoint,
          distinct: true
        )
      )

    Enum.each(endpoints, &wake(&1, now))
    :ok
  end

  # Now, or once at the end of the minute since the last wake-up; `{:pending, e}` makes sure at
  # most one deferred wake-up waits per endpoint (insert_new is atomic).
  defp wake(endpoint, now) do
    interval = min_interval_ms()

    case :ets.lookup(@table, endpoint) do
      [{_, last}] when now - last < interval ->
        if :ets.insert_new(@table, {{:pending, endpoint}, true}) do
          delay = interval - (now - last)

          Task.Supervisor.start_child(MemHub.WakeTasks, fn ->
            Process.sleep(delay)
            :ets.insert(@table, {endpoint, MemHub.Store.now_ms()})
            :ets.delete(@table, {:pending, endpoint})
            post(endpoint)
          end)
        end

      _ ->
        :ets.insert(@table, {endpoint, now})
        Task.Supervisor.start_child(MemHub.WakeTasks, fn -> post(endpoint) end)
    end
  end

  @doc "Removes expired registrations."
  def sweep(now \\ MemHub.Store.now_ms()) do
    Repo.delete_all(from(w in "wakes", where: w.expires_at_ms <= ^now))
    :ok
  end

  # Checked again right before each POST: a name that resolved to a public address when
  # registered may point somewhere private now (DNS rebinding). A rebinding between this check
  # and :httpc's own lookup remains possible; deployments should also block private ranges for
  # the hub's outbound traffic (README).
  defp post(endpoint) do
    if acceptable?(endpoint) do
      request = {String.to_charlist(endpoint), [], ~c"application/octet-stream", @body}

      case :httpc.request(:post, request, http_options(endpoint), body_format: :binary) do
        {:ok, {{_, status, _}, _, _}} when status in [404, 410] -> unregister(endpoint)
        _ -> :ok
      end
    else
      unregister(endpoint)
    end
  end

  defp http_options(endpoint) do
    base = [timeout: 10_000, connect_timeout: 5_000, autoredirect: false]

    if String.starts_with?(endpoint, "https://") do
      [
        ssl: [
          verify: :verify_peer,
          cacerts: :public_key.cacerts_get(),
          customize_hostname_check: [
            match_fun: :public_key.pkix_verify_hostname_match_fun(:https)
          ]
        ]
      ] ++ base
    else
      base
    end
  end

  @doc "Whether the hub may call `endpoint` at all (scheme, length, resolved address)."
  def acceptable?(endpoint) when is_binary(endpoint) and byte_size(endpoint) <= 1024 do
    uri = URI.parse(endpoint)
    loopback_ok = Application.get_env(:mem_hub, :allow_loopback_push, false)

    cond do
      uri.host in [nil, ""] -> false
      uri.scheme == "https" -> public_host?(uri.host, loopback_ok)
      uri.scheme == "http" and loopback_ok -> loopback?(uri.host)
      true -> false
    end
  end

  def acceptable?(_), do: false

  defp loopback?(host), do: host in ["localhost", "127.0.0.1"]

  defp public_host?(host, loopback_ok) do
    case :inet.getaddr(String.to_charlist(host), :inet) do
      {:ok, addr} -> public?(addr) or (loopback_ok and match?({127, _, _, _}, addr))
      _ -> false
    end
  end

  defp public?({10, _, _, _}), do: false
  defp public?({127, _, _, _}), do: false
  defp public?({0, _, _, _}), do: false
  defp public?({169, 254, _, _}), do: false
  defp public?({172, b, _, _}) when b in 16..31, do: false
  defp public?({192, 168, _, _}), do: false
  defp public?({100, b, _, _}) when b in 64..127, do: false
  defp public?({a, _, _, _}) when a >= 224, do: false
  defp public?(_), do: true
end
