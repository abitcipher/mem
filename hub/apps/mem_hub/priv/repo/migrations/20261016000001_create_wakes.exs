defmodule MemHub.Repo.Migrations.CreateWakes do
  use Ecto.Migration

  # Push wake-up registrations: an endpoint and the prefixes it wants, until
  # renewed or 48 h pass.
  def change do
    create table(:wakes, primary_key: false) do
      add :endpoint, :string, null: false
      add :prefix, :binary, null: false
      add :expires_at_ms, :bigint, null: false
    end

    create unique_index(:wakes, [:endpoint, :prefix])
    create index(:wakes, [:prefix])
    create index(:wakes, [:expires_at_ms])
  end
end
