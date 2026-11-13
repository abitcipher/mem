/// Where a call's media goes.
enum MemCallPath {
  /// Face to face over Wi-Fi Direct.
  wifiDirect,

  /// Directly over the local network.
  localNetwork,

  /// Through STUN/TURN servers.
  servers,
}

/// The path of a local candidate of type [candidateType] (`host`, `srflx`, `prflx`, `relay`)
/// at [address]; `null` if unknown.
MemCallPath? pathOf(String? candidateType, String? address) {
  switch (candidateType) {
    case 'host':
      // Android's Wi-Fi Direct groups use 192.168.49.0/24.
      return (address ?? '').startsWith('192.168.49.')
          ? MemCallPath.wifiDirect
          : MemCallPath.localNetwork;
    case 'srflx' || 'prflx' || 'relay':
      return MemCallPath.servers;
    default:
      return null;
  }
}
