defmodule MemHub.Peer do
  @moduledoc """
  Pulls one peer hub's log. Dials the peer's listed URL, proves this
  hub's key and checks the peer's (it must be the key the list gives for that URL), asks for
  everything after the last number it stored, and keeps the session open for live entries.

  What arrives is checked again with the NIF and stored like an upload (quotas, tombstones,
  live delivery to clients, push wake-ups); copying never lengthens a bundle's life. On any
  failure the session ends and is retried with a growing delay.

  From a partner operator's hub everything is stored as foreign and never as
  "may cross", so it reaches this operator's clients and hubs but no further partner.
  """
  use GenServer, restart: :transient
  require Logger

  alias MemHub.{Native, Network, Store}

  @min_backoff 1_000
  @max_backoff 60_000

  def start_link({url, key, kind}),
    do: GenServer.start_link(__MODULE__, {url, key, kind}, name: via(key))

  def stop(key) do
    case Registry.lookup(MemHub.PeerNames, key) do
      [{pid, _}] -> DynamicSupervisor.terminate_child(MemHub.Peers, pid)
      [] -> :ok
    end
  end

  defp via(key), do: {:via, Registry, {MemHub.PeerNames, key}}

  @impl true
  def init({url, key, kind}) do
    send(self(), :connect)

    {:ok,
     %{
       url: URI.parse(url),
       key: key,
       kind: kind,
       conn: nil,
       ws: nil,
       ref: nil,
       phase: :down,
       nonce: nil,
       their_nonce: nil,
       backoff: @min_backoff
     }}
  end

  @impl true
  def handle_info(:connect, s) do
    case connect(s) do
      {:ok, s} ->
        {:noreply, s}

      {:error, _} ->
        {:noreply, schedule(s)}
    end
  end

  def handle_info(msg, %{conn: conn} = s) when conn != nil do
    case Mint.WebSocket.stream(conn, msg) do
      {:ok, conn, responses} -> {:noreply, Enum.reduce_while(responses, %{s | conn: conn}, &on/2)}
      {:error, _conn, _reason, _} -> {:noreply, retry(s)}
      :unknown -> {:noreply, s}
    end
  end

  def handle_info(_, s), do: {:noreply, s}

  defp connect(%{url: url} = s) do
    {http, ws} = if url.scheme == "wss", do: {:https, :wss}, else: {:http, :ws}
    port = url.port || if(http == :https, do: 443, else: 80)

    with {:ok, conn} <- Mint.HTTP.connect(http, url.host, port, protocols: [:http1]),
         {:ok, conn, ref} <- Mint.WebSocket.upgrade(ws, conn, url.path || "/v1", []) do
      {:ok, %{s | conn: conn, ref: ref, phase: :upgrading}}
    end
  end

  # The upgrade answer, then frames.
  defp on({:status, ref, status}, %{ref: ref} = s), do: {:cont, Map.put(s, :status, status)}

  defp on({:headers, ref, headers}, %{ref: ref, phase: :upgrading} = s) do
    case Mint.WebSocket.new(s.conn, ref, s.status, headers) do
      {:ok, conn, ws} ->
        nonce = :crypto.strong_rand_bytes(16)
        s = %{s | conn: conn, ws: ws, phase: :hello, nonce: nonce}
        {:cont, send_msg(s, {:peer_hello, Network.own_key() || <<0::256>>, nonce})}

      {:error, _conn, _} ->
        {:halt, retry(s)}
    end
  end

  defp on({:data, ref, data}, %{ref: ref, ws: ws} = s) when ws != nil do
    case Mint.WebSocket.decode(ws, data) do
      {:ok, ws, frames} -> Enum.reduce_while(frames, %{s | ws: ws}, &frame/2) |> wrap()
      {:error, _, _} -> {:halt, retry(s)}
    end
  end

  defp on({:done, _}, s), do: {:cont, s}
  defp on(_, s), do: {:cont, s}

  defp wrap(%{phase: :down} = s), do: {:halt, s}
  defp wrap(s), do: {:cont, s}

  defp frame({:binary, bin}, s) do
    case Native.decode_msg(bin) do
      {:ok, msg} -> handle(msg, s)
      _ -> {:halt, retry(s)}
    end
  end

  defp frame({:ping, data}, s), do: {:cont, send_frame(s, {:pong, data})}
  defp frame(_, s), do: {:halt, retry(s)}

  # The peer must present the key the list names for this URL.
  defp handle({:peer_hello, key, nonce}, %{phase: :hello, key: key} = s),
    do: {:cont, %{s | their_nonce: nonce, phase: :auth}}

  defp handle({:peer_auth, sig}, %{phase: :auth} = s) do
    if Native.peer_verify(s.key, s.nonce, s.their_nonce, sig) do
      s = send_msg(s, {:peer_auth, Network.sign(s.their_nonce, s.nonce)})

      {:cont,
       %{send_msg(s, {:sync_from, Store.peer_seq(s.key)}) | phase: :synced, backoff: @min_backoff}}
    else
      Logger.warning("hub peer refused: bad handshake")
      {:halt, retry(s)}
    end
  end

  defp handle({:replicate, seq, bytes, expires, foreign, may_cross}, %{phase: :synced} = s) do
    # A partner's bundles are foreign here and stop at this operator (one crossing at most).
    {foreign, may_cross} = if s.kind == :partner, do: {true, false}, else: {foreign, may_cross}

    with {:ok, fields} <- Native.check_bundle(bytes, Store.now_ms()),
         :ok <-
           Store.put(fields, bytes,
             expires_at_ms: expires,
             foreign: foreign,
             may_cross: may_cross
           ) do
      MemHub.Socket.deliver(fields.tag, bytes, foreign)
    end

    # Refused copies (quota, purged, expired) are skipped, not retried.
    Store.put_peer_seq(s.key, seq)
    {:cont, s}
  end

  defp handle({:replicate_purge, seq, tokens}, %{phase: :synced} = s) do
    Store.purge(tokens)
    Store.put_peer_seq(s.key, seq)
    {:cont, s}
  end

  defp handle(_, s), do: {:halt, retry(s)}

  defp send_msg(s, msg) do
    {:ok, bin} = Native.encode_msg(msg)
    send_frame(s, {:binary, bin})
  end

  defp send_frame(s, frame) do
    with {:ok, ws, data} <- Mint.WebSocket.encode(s.ws, frame),
         {:ok, conn} <- Mint.WebSocket.stream_request_body(s.conn, s.ref, data) do
      %{s | ws: ws, conn: conn}
    else
      _ -> retry(s)
    end
  end

  # Once per lost session: frames still queued in the same batch see `:down` and stop.
  defp retry(%{phase: :down} = s), do: s

  defp retry(s) do
    if s.conn, do: Mint.HTTP.close(s.conn)
    schedule(s)
  end

  defp schedule(s) do
    Process.send_after(self(), :connect, s.backoff)
    %{s | conn: nil, ws: nil, ref: nil, phase: :down, backoff: min(s.backoff * 2, @max_backoff)}
  end
end
