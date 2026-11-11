defmodule MemHub.WakeTest do
  use ExUnit.Case, async: false

  alias MemHub.{Fixtures, Repo, Store, Wake}

  # A stand-in UnifiedPush distributor: reports every POST to the test process.
  defmodule Receiver do
    use Plug.Router
    plug(:match)
    plug(:dispatch)

    post "/gone" do
      send(:wake_test, {:posted, :gone, read_body(conn) |> elem(1)})
      send_resp(conn, 410, "")
    end

    post "/up" do
      {:ok, body, conn} = read_body(conn)
      send(:wake_test, {:posted, :up, body})
      send_resp(conn, 201, "")
    end
  end

  setup_all do
    {:ok, pid} = Bandit.start_link(plug: Receiver, port: 0, ip: :loopback)
    {:ok, {_ip, port}} = ThousandIsland.listener_info(pid)
    %{base: "http://127.0.0.1:#{port}"}
  end

  setup do
    Process.register(self(), :wake_test)
    Repo.delete_all("bundles")
    Repo.delete_all("tombstones")
    Repo.delete_all("wakes")
    :ets.delete_all_objects(MemHub.Wake)
    :ok
  end

  defp prefixes(real), do: Enum.sort([real | for(i <- 0..6, do: <<0xF0, i>>)])

  test "an upload under a registered prefix wakes the endpoint with a fixed one-byte body, at most once per interval",
       %{base: base} do
    tag = Fixtures.tag(:a1)
    :ok = Wake.register(base <> "/up", prefixes(binary_part(tag, 0, 2)))
    :ok = Store.put(Fixtures.fields(:a1), Fixtures.bytes(:a1))
    Wake.notify(tag)
    assert_receive {:posted, :up, "w"}, 5_000
    # Uploads within the interval merge into one wake-up at its end (1 s in tests), never lost.
    Wake.notify(tag)
    Wake.notify(tag)
    refute_receive {:posted, :up, _}, 500
    assert_receive {:posted, :up, "w"}, 2_000
    refute_receive {:posted, :up, _}, 1_200
    # Another prefix wakes nobody.
    Wake.notify(<<0x00, 0x01>> <> :binary.copy(<<0>>, 14))
    refute_receive {:posted, _, _}, 300
  end

  test "an endpoint that answers 410 is forgotten", %{base: base} do
    tag = Fixtures.tag(:a1)
    :ok = Wake.register(base <> "/gone", prefixes(binary_part(tag, 0, 2)))
    Wake.notify(tag)
    assert_receive {:posted, :gone, "w"}, 5_000
    Process.sleep(200)
    assert Repo.aggregate("wakes", :count) == 0
  end

  test "registrations expire after 48 hours", %{base: base} do
    :ok = Wake.register(base <> "/up", prefixes(<<1, 2>>))
    Wake.sweep(Store.now_ms() + 49 * 60 * 60 * 1000)
    assert Repo.aggregate("wakes", :count) == 0
  end

  test "endpoints that could reach a private network are refused (SSRF)" do
    for e <- [
          "http://example.org/up",
          "https://10.0.0.1/up",
          "https://192.168.1.10/up",
          "https://169.254.169.254/latest",
          "https://[::1]/up",
          "ftp://example.org/x",
          "https:///nohost"
        ] do
      refute Wake.acceptable?(e), e
      assert {:error, :endpoint} = Wake.register(e, prefixes(<<1, 2>>))
    end
  end
end
