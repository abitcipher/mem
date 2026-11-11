#!/usr/bin/env bash
# Three hubs of one network on this machine: a node on hub A sends to a node on
# hub B; hub C starts only afterwards and must catch up (bundles purged, tombstones copied).
# Then partner operators: hub P of a second operator; the two lists name each other's
# hubs as partners; a node on A and a node on P exchange a message and its ack across them.
# Run from the repository root: hub/scripts/network-test.sh
set -euo pipefail

root=$(pwd)
work=$(mktemp -d -t mem-hubnet-XXXXXX)
tool="$root/target/debug/mem-hublist"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT

cargo build -q -p mem-hublist
op=$("$tool" keygen "$work/operator.seed")
op2=$("$tool" keygen "$work/operator2.seed")
declare -A key port=([a]=4301 [b]=4302 [c]=4303 [p]=4304) list=([a]=hubs [b]=hubs [c]=hubs [p]=hubs2) opkey
entries=()
partners=()
for h in a b c p; do
  key[$h]=$("$tool" keygen "$work/$h.seed")
done
for h in a b c; do
  entries+=("ws://127.0.0.1:${port[$h]}/v1=${key[$h]}")
  partners+=("partner:ws://127.0.0.1:${port[$h]}/v1=${key[$h]}@Operator one")
  opkey[$h]=$op
done
opkey[p]=$op2
"$tool" sign "$work/operator.seed" 1 1 "$work/hubs.cbor" "${entries[@]}" \
  "partner:ws://127.0.0.1:${port[p]}/v1=${key[p]}@Operator two" >/dev/null
"$tool" sign "$work/operator2.seed" 1 1 "$work/hubs2.cbor" \
  "ws://127.0.0.1:${port[p]}/v1=${key[p]}" "${partners[@]}" >/dev/null

(cd hub && MIX_ENV=dev mix compile >/dev/null)

start() {
  local h=$1
  (cd hub && MIX_ENV=dev MEM_HUB_PORT=${port[$h]} MEM_HUB_DB="$work/$h.db" \
    MEM_HUB_KEY="$work/$h.seed" MEM_HUB_LIST="$work/${list[$h]}.cbor" MEM_HUB_OPERATOR_KEYS="${opkey[$h]}" \
    exec mix run --no-halt >"$work/$h.log" 2>&1) &
  pids+=($!)
  for _ in $(seq 1 60); do
    curl -sf "http://127.0.0.1:${port[$h]}/health" >/dev/null && return
    sleep 1
  done
  echo "hub $h did not start"; cat "$work/$h.log"; exit 1
}

count() { sqlite3 "$work/$1.db" "select count(*) from $2"; }

start a
start b
start p
MEM_HUB_URL=ws://127.0.0.1:4301/v1 MEM_HUB_URL_B=ws://127.0.0.1:4302/v1 \
  cargo test -q -p mem-runtime two_nodes_deliver_through_the_hub -- --exact \
  node::tests::two_nodes_deliver_through_the_hub 2>&1 | grep -E "test result|panicked"

sleep 2
echo "a: $(count a bundles) bundles, $(count a tombstones) tombstones"
echo "b: $(count b bundles) bundles, $(count b tombstones) tombstones"

start c
for _ in $(seq 1 20); do
  [ "$(count c tombstones)" -ge "$(count a tombstones)" ] && break
  sleep 1
done
echo "c (started late): $(count c bundles) bundles, $(count c tombstones) tombstones"

[ "$(count a bundles)" = 0 ] && [ "$(count b bundles)" = 0 ] && [ "$(count c bundles)" = 0 ] \
  && [ "$(count c tombstones)" -ge 2 ] && [ "$(count c tombstones)" = "$(count a tombstones)" ] \
  && echo "one operator: OK" || { echo "network test: FAILED ($work)"; exit 1; }

# Partner operators: nothing of the first exchange crossed (those nodes had no list).
[ "$(count p bundles)" = 0 ] || { echo "network test: FAILED, P got bundles ($work)"; exit 1; }
before=$(count p tombstones)
MEM_OP1_URL=ws://127.0.0.1:4301/v1 MEM_OP2_URL=ws://127.0.0.1:4304/v1 \
  MEM_OP1_LIST="$work/hubs.cbor" MEM_OP2_LIST="$work/hubs2.cbor" MEM_OP1_KEY="$op" MEM_OP2_KEY="$op2" \
  cargo test -q -p mem-runtime two_nodes_deliver_across_operators >"$work/partner-test.log" 2>&1 \
  || { grep -E "panicked|test result" "$work/partner-test.log"; echo "network test: FAILED ($work)"; exit 1; }
grep -E "test result" "$work/partner-test.log"
for _ in $(seq 1 20); do
  [ "$(count a bundles)" = 0 ] && [ "$(count p bundles)" = 0 ] && break
  sleep 1
done
echo "a: $(count a bundles) bundles, $(count a tombstones) tombstones"
echo "p (other operator): $(count p bundles) bundles, $(count p tombstones) tombstones"
[ "$(count a bundles)" = 0 ] && [ "$(count p bundles)" = 0 ] && [ "$(count p tombstones)" -ge $((before + 2)) ] \
  && echo "network test: OK" || { echo "network test: FAILED ($work)"; exit 1; }
