defmodule MemHub.Repo do
  use Ecto.Repo, otp_app: :mem_hub, adapter: Ecto.Adapters.SQLite3
end
