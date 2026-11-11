defmodule MemHub.Socket do
  @moduledoc """
  One client connection: binary frames of CBOR hub messages, decoded and
  encoded by the NIF. After `fetch` the client gets the stored backlog, then `fetch_done`, then
  new matching bundles live (through `MemHub.Live`, keyed by prefix). Text frames, or anything
  before `hello`, end the session.

  A session that starts with `peer_hello` is another hub of the network instead:
  only a hub key in the current signed list, proven by signing our nonce, gets the log
  (`sync_from`), then live entries. A peer session never accepts client messages, and a client
  session never accepts hub-to-hub ones.

  A partner operator's hub gets a filtered log: only bundles their sender let
  cross that did not come from another operator themselves, and every purge. Bundles
  from partners go to clients marked as from another operator (`bundles` key 1).
  """
  @behaviour WebSock

  alias MemHub.{Native, RateLimit, Store}

  @version 1

  @impl true
  def init(%{ip: ip}) do
    if RateLimit.open?(ip) do
      {:ok,
       %{ip: ip, hello: false, prefixes: [], endpoints: MapSet.new(), peer: nil, sent_seq: 0}}
    else
      {:stop, :normal, 1013, %{ip: nil}}
    end
  end

  @impl true
  def handle_in({data, opcode: :binary}, state) do
    case Native.decode_msg(data) do
      {:ok, msg} -> handle(msg, state)
      {:error, _} -> {:stop, :normal, 1007, state}
    end
  end

  def handle_in({_text, opcode: :text}, state), do: {:stop, :normal, 1003, state}

  defp handle({:hello, _v}, %{hello: false} = state) do
    l = Application.fetch_env!(:mem_hub, :limits)

    reply([{:welcome, @version, l.max_bundle, l.per_tag, l.puts_per_minute}], %{
      state
      | hello: true
    })
  end

  # Hub to hub: we answer with our key, nonce and signature over theirs.
  defp handle({:peer_hello, key, their_nonce}, %{hello: false, peer: nil} = state) do
    if kind = MemHub.Network.peer_kind(key) do
      nonce = :crypto.strong_rand_bytes(16)
      sig = MemHub.Network.sign(their_nonce, nonce)

      reply([{:peer_hello, MemHub.Network.own_key(), nonce}, {:peer_auth, sig}], %{
        state
        | peer: {:auth, key, kind, nonce, their_nonce}
      })
    else
      {:stop, :normal, 1008, state}
    end
  end

  defp handle({:peer_auth, sig}, %{peer: {:auth, key, kind, nonce, their_nonce}} = state) do
    if Native.peer_verify(key, nonce, their_nonce, sig),
      do: {:ok, %{state | peer: {:ok, key, kind}}},
      else: {:stop, :normal, 1008, state}
  end

  defp handle({:sync_from, n}, %{peer: {:ok, _, _}} = state) do
    Registry.register(MemHub.Repl, :log, nil)
    send(self(), :replog)
    {:ok, %{state | sent_seq: n}}
  end

  defp handle(_msg, %{peer: p} = state) when p != nil, do: {:stop, :normal, 1002, state}
  defp handle(_msg, %{hello: false} = state), do: {:stop, :normal, 1002, state}

  defp handle({:put, id, bundle, may_cross}, state) do
    answer =
      if RateLimit.put?(state.ip) do
        with {:ok, fields} <- Native.check_bundle(bundle, Store.now_ms()),
             :ok <- Store.put(fields, bundle, may_cross: may_cross) do
          deliver(fields.tag, bundle, false)
          {:put_ok, id}
        else
          {:error, reason} -> {:put_err, id, reason}
        end
      else
        {:put_err, id, :rate_limited}
      end

    reply([answer], state)
  end

  defp handle({:fetch, prefixes}, state) do
    Enum.each(state.prefixes, &Registry.unregister(MemHub.Live, &1))
    Enum.each(prefixes, &Registry.register(MemHub.Live, &1, nil))
    backlog = prefixes |> Store.fetch() |> Enum.chunk_every(16) |> Enum.map(&batch/1)
    reply(backlog ++ [:fetch_done], %{state | prefixes: prefixes})
  end

  defp handle({:purge, tokens}, state) do
    Store.purge(tokens)
    {:ok, state}
  end

  # Push wake-ups: a few endpoints per connection; refused ones are simply not stored.
  defp handle({:wake_register, endpoint, prefixes}, state) do
    known = MapSet.member?(state.endpoints, endpoint)

    if known or MapSet.size(state.endpoints) < MemHub.Wake.max_endpoints_per_socket() do
      case MemHub.Wake.register(endpoint, prefixes) do
        :ok -> {:ok, %{state | endpoints: MapSet.put(state.endpoints, endpoint)}}
        {:error, _} -> {:ok, state}
      end
    else
      {:ok, state}
    end
  end

  defp handle({:wake_unregister, endpoint}, state) do
    if MapSet.member?(state.endpoints, endpoint), do: MemHub.Wake.unregister(endpoint)
    {:ok, %{state | endpoints: MapSet.delete(state.endpoints, endpoint)}}
  end

  # The signed hub list, when this hub belongs to a network.
  defp handle(:get_hubs, state) do
    case MemHub.Network.list_bytes() do
      nil -> {:ok, state}
      bytes -> reply([{:hubs, bytes}], state)
    end
  end

  defp handle(:bye, state), do: {:stop, :normal, 1000, state}
  # Hub-to-client kinds from a client are a protocol violation.
  defp handle(_other, state), do: {:stop, :normal, 1002, state}

  @doc """
  A newly stored bundle (`foreign`: from another operator): live to matching clients, and push
  wake-ups.
  """
  def deliver(tag, bundle, foreign) do
    notify(tag, bundle, foreign)
    MemHub.Wake.notify(tag)
  end

  defp notify(tag, bundle, foreign) do
    Registry.dispatch(MemHub.Live, binary_part(tag, 0, 2), fn entries ->
      for {pid, _} <- Enum.uniq_by(entries, &elem(&1, 0)), do: send(pid, {:live, bundle, foreign})
    end)
  end

  # One `bundles` message with the positions of foreign bundles.
  defp batch(rows) do
    foreign = for {{_, true}, i} <- Enum.with_index(rows), do: i
    {:bundles, Enum.map(rows, &elem(&1, 0)), foreign}
  end

  @impl true
  def handle_info({:live, bundle, foreign}, state),
    do: reply([batch([{bundle, foreign}])], state)

  # A peer hub's log stream: the next batch after what it has; more until caught up.
  def handle_info(:replog, %{peer: {:ok, _, kind}} = state) do
    {entries, last} = Store.log_after(state.sent_seq)

    msgs = entries |> Enum.map(&for_peer(&1, kind)) |> Enum.reject(&is_nil/1)

    if last > state.sent_seq, do: send(self(), :replog)
    reply(msgs, %{state | sent_seq: last})
  end

  def handle_info(_, state), do: {:ok, state}

  # What a peer gets of one log entry: own hubs everything, with its marks; partners only what
  # may cross and is not foreign itself, unmarked, and every purge.
  defp for_peer({:bundle, n, bytes, exp, foreign, cross}, :own),
    do: {:replicate, n, bytes, exp, foreign, cross}

  defp for_peer({:bundle, n, bytes, exp, false, true}, :partner),
    do: {:replicate, n, bytes, exp, false, false}

  defp for_peer({:bundle, _, _, _, _, _}, :partner), do: nil
  defp for_peer({:purge, n, tokens}, _), do: {:replicate_purge, n, tokens}

  @impl true
  def terminate(_reason, %{ip: nil}), do: :ok
  def terminate(_reason, %{ip: ip}), do: RateLimit.close(ip)

  defp reply(msgs, state) do
    frames = for m <- msgs, {:ok, bin} = Native.encode_msg(m), do: {:binary, bin}
    {:push, frames, state}
  end
end
