defmodule MemHub.Network do
  @moduledoc """
  The hub network: this hub's key, the operator-signed hub list, and one
  `MemHub.Peer` per other listed hub, own (key 4 of the list) or partner (key 6, another
  operator's hub).

  Configured with

      config :mem_hub, network: %{
        key_file: "/data/hub.seed",        # 32-byte seed from `mem-hublist keygen`
        list_file: "/data/hubs.cbor",      # from `mem-hublist sign`
        operator_keys: ["<64 hex digits>"] # who may sign the list
      }

  Without `network`, or while the list is missing, invalid or does not name this hub, the hub
  runs alone: it serves clients as before and refuses hub-to-hub sessions. The list file is read
  again every hour, so the operator adds or removes hubs by replacing it.
  """
  use GenServer
  require Logger

  alias MemHub.Native

  @reload_ms 60 * 60 * 1000

  def start_link(_), do: GenServer.start_link(__MODULE__, nil, name: __MODULE__)

  @doc "This hub's public key, or nil when the network is off."
  def own_key, do: call(:own_key)

  @doc "True when `key` belongs to another hub in the current list (own or partner)."
  def listed?(key), do: peer_kind(key) != nil

  @doc "`:own` or `:partner` for another hub in the current list, else nil."
  def peer_kind(key), do: call({:peer_kind, key})

  @doc "The current signed list, to hand to clients, or nil."
  def list_bytes, do: call(:list_bytes)

  @doc "Our handshake signature over the peer's nonce, then ours."
  def sign(peer_nonce, own_nonce), do: call({:sign, peer_nonce, own_nonce})

  @doc "Reads the list file again now (also hourly)."
  def reload, do: call(:reload)

  defp call(msg) do
    if Process.whereis(__MODULE__), do: GenServer.call(__MODULE__, msg), else: nil
  end

  @impl true
  def init(nil) do
    state = %{seed: nil, key: nil, list: nil, peers: %{}}

    case Application.get_env(:mem_hub, :network) do
      nil ->
        {:ok, state}

      cfg ->
        :timer.send_interval(@reload_ms, :reload)
        {:ok, load(Map.merge(state, %{cfg: cfg}))}
    end
  end

  @impl true
  def handle_call(:own_key, _, s), do: {:reply, s.key, s}
  def handle_call(:list_bytes, _, s), do: {:reply, s.list, s}

  def handle_call({:peer_kind, key}, _, s) do
    kind =
      case Map.get(s.peers, key) do
        {_url, kind} when s.list != nil and key != s.key -> kind
        _ -> nil
      end

    {:reply, kind, s}
  end

  def handle_call({:sign, peer_nonce, own_nonce}, _, s) when s.seed != nil,
    do: {:reply, Native.peer_sign(s.seed, peer_nonce, own_nonce), s}

  def handle_call({:sign, _, _}, _, s), do: {:reply, nil, s}
  def handle_call(:reload, _, s), do: {:reply, :ok, if(s[:cfg], do: load(s), else: s)}

  @impl true
  def handle_info(:reload, s), do: {:noreply, load(s)}

  defp load(%{cfg: cfg} = s) do
    with {:ok, seed} <- read(cfg.key_file),
         32 <- byte_size(seed),
         key = Native.hub_public(seed),
         {:ok, bytes} <- read(cfg.list_file),
         ops = Enum.map(cfg.operator_keys, &Base.decode16!(&1, case: :mixed)),
         {:ok, _serial, hubs, partners} <-
           Native.verify_hub_list(bytes, ops, MemHub.Store.now_ms()),
         true <- Enum.any?(hubs, fn {_, k} -> k == key end) || :not_listed do
      own = for {url, k} <- hubs, k != key, into: %{}, do: {k, {url, :own}}
      # A key listed both ways counts as own (mem-hublist refuses such lists).
      peers =
        Enum.reduce(partners, own, fn {url, k}, acc -> Map.put_new(acc, k, {url, :partner}) end)

      sync_peers(s.peers, peers)
      %{s | seed: seed, key: key, list: bytes, peers: peers}
    else
      other ->
        # Never the key or the list contents: only that the network is off and why.
        Logger.warning("hub network off: #{inspect(reason(other))}")
        sync_peers(s.peers, %{})
        %{s | seed: nil, key: nil, list: nil, peers: %{}}
    end
  end

  defp read(path), do: File.read(path)

  defp reason({:error, :invalid}), do: :invalid_list
  defp reason({:error, posix}) when is_atom(posix), do: {:file, posix}
  defp reason(:not_listed), do: :this_hub_not_in_list
  defp reason(n) when is_integer(n), do: :bad_key_file
  defp reason(_), do: :unknown

  # Starts pullers for new peers (or peers whose address or kind changed) and stops removed ones.
  defp sync_peers(old, new) do
    for {k, entry} <- old, Map.get(new, k) != entry, do: MemHub.Peer.stop(k)

    for {k, {url, kind} = entry} <- new, Map.get(old, k) != entry do
      DynamicSupervisor.start_child(MemHub.Peers, {MemHub.Peer, {url, k, kind}})
    end

    :ok
  end
end
