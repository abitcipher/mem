defmodule MemHub.NetworkTest do
  @moduledoc """
  The hub network: the signed list, the handshake, the log stream; partner
  operators' hubs.
  """
  use ExUnit.Case, async: false

  import Ecto.Query

  alias MemHub.{Fixtures, Native, Network, Repo, Store}

  @domain_list "MEM/v1/hub-list"
  @domain_peer "MEM/v1/hub-peer"

  setup_all do
    {:ok, pid} = Bandit.start_link(plug: MemHub.Router, port: 0, ip: :loopback)
    {:ok, {_ip, port}} = ThousandIsland.listener_info(pid)
    dir = Path.join(System.tmp_dir!(), "mem-hubnet-#{System.unique_integer([:positive])}")
    File.mkdir_p!(dir)

    op = :crypto.strong_rand_bytes(32)
    a = :crypto.strong_rand_bytes(32)
    p = :crypto.strong_rand_bytes(32)
    # A partner operator's hub.
    q = :crypto.strong_rand_bytes(32)
    File.write!(Path.join(dir, "a.seed"), a)

    write_list(dir, op, [{"wss://hub-a.invalid/v1", pub(a)}, {"ws://127.0.0.1:1/v1", pub(p)}],
      partners: [{"ws://127.0.0.1:2/v1", pub(q), "Partner"}]
    )

    Application.put_env(:mem_hub, :network, %{
      key_file: Path.join(dir, "a.seed"),
      list_file: Path.join(dir, "hubs.cbor"),
      operator_keys: [Base.encode16(pub(op))]
    })

    # Network started before the env was set: start a fresh one with it.
    :ok = Supervisor.terminate_child(MemHub.Supervisor, MemHub.Network)
    {:ok, _} = Supervisor.restart_child(MemHub.Supervisor, MemHub.Network)

    on_exit(fn ->
      Application.delete_env(:mem_hub, :network)
      Supervisor.terminate_child(MemHub.Supervisor, MemHub.Network)
      Supervisor.restart_child(MemHub.Supervisor, MemHub.Network)
    end)

    %{port: port, dir: dir, op: op, a: a, p: p, q: q}
  end

  setup do
    for t <- ["bundles", "tombstones", "replog"], do: Repo.delete_all(t)
    :ok
  end

  defp pub(seed), do: elem(:crypto.generate_key(:eddsa, :ed25519, seed), 0)
  defp sign(seed, msg), do: :crypto.sign(:eddsa, :none, msg, [seed, :ed25519])

  # --- The signed list, built by hand (deterministic CBOR, spec/hublist.cddl).

  defp write_list(dir, op, hubs, opts \\ []) do
    now = Store.now_ms()
    expires = Keyword.get(opts, :expires, now + 86_400_000)

    body = [
      {0, uint(1)},
      {1, uint(Keyword.get(opts, :serial, 1))},
      {2, uint(now)},
      {3, uint(expires)},
      {4, array(for {url, key} <- hubs, do: map([{0, text(url)}, {1, bytes(key)}]))}
    ]

    partners =
      case Keyword.get(opts, :partners, []) do
        [] ->
          []

        ps ->
          [
            {6,
             array(
               for {url, key, name} <- ps,
                   do: map([{0, text(url)}, {1, bytes(key)}, {2, text(name)}])
             )}
          ]
      end

    # The signature covers every key but 5, the signature itself.
    sig = sign(op, @domain_list <> map(body ++ partners))
    File.write!(Path.join(dir, "hubs.cbor"), map(body ++ [{5, bytes(sig)}] ++ partners))
  end

  defp head(major, n) when n < 24, do: <<major * 32 + n>>
  defp head(major, n) when n < 256, do: <<major * 32 + 24, n>>
  defp head(major, n) when n < 65_536, do: <<major * 32 + 25, n::16>>
  defp head(major, n) when n < 4_294_967_296, do: <<major * 32 + 26, n::32>>
  defp head(major, n), do: <<major * 32 + 27, n::64>>
  defp uint(n), do: head(0, n)
  defp bytes(b), do: head(2, byte_size(b)) <> b
  defp text(t), do: head(3, byte_size(t)) <> t
  defp array(items), do: head(4, length(items)) <> Enum.join(items)

  defp map(pairs),
    do: head(5, length(pairs)) <> Enum.map_join(pairs, fn {k, v} -> uint(k) <> v end)

  # --- A WebSocket client over Mint (as in socket_test.exs).

  defp connect(port) do
    {:ok, conn} = Mint.HTTP.connect(:http, "127.0.0.1", port)
    {:ok, conn, ref} = Mint.WebSocket.upgrade(:ws, conn, "/v1", [])
    {conn, [{:status, ^ref, status}, {:headers, ^ref, headers} | _]} = recv_http(conn)
    {:ok, conn, ws} = Mint.WebSocket.new(conn, ref, status, headers)
    %{conn: conn, ref: ref, ws: ws}
  end

  defp recv_http(conn) do
    receive do
      msg ->
        {:ok, conn, responses} = Mint.WebSocket.stream(conn, msg)
        {conn, responses}
    after
      5_000 -> flunk("no answer")
    end
  end

  defp send_msg(c, msg) do
    {:ok, bin} = Native.encode_msg(msg)
    {:ok, ws, data} = Mint.WebSocket.encode(c.ws, {:binary, bin})
    {:ok, conn} = Mint.WebSocket.stream_request_body(c.conn, c.ref, data)
    %{c | ws: ws, conn: conn}
  end

  # Frames until `n` hub messages arrived (or a close).
  defp recv(c, n, acc \\ [])
  defp recv(c, n, acc) when length(acc) >= n, do: {c, acc}

  defp recv(c, n, acc) do
    socket = Mint.HTTP.get_socket(c.conn)

    receive do
      {tag, ^socket, _} = msg when tag in [:tcp, :tcp_closed] ->
        {:ok, conn, responses} = Mint.WebSocket.stream(c.conn, msg)
        c = %{c | conn: conn}

        Enum.reduce(responses, {c, acc}, fn
          {:data, _, data}, {c, acc} ->
            {:ok, ws, frames} = Mint.WebSocket.decode(c.ws, data)

            msgs =
              for f <- frames do
                case f do
                  {:binary, b} -> elem(Native.decode_msg(b), 1)
                  {:close, code, _} -> {:closed, code}
                  other -> other
                end
              end

            {%{c | ws: ws}, acc ++ msgs}

          _, x ->
            x
        end)
        |> then(fn {c, acc} -> recv(c, n, acc) end)
    after
      5_000 -> {c, acc}
    end
  end

  # Handshake as peer P (own operator), or with `seed` as another listed hub; returns the
  # authenticated session.
  defp as_peer(ctx, seed \\ nil) do
    seed = seed || ctx.p
    nonce = :crypto.strong_rand_bytes(16)
    c = connect(ctx.port) |> send_msg({:peer_hello, pub(seed), nonce})
    {c, [{:peer_hello, key, their}, {:peer_auth, sig}]} = recv(c, 2)
    assert key == pub(ctx.a)
    # Hub A signed our nonce, then its own.
    assert :crypto.verify(:eddsa, :none, @domain_peer <> nonce <> their, sig, [key, :ed25519])
    send_msg(c, {:peer_auth, sign(seed, @domain_peer <> their <> nonce)})
  end

  defp seqs, do: Repo.all(from(r in "replog", order_by: r.seq, select: r.seq))

  test "the list verifies and is served to clients" do
    assert Network.own_key() != nil
    {:ok, bytes} = File.read(Application.get_env(:mem_hub, :network).list_file)
    assert Network.list_bytes() == bytes
  end

  test "an unlisted hub key is refused", ctx do
    stranger = :crypto.strong_rand_bytes(32)
    c = connect(ctx.port) |> send_msg({:peer_hello, pub(stranger), :crypto.strong_rand_bytes(16)})
    assert {_, [{:closed, 1008}]} = recv(c, 1)
  end

  test "a listed key with a wrong signature is refused", ctx do
    nonce = :crypto.strong_rand_bytes(16)
    c = connect(ctx.port) |> send_msg({:peer_hello, pub(ctx.p), nonce})
    {c, [{:peer_hello, _, their}, {:peer_auth, _}]} = recv(c, 2)
    # Signed with the wrong key.
    c = send_msg(c, {:peer_auth, sign(ctx.op, @domain_peer <> their <> nonce)})
    assert {_, [{:closed, 1008}]} = recv(c, 1)
  end

  test "client messages on a peer session end it", ctx do
    c = as_peer(ctx) |> send_msg({:hubs, <<1>>})
    assert {_, [{:closed, 1002}]} = recv(c, 1)
  end

  test "a peer gets the log after its number, then live entries", ctx do
    # Stored before the peer connects: one bundle, one purge.
    :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1))
    :ok = Store.purge([Fixtures.token(:a2)])

    [n1, n2] = seqs()
    c = as_peer(ctx) |> send_msg({:sync_from, 0})

    {c, [{:replicate, ^n1, bytes, exp, false, false}, {:replicate_purge, ^n2, [token]}]} =
      recv(c, 2)

    assert bytes == Fixtures.bytes(:a1)
    assert_in_delta exp, Fixtures.fields(:a1).expires_at_ms, 5_000
    assert token == Fixtures.token(:a2)

    # A repeat of what is held is not logged again; a purged bundle stays out.
    :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1))
    {:error, :purged} = Store.put(Fixtures.fields(:a2), Fixtures.bytes(:a2))
    :ok = Store.purge([Fixtures.token(:a2)])
    assert {_, []} = recv(c, 1)

    :ok = Store.put(Fixtures.fields(:b_big), Fixtures.bytes(:b_big))
    n3 = List.last(seqs())
    assert n3 > n2
    {_, [{:replicate, ^n3, _, _, _, _}]} = recv(c, 1)
  end

  test "a peer that already has entries gets only newer ones", ctx do
    :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1))
    :ok = Store.put(Fixtures.fields(:b_big), Fixtures.bytes(:b_big))
    [n1, n2] = seqs()
    c = as_peer(ctx) |> send_msg({:sync_from, n1})
    {_, [{:replicate, ^n2, _, _, _, _}]} = recv(c, 1)
  end

  # --- Partner operators.

  test "the list's partners are known as partners", ctx do
    assert Network.peer_kind(pub(ctx.p)) == :own
    assert Network.peer_kind(pub(ctx.q)) == :partner
    assert Network.peer_kind(:crypto.strong_rand_bytes(32)) == nil
  end

  test "own hubs get the marks; a partner only what may cross, unmarked, and every purge", ctx do
    # May cross; stays here; came from a partner (never passed on to another partner).
    :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1), may_cross: true)
    :ok = Store.put(Fixtures.fields(:b_big), Fixtures.bytes(:b_big))
    :ok = Store.put(Fixtures.fields(:a2), Fixtures.bytes(:a2), foreign: true, may_cross: true)
    :ok = Store.purge([Fixtures.token(:a2)])
    [n1, n2, n3, n4] = seqs()

    own = as_peer(ctx) |> send_msg({:sync_from, 0})

    {_, msgs} = recv(own, 3)

    assert [
             {:replicate, ^n1, _, _, false, true},
             {:replicate, ^n2, _, _, false, false},
             # a2 was purged: only its purge is left.
             {:replicate_purge, ^n4, _}
           ] = msgs

    partner = as_peer(ctx, ctx.q) |> send_msg({:sync_from, 0})
    {_, msgs} = recv(partner, 2)
    a1 = Fixtures.bytes(:a1)
    assert [{:replicate, ^n1, ^a1, _, false, false}, {:replicate_purge, ^n4, _}] = msgs
    assert n3 in seqs()
  end

  test "bundles from partners stay within their quota" do
    limits = Application.fetch_env!(:mem_hub, :limits)
    on_exit(fn -> Application.put_env(:mem_hub, :limits, limits) end)
    Application.put_env(:mem_hub, :limits, %{limits | foreign_total: 1})
    assert {:error, :quota} = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1), foreign: true)
    # This operator's own uploads are not limited by it.
    assert :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1))
  end

  test "a copy never lives longer than the original" do
    f = Fixtures.fields(:a1)
    :ok = Store.put(f, Fixtures.bytes(:a1), expires_at_ms: f.expires_at_ms - 60_000)

    assert Repo.one(from(b in "bundles", select: b.expires_at_ms)) ==
             f.expires_at_ms - 60_000

    # An already expired copy is refused.
    Repo.delete_all("bundles")
    g = Fixtures.fields(:b_big)
    assert {:error, :expired} = Store.put(g, Fixtures.bytes(:b_big), expires_at_ms: 1)
  end

  @tag :capture_log
  test "a list that is expired, badly signed or without this hub turns the network off", ctx do
    on_exit(fn ->
      write_list(
        ctx.dir,
        ctx.op,
        [
          {"wss://hub-a.invalid/v1", pub(ctx.a)},
          {"ws://127.0.0.1:1/v1", pub(ctx.p)}
        ],
        partners: [{"ws://127.0.0.1:2/v1", pub(ctx.q), "Partner"}]
      )

      Network.reload()
    end)

    write_list(ctx.dir, ctx.op, [{"wss://a/v1", pub(ctx.a)}], expires: Store.now_ms() - 1)
    Network.reload()
    assert Network.own_key() == nil
    refute Network.listed?(pub(ctx.p))

    write_list(ctx.dir, ctx.p, [{"wss://a/v1", pub(ctx.a)}, {"wss://p/v1", pub(ctx.p)}])
    Network.reload()
    assert Network.own_key() == nil

    write_list(ctx.dir, ctx.op, [{"wss://p/v1", pub(ctx.p)}])
    Network.reload()
    assert Network.own_key() == nil
  end
end
