defmodule MemHub.Repo.Migrations.CreateReplog do
  use Ecto.Migration

  # The hub network. `replog` numbers everything this hub newly stored or newly
  # tombstoned; peers pull it after the last number they saw. AUTOINCREMENT so a number is never
  # reused after old rows are swept.
  def up do
    execute("""
    CREATE TABLE replog (
      seq INTEGER PRIMARY KEY AUTOINCREMENT,
      kind TEXT NOT NULL,
      bundle_id BLOB,
      commitment BLOB,
      tokens BLOB,
      inserted_at_ms INTEGER NOT NULL
    )
    """)

    execute("CREATE INDEX replog_inserted_at_ms_index ON replog (inserted_at_ms)")

    # The last number pulled from each peer hub, by its hub key.
    create table(:peers, primary_key: false) do
      add :key, :binary, primary_key: true
      add :last_seq, :bigint, null: false
    end
  end

  def down do
    drop table(:peers)
    execute("DROP TABLE replog")
  end
end
