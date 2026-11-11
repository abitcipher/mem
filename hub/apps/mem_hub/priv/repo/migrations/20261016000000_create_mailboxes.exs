defmodule MemHub.Repo.Migrations.CreateMailboxes do
  use Ecto.Migration

  # Envelopes by tag. The hub keeps only what it needs to route and expire.
  def change do
    create table(:bundles, primary_key: false) do
      add :tag, :binary, null: false
      add :prefix, :binary, null: false
      add :commitment, :binary, null: false
      add :bundle_id, :binary, null: false
      add :kind, :string, null: false
      add :size, :integer, null: false
      add :bytes, :binary, null: false
      add :expires_at_ms, :bigint, null: false
    end

    # Dedup on (bundle_id, commitment), as couriers do.
    create unique_index(:bundles, [:bundle_id, :commitment])
    create index(:bundles, [:prefix])
    create index(:bundles, [:tag])
    # Non-unique: a junk bundle may copy a commitment; a purge removes both.
    create index(:bundles, [:commitment])
    create index(:bundles, [:expires_at_ms])

    create table(:tombstones, primary_key: false) do
      add :commitment, :binary, primary_key: true
      add :token, :binary, null: false
      add :expires_at_ms, :bigint, null: false
    end

    create index(:tombstones, [:expires_at_ms])
  end
end
