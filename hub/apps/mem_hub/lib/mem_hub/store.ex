defmodule MemHub.Store do
  @moduledoc """
  Mailboxes by tag.

  - `put/2` stores a checked envelope within every quota, or says which one is full. A bundle
    already held is acknowledged and kept once.
  - `fetch/1` returns envelopes under the given 2-byte prefixes, newest last, within the fetch
    byte cap. It never deletes anything: tags are not secret.
  - `purge/2` deletes only on a token whose commitment matches, and keeps a tombstone until the
    longest lifetime so the same bundle cannot be uploaded again. Presenting a token proves
    nothing about who presents it.
  - Everything newly stored or newly tombstoned is numbered in `replog` for the other hubs of the
    network; `log_after/2` reads it. What was already here is not logged again,
    so copies stop at the first hub that has them.
  - Partner operators: a bundle remembers whether it came from another
    operator (`from_partner`, within the `foreign_total` quota) and whether its sender let it cross
    to partners (`may_cross`).
  """
  import Ecto.Query

  alias MemHub.Repo

  # The longest bundle lifetime: tombstones live this long.
  @max_lifetime_ms 30 * 24 * 60 * 60 * 1000

  defp limits, do: Application.fetch_env!(:mem_hub, :limits)

  @doc """
  Stores `bytes` described by `fields` (from `MemHub.Native.check_bundle/2`). With
  `expires_at_ms:` (a copy from a peer hub) the earlier expiry wins. `foreign: true` marks a
  bundle from another operator, `may_cross: true` one its sender let cross to partners.
  """
  def put(fields, bytes, opts \\ []) do
    %{tag: tag, commitment: commitment, bundle_id: bundle_id, kind: kind, size: size} = fields

    expires =
      case opts[:expires_at_ms] do
        nil -> fields.expires_at_ms
        e -> min(e, fields.expires_at_ms)
      end

    l = limits()
    foreign = Keyword.get(opts, :foreign, false)
    may_cross = Keyword.get(opts, :may_cross, false)

    cond do
      size > l.max_bundle ->
        {:error, :too_large}

      tombstoned?(commitment) ->
        {:error, :purged}

      held?(bundle_id, commitment) ->
        :ok

      used(where: [tag: tag]) + size > l.per_tag ->
        {:error, :quota}

      kind == :intro and
          (used(where: [tag: tag, kind: "intro"]) + size > l.intro_per_tag or
             used(where: [kind: "intro"]) + size > l.intro_total) ->
        {:error, :quota}

      used([]) + size > l.total ->
        {:error, :quota}

      foreign and used(where: [from_partner: 1]) + size > l.foreign_total ->
        {:error, :quota}

      expires <= now_ms() ->
        {:error, :expired}

      true ->
        {:ok, new} =
          Repo.transaction(fn ->
            {n, _} =
              Repo.insert_all(
                "bundles",
                [
                  %{
                    tag: tag,
                    prefix: binary_part(tag, 0, 2),
                    commitment: commitment,
                    bundle_id: bundle_id,
                    kind: Atom.to_string(kind),
                    size: size,
                    bytes: bytes,
                    expires_at_ms: expires,
                    # SQLite booleans as 0/1 (a schemaless insert would store text).
                    from_partner: bit(foreign),
                    may_cross: bit(may_cross)
                  }
                ],
                on_conflict: :nothing
              )

            if n == 1,
              do: log(%{kind: "bundle", bundle_id: bundle_id, commitment: commitment})

            n == 1
          end)

        if new, do: announce()
        :ok
    end
  end

  defp bit(true), do: 1
  defp bit(false), do: 0

  defp log(row) do
    Repo.insert_all("replog", [Map.put(row, :inserted_at_ms, now_ms())])
  end

  # Peer sessions waiting for new log entries.
  defp announce do
    Registry.dispatch(MemHub.Repl, :log, fn entries ->
      for {pid, _} <- entries, do: send(pid, :replog)
    end)
  end

  @doc """
  Log entries after `seq`, oldest first, at most `limit`:
  `{:bundle, seq, bytes, expires_at_ms, foreign, may_cross}` for bundles still held and
  `{:purge, seq, [token]}`. Returns `{entries, last_seq_read}`.
  """
  def log_after(seq, limit \\ 128) do
    rows =
      Repo.all(
        from(r in "replog",
          where: r.seq > ^seq,
          order_by: r.seq,
          limit: ^limit,
          select: {r.seq, r.kind, r.bundle_id, r.commitment, r.tokens}
        )
      )

    entries =
      Enum.flat_map(rows, fn
        {n, "bundle", id, c, _} ->
          from(b in "bundles",
            where: b.bundle_id == ^id and b.commitment == ^c and b.expires_at_ms > ^now_ms(),
            select:
              {b.bytes, b.expires_at_ms, type(b.from_partner, :boolean),
               type(b.may_cross, :boolean)}
          )
          |> Repo.one()
          |> case do
            nil -> []
            {bytes, exp, foreign, cross} -> [{:bundle, n, bytes, exp, foreign, cross}]
          end

        {n, "purge", _, _, tokens} ->
          [{:purge, n, for(<<t::binary-16 <- tokens>>, do: t)}]
      end)

    last = rows |> List.last() |> then(&if(&1, do: elem(&1, 0), else: seq))
    {entries, last}
  end

  @doc "The last log number pulled from the peer hub with `key` (0 if none)."
  def peer_seq(key),
    do: Repo.one(from(p in "peers", where: p.key == ^key, select: p.last_seq)) || 0

  @doc "Records the last log number pulled from the peer hub with `key`."
  def put_peer_seq(key, seq) do
    Repo.insert_all("peers", [%{key: key, last_seq: seq}],
      on_conflict: {:replace, [:last_seq]},
      conflict_target: [:key]
    )

    :ok
  end

  defp tombstoned?(commitment),
    do: Repo.exists?(from(t in "tombstones", where: t.commitment == ^commitment))

  defp held?(bundle_id, commitment),
    do:
      Repo.exists?(
        from(b in "bundles", where: b.bundle_id == ^bundle_id and b.commitment == ^commitment)
      )

  defp used(where: conditions) do
    q = from(b in "bundles", select: coalesce(sum(b.size), 0))
    Repo.one(Enum.reduce(conditions, q, fn {k, v}, q -> where(q, [b], field(b, ^k) == ^v) end))
  end

  defp used([]), do: used(where: [])

  @doc """
  Envelopes under `prefixes`, oldest first, within the fetch byte cap, as `{bytes, foreign}`.
  """
  def fetch(prefixes) do
    cap = limits().fetch_bytes

    from(b in "bundles",
      where: b.prefix in ^prefixes and b.expires_at_ms > ^now_ms(),
      order_by: b.expires_at_ms,
      select: {b.bytes, type(b.from_partner, :boolean)}
    )
    |> Repo.all()
    |> Enum.reduce_while({[], 0}, fn {bytes, _} = row, {acc, total} ->
      if total + byte_size(bytes) > cap,
        do: {:halt, {acc, total}},
        else: {:cont, {[row | acc], total + byte_size(bytes)}}
    end)
    |> elem(0)
    |> Enum.reverse()
  end

  @doc "Applies purge tokens: deletes matching bundles and keeps tombstones."
  def purge(tokens, now \\ now_ms()) do
    rows =
      for t <- tokens, byte_size(t) == 16, {:ok, c} = MemHub.Native.commit(t) do
        %{commitment: c, token: t, expires_at_ms: now + @max_lifetime_ms}
      end

    {:ok, fresh} =
      Repo.transaction(fn ->
        commitments = Enum.map(rows, & &1.commitment)

        known =
          MapSet.new(
            Repo.all(
              from(t in "tombstones", where: t.commitment in ^commitments, select: t.commitment)
            )
          )

        fresh = Enum.reject(rows, &MapSet.member?(known, &1.commitment))
        Repo.delete_all(from(b in "bundles", where: b.commitment in ^commitments))
        Repo.insert_all("tombstones", rows, on_conflict: :nothing)

        if fresh != [],
          do: log(%{kind: "purge", tokens: Enum.map_join(fresh, & &1.token)})

        fresh
      end)

    if fresh != [], do: announce()
    :ok
  end

  @doc "Removes expired bundles and tombstones."
  def sweep(now \\ now_ms()) do
    Repo.delete_all(from(b in "bundles", where: b.expires_at_ms <= ^now))
    Repo.delete_all(from(t in "tombstones", where: t.expires_at_ms <= ^now))
    # Nothing in the log outlives the longest lifetime.
    Repo.delete_all(from(r in "replog", where: r.inserted_at_ms <= ^(now - @max_lifetime_ms)))
    :ok
  end

  @doc "Bundles held (for operators and tests; never their tags)."
  def count, do: Repo.aggregate("bundles", :count)

  def now_ms, do: System.os_time(:millisecond)
end
