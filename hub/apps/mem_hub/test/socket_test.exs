defmodule MemHub.SocketTest do
  use ExUnit.Case, async: false

  alias MemHub.{Fixtures, Native, Repo, Store}

  setup_all do
    {:ok, pid} = Bandit.start_link(plug: MemHub.Router, port: 0, ip: :loopback)
    {:ok, {_ip, port}} = ThousandIsland.listener_info(pid)
    %{port: port}
  end

  setup do
    Repo.delete_all("bundles")
    Repo.delete_all("tombstones")
    :ok
  end

  # A minimal WebSocket client over Mint.
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

  defp send_msg(c, frame) do
    {:ok, ws, data} = Mint.WebSocket.encode(c.ws, frame)
    {:ok, conn} = Mint.WebSocket.stream_request_body(c.conn, c.ref, data)
    %{c | ws: ws, conn: conn}
  end

  # Hub messages we build by hand: the CBOR client side lives in Rust; these are tiny.
  defp hello, do: <<0x82, 0x00, 0xA1, 0x00, 0x01>>
  defp put(id, bundle), do: <<0x82, 0x01, 0xA2, 0x00, id, 0x01>> <> cbor_bytes(bundle)

  defp fetch(prefixes),
    do:
      <<0x82, 0x02, 0xA1, 0x00, 0x80 + length(prefixes)>> <>
        Enum.map_join(prefixes, &cbor_bytes/1)

  defp purge(tokens),
    do: <<0x82, 0x03, 0xA1, 0x00, 0x80 + length(tokens)>> <> Enum.map_join(tokens, &cbor_bytes/1)

  defp cbor_bytes(b) when byte_size(b) < 24, do: <<0x40 + byte_size(b)>> <> b
  defp cbor_bytes(b) when byte_size(b) < 256, do: <<0x58, byte_size(b)>> <> b
  defp cbor_bytes(b) when byte_size(b) < 65_536, do: <<0x59, byte_size(b)::16>> <> b

  defp recv_msgs(c, n, acc \\ [])
  defp recv_msgs(c, 0, acc), do: {c, Enum.reverse(acc)}

  defp recv_msgs(c, n, acc) do
    socket = Mint.HTTP.get_socket(c.conn)

    # Only this connection's packets: the other client's live pushes wait in the mailbox.
    receive do
      {tag, ^socket, _} = msg when tag in [:tcp, :tcp_closed] ->
        {:ok, conn, responses} = Mint.WebSocket.stream(c.conn, msg)
        c = %{c | conn: conn}

        {c, frames} =
          Enum.reduce(responses, {c, []}, fn
            {:data, _, data}, {c, fs} ->
              {:ok, ws, frames} = Mint.WebSocket.decode(c.ws, data)
              {%{c | ws: ws}, fs ++ frames}

            _, acc ->
              acc
          end)

        msgs = for {:binary, b} <- frames, {:ok, m} = Native.decode_msg(b), do: m
        recv_msgs(c, max(n - length(msgs), 0), Enum.reverse(msgs) ++ acc)
    after
      5_000 -> flunk("expected #{n} more messages, got #{inspect(Enum.reverse(acc))}")
    end
  end

  defp eventually(f, tries \\ 50) do
    cond do
      f.() -> true
      tries == 0 -> false
      true -> Process.sleep(20) && eventually(f, tries - 1)
    end
  end

  defp prefixes(real), do: Enum.sort([real | for(i <- 0..6, do: <<0xF0, i>>)])

  test "greeting, upload, backlog, live push and purge", %{port: port} do
    a = connect(port) |> send_msg({:binary, hello()})
    {a, [{:welcome, 1}]} = recv_msgs(a, 1)
    b = connect(port) |> send_msg({:binary, hello()})
    {b, [{:welcome, 1}]} = recv_msgs(b, 1)

    tag = Fixtures.tag(:a1)
    b = send_msg(b, {:binary, fetch(prefixes(binary_part(tag, 0, 2)))})
    {b, [:fetch_done]} = recv_msgs(b, 1)

    a = send_msg(a, {:binary, put(1, Fixtures.bytes(:a1))})
    {_a, [{:put_ok, 1}]} = recv_msgs(a, 1)
    {b, [{:bundles, [got], []}]} = recv_msgs(b, 1)
    assert got == Fixtures.bytes(:a1)

    b = send_msg(b, {:binary, purge([Fixtures.token(:a1)])})
    assert eventually(fn -> Store.count() == 0 end)
    send_msg(b, {:binary, <<0x82, 0x04, 0xA0>>})
  end

  # A client sees which bundles came from another operator.
  test "bundles from another operator are marked for clients", %{port: port} do
    :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1), foreign: true)
    c = connect(port) |> send_msg({:binary, hello()})
    {c, [{:welcome, 1}]} = recv_msgs(c, 1)
    c = send_msg(c, {:binary, fetch(prefixes(binary_part(Fixtures.tag(:a1), 0, 2)))})
    {c, [{:bundles, [_], [0]}, :fetch_done]} = recv_msgs(c, 2)

    # Live: an upload from this operator's client is not foreign.
    Repo.delete_all("bundles")
    a = connect(port) |> send_msg({:binary, hello()})
    {a, [{:welcome, 1}]} = recv_msgs(a, 1)
    a = send_msg(a, {:binary, put(2, Fixtures.bytes(:a1))})
    {_a, [{:put_ok, 2}]} = recv_msgs(a, 1)
    {_c, [{:bundles, [_], []}]} = recv_msgs(c, 1)
  end

  test "garbage and text frames end the session", %{port: port} do
    c = connect(port) |> send_msg({:text, "hello"})
    {_c, []} = recv_msgs(c, 0)
    c2 = connect(port) |> send_msg({:binary, <<0xFF, 0x00>>})
    {_c2, []} = recv_msgs(c2, 0)
  end
end
