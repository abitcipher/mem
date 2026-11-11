// MEM's addition to the vendored ggwave (not part of upstream; the vendored files stay
// unchanged). ggwave keeps its instances in an anonymous namespace, so this file compiles
// ggwave.cpp in its own translation unit and adds one accessor: whether an instance is in the
// middle of receiving a transmission, so a phone can wait before speaking.
#include "../vendor/ggwave/src/ggwave.cpp"

extern "C" int mem_ggwave_rx_receiving(ggwave_Instance id) {
    if (id < 0 || id >= GGWAVE_MAX_INSTANCES || g_instances[id] == nullptr) {
        return 0;
    }
    return g_instances[id]->rxReceiving() ? 1 : 0;
}
