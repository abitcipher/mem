defmodule MemHub.Repo.Migrations.AddBundleOrigin do
  use Ecto.Migration

  # Partner operators: `from_partner` marks a bundle that came from another
  # operator's hub; `may_cross` marks one its sender let cross to partners.
  def change do
    alter table(:bundles) do
      add :from_partner, :boolean, null: false, default: false
      add :may_cross, :boolean, null: false, default: false
    end

    create index(:bundles, [:from_partner])
  end
end
