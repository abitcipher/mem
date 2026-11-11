defmodule MemHub do
  @moduledoc """
  The MEM hub: a dumb, replaceable P2 mailbox. It stores sealed
  envelopes by destination tag, hands them to whoever fetches their 2-byte tag prefix, and
  deletes them only when shown their purge token or when they expire. It has no accounts and
  never sees message content.
  """
end
