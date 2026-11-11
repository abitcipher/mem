defmodule MemHub.Router do
  @moduledoc "HTTP entry: `/v1` upgrades to the hub WebSocket; `/health` for operators."
  use Plug.Router

  plug(:match)
  plug(:dispatch)

  # One hub message per frame, bounded before buffering.
  @max_frame 1_048_768 + 64

  get "/v1" do
    if websocket_upgrade?(conn), do: upgrade(conn), else: send_resp(conn, 426, "")
  end

  defp upgrade(conn) do
    conn
    |> WebSockAdapter.upgrade(MemHub.Socket, %{ip: client_ip(conn)},
      timeout: 120_000,
      max_frame_size: @max_frame,
      compress: false
    )
    |> halt()
  end

  # A plain or HTTP/2 request without the upgrade: refuse cleanly, no crash in the log.
  defp websocket_upgrade?(conn) do
    conn
    |> get_req_header("upgrade")
    |> Enum.any?(&(String.downcase(&1) =~ "websocket"))
  end

  get "/health" do
    send_resp(conn, 200, "ok")
  end

  match _ do
    send_resp(conn, 404, "")
  end

  # Behind a trusted reverse proxy, the client it names (per-address limits).
  defp client_ip(conn) do
    trusted = Application.get_env(:mem_hub, :trusted_proxies, [])
    MemHub.ClientIp.resolve(conn.remote_ip, conn.req_headers, trusted)
  end
end
