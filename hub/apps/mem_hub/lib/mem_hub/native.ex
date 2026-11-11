defmodule MemHub.Native do
  @moduledoc """
  The NIF over `mem-proto` and `mem-crypto`: the hub parses and checks with the
  same bounded, fuzzed Rust code as the apps.
  """
  use Rustler, otp_app: :mem_hub, crate: "mem_hub_nif", path: "../../native/mem_hub_nif"

  @doc "Decodes one hub message: `{:ok, term}` or `{:error, :malformed}`."
  def decode_msg(_binary), do: :erlang.nif_error(:nif_not_loaded)

  @doc "Encodes one hub-to-client message term: `{:ok, binary}`."
  def encode_msg(_term), do: :erlang.nif_error(:nif_not_loaded)

  @doc "Validates an upload at `now_ms`: `{:ok, fields}` or `{:error, put_error}`."
  def check_bundle(_binary, _now_ms), do: :erlang.nif_error(:nif_not_loaded)

  @doc "The purge commitment of a 16-byte token: `{:ok, commitment}`."
  def commit(_token), do: :erlang.nif_error(:nif_not_loaded)

  @doc "The hub public key of a 32-byte seed."
  def hub_public(_seed), do: :erlang.nif_error(:nif_not_loaded)

  @doc "Handshake signature over the peer's nonce, then ours."
  def peer_sign(_seed, _peer_nonce, _own_nonce), do: :erlang.nif_error(:nif_not_loaded)

  @doc "Checks a peer's handshake signature over our nonce, then theirs."
  def peer_verify(_key, _our_nonce, _their_nonce, _sig), do: :erlang.nif_error(:nif_not_loaded)

  @doc "`{:ok, serial, [{url, key}]}` for a list signed by one of `operator_keys`, valid at `now_ms`."
  def verify_hub_list(_bytes, _operator_keys, _now_ms), do: :erlang.nif_error(:nif_not_loaded)
end
