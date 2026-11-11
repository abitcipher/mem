defmodule MemHub.ClientIp do
  @moduledoc """
  The client's address for per-address limits. Behind a reverse proxy every connection comes
  from the proxy, so a proxy listed as trusted may name the client in `x-real-ip`; from anyone
  else the header is ignored (it could be forged to dodge the limits).
  """

  @doc "The client address: `x-real-ip` when `remote_ip` is a trusted proxy, else `remote_ip`."
  @spec resolve(:inet.ip_address(), [{String.t(), String.t()}], [:inet.ip_address()]) ::
          :inet.ip_address()
  def resolve(remote_ip, headers, trusted) do
    with true <- remote_ip in trusted,
         {_, value} <- List.keyfind(headers, "x-real-ip", 0),
         {:ok, ip} <-
           value |> String.trim() |> String.to_charlist() |> :inet.parse_strict_address() do
      ip
    else
      _ -> remote_ip
    end
  end

  @doc "Parses a comma-separated list of proxy addresses (configuration); bad entries are an error."
  @spec parse_list(String.t()) ::
          {:ok, [:inet.ip_address()]} | {:error, {:bad_address, String.t()}}
  def parse_list(text) do
    text
    |> String.split(",", trim: true)
    |> Enum.map(&String.trim/1)
    |> Enum.reduce_while({:ok, []}, &add_address/2)
    |> then(fn
      {:ok, ips} -> {:ok, Enum.reverse(ips)}
      error -> error
    end)
  end

  defp add_address(entry, {:ok, acc}) do
    case :inet.parse_strict_address(String.to_charlist(entry)) do
      {:ok, ip} -> {:cont, {:ok, [ip | acc]}}
      {:error, _} -> {:halt, {:error, {:bad_address, entry}}}
    end
  end
end
