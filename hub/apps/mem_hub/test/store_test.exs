defmodule MemHub.StoreTest do
  use ExUnit.Case, async: false
  use ExUnitProperties

  alias MemHub.{Fixtures, Native, Repo, Store}

  setup do
    Repo.delete_all("bundles")
    Repo.delete_all("tombstones")
    limits = Application.fetch_env!(:mem_hub, :limits)
    on_exit(fn -> Application.put_env(:mem_hub, :limits, limits) end)
    :ok
  end

  defp put(name), do: Store.put(Fixtures.fields(name), Fixtures.bytes(name))
  defp prefix(name), do: binary_part(Fixtures.tag(name), 0, 2)

  test "stores, deduplicates and fetches by prefix" do
    assert :ok = put(:a1)
    assert :ok = put(:a1)
    assert Store.count() == 1
    assert [_] = Store.fetch([prefix(:a1)])
    assert [] = Store.fetch([<<0xFF, 0xFF>>])
  end

  test "fetching never deletes" do
    :ok = put(:a1)
    for _ <- 1..5, do: Store.fetch([prefix(:a1)])
    assert Store.count() == 1
  end

  test "only the matching purge token deletes, and the tombstone refuses re-upload" do
    :ok = put(:a1)
    :ok = put(:a2)
    Store.purge([Fixtures.token(:b_big)])
    assert Store.count() == 2
    Store.purge([Fixtures.token(:a1)])
    assert Store.count() == 1
    assert {:error, :purged} = put(:a1)
  end

  test "per-tag quota" do
    Application.put_env(
      :mem_hub,
      :limits,
      %{Application.fetch_env!(:mem_hub, :limits) | per_tag: 1500}
    )

    assert :ok = put(:a1)
    assert {:error, :quota} = put(:a2)
  end

  test "intro bundles need a valid proof of work" do
    assert {:ok, %{kind: :intro}} =
             Native.check_bundle(Fixtures.bytes(:intro_ok), Fixtures.t0_ms())

    assert {:error, :bad_proof_of_work} =
             Native.check_bundle(Fixtures.bytes(:intro_bad), Fixtures.t0_ms())
  end

  test "malformed and expired uploads are refused" do
    assert {:error, :malformed} = Native.check_bundle(<<1, 2, 3>>, Store.now_ms())
    # One hour after its 1-hour lifetime.
    a1 = Fixtures.bytes(:a1)
    assert {:ok, f} = Native.check_bundle(a1, Store.now_ms())
    assert f.expires_at_ms > Store.now_ms()
    :ok = put(:a1)
    Store.sweep(Store.now_ms() + 2 * 3_600_000)
    assert Store.count() == 0
  end

  test "the NIF never crashes on random input" do
    check all(data <- StreamData.binary(max_length: 512), max_runs: 2_000) do
      assert match?({:error, _}, Native.decode_msg(data)) or
               match?({:ok, _}, Native.decode_msg(data))

      assert match?({:error, _}, Native.check_bundle(data, 0)) or
               match?({:ok, _}, Native.check_bundle(data, 0))
    end
  end
end
