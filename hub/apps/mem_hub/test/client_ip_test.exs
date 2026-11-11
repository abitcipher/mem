defmodule MemHub.ClientIpTest do
  use ExUnit.Case, async: true

  alias MemHub.ClientIp

  @proxy {127, 0, 0, 1}

  test "a trusted proxy names the client" do
    assert ClientIp.resolve(@proxy, [{"x-real-ip", "203.0.113.7"}], [@proxy]) == {203, 0, 113, 7}
  end

  test "the header from anyone else is ignored" do
    assert ClientIp.resolve({198, 51, 100, 2}, [{"x-real-ip", "203.0.113.7"}], [@proxy]) ==
             {198, 51, 100, 2}
  end

  test "no trusted proxies: the header is ignored" do
    assert ClientIp.resolve(@proxy, [{"x-real-ip", "203.0.113.7"}], []) == @proxy
  end

  test "a missing or malformed header keeps the proxy's address" do
    assert ClientIp.resolve(@proxy, [], [@proxy]) == @proxy
    assert ClientIp.resolve(@proxy, [{"x-real-ip", "not an ip"}], [@proxy]) == @proxy
  end

  test "IPv6 clients" do
    assert ClientIp.resolve(@proxy, [{"x-real-ip", "2001:db8::1"}], [@proxy]) ==
             {8193, 3512, 0, 0, 0, 0, 0, 1}
  end

  test "configuration lists" do
    assert ClientIp.parse_list("127.0.0.1, ::1") ==
             {:ok, [{127, 0, 0, 1}, {0, 0, 0, 0, 0, 0, 0, 1}]}

    assert ClientIp.parse_list("") == {:ok, []}
    assert ClientIp.parse_list("127.0.0.1,nope") == {:error, {:bad_address, "nope"}}
  end
end
