import 'package:material_ui/material_ui.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mem_netmap/mem_netmap.dart';

const _green = Color(0xFF2E7D32);

NetMapData _data({NetNodeState wifi = NetNodeState.up}) => NetMapData(
  nodes: [
    const NetNode(
      id: 'me',
      ring: NetRing.centre,
      label: 'This phone',
      icon: Icons.smartphone,
    ),
    NetNode(
      id: 'wifi',
      ring: NetRing.inner,
      label: 'Wi-Fi',
      icon: Icons.wifi,
      state: wifi,
    ),
    const NetNode(
      id: 'hub',
      ring: NetRing.inner,
      label: 'Hub',
      icon: Icons.cloud_outlined,
      acceptsDrop: true,
    ),
    const NetNode(
      id: 'bt',
      ring: NetRing.inner,
      label: 'Bluetooth',
      icon: Icons.bluetooth,
    ),
    const NetNode(
      id: 'olena',
      ring: NetRing.outer,
      label: 'Olena',
      icon: Icons.person,
      draggable: true,
    ),
    const NetNode(
      id: 'taras',
      ring: NetRing.outer,
      label: 'Taras',
      icon: Icons.person,
    ),
  ],
  edges: const [
    NetEdge(
      from: 'me',
      to: 'wifi',
      style: NetEdgeStyle(color: _green),
    ),
    NetEdge(
      from: 'me',
      to: 'hub',
      style: NetEdgeStyle(color: Colors.blue),
    ),
    NetEdge(
      from: 'me',
      to: 'bt',
      style: NetEdgeStyle(color: Colors.grey),
    ),
    NetEdge(
      from: 'wifi',
      to: 'olena',
      style: NetEdgeStyle(color: _green, flow: NetFlow.forward, label: 'P1'),
    ),
    NetEdge(
      from: 'hub',
      to: 'taras',
      style: NetEdgeStyle(color: Colors.blue, dashed: true),
    ),
  ],
);

void main() {
  test('the layout puts the centre in the middle, rings at their radii, and contacts near '
      'their channel', () {
    const layout = RadialLayout(innerRadius: 100, outerRadius: 200);
    final p = layout.place(_data());
    final (inner, outer) = layout.radii(_data());
    expect(inner, greaterThanOrEqualTo(100));
    // The outer ring stays clear of the inner ring's cards.
    expect(outer - inner, greaterThanOrEqualTo(layout.spacing));
    expect(p['me'], Offset.zero);
    for (final id in ['wifi', 'hub', 'bt']) {
      expect(p[id]!.distance, closeTo(inner, 0.001));
    }
    for (final id in ['olena', 'taras']) {
      expect(p[id]!.distance, closeTo(outer, 0.001));
    }
    // Olena (on Wi-Fi) is nearer Wi-Fi than the hub; Taras (via the hub) nearer the hub.
    double d(String a, String b) => (p[a]! - p[b]!).distance;
    expect(d('olena', 'wifi'), lessThan(d('olena', 'hub')));
    expect(d('taras', 'hub'), lessThan(d('taras', 'wifi')));
    // Deterministic.
    expect(layout.place(_data()), p);
  });

  Future<void> pump(
    WidgetTester tester, {
    NetMapData? data,
    NetMapController? controller,
    void Function(String)? onTap,
    void Function(String, String)? onDrop,
    Brightness brightness = Brightness.dark,
  }) => tester.pumpWidget(
    MaterialApp(
      theme: ThemeData(brightness: brightness),
      home: Scaffold(
        body: NetMapView(
          data: data ?? _data(),
          controller: controller,
          onTapNode: onTap,
          onDropNode: onDrop,
        ),
      ),
    ),
  );

  testWidgets('tapping a node reports it', (tester) async {
    String? tapped;
    await pump(tester, onTap: (id) => tapped = id);
    await tester.tap(find.text('Bluetooth'));
    expect(tapped, 'bt');
    // The view animates (moving dots); stop it before the test ends.
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('dragging a contact onto a hub asks to connect them', (
    tester,
  ) async {
    (String, String)? dropped;
    await pump(tester, onDrop: (a, b) => dropped = (a, b));
    final from = tester.getCenter(find.text('Olena'));
    final to = tester.getCenter(find.text('Hub'));
    final g = await tester.startGesture(from);
    await tester.pump(kLongPressTimeout + const Duration(milliseconds: 100));
    for (var i = 1; i <= 10; i++) {
      await g.moveTo(Offset.lerp(from, to, i / 10)!);
      await tester.pump(const Duration(milliseconds: 16));
    }
    await g.up();
    await tester.pump();
    expect(dropped, ('olena', 'hub'));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('dropping on a node that takes no drops does nothing', (
    tester,
  ) async {
    (String, String)? dropped;
    await pump(tester, onDrop: (a, b) => dropped = (a, b));
    final from = tester.getCenter(find.text('Olena'));
    final g = await tester.startGesture(from);
    await tester.pump(kLongPressTimeout + const Duration(milliseconds: 100));
    await g.moveTo(tester.getCenter(find.text('Bluetooth')));
    await tester.pump();
    await g.up();
    await tester.pump();
    expect(dropped, isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('pulses, ripples and both themes draw without errors', (
    tester,
  ) async {
    final controller = NetMapController();
    for (final b in Brightness.values) {
      await pump(tester, controller: controller, brightness: b);
      controller.pulse('wifi', 'olena');
      controller.pulse('taras', 'hub', color: Colors.orange);
      await tester.pump(const Duration(milliseconds: 300));
      // Wi-Fi goes down: a ripple.
      await pump(
        tester,
        data: _data(wifi: NetNodeState.off),
        controller: controller,
        brightness: b,
      );
      await tester.pump(const Duration(milliseconds: 500));
      await pump(
        tester,
        data: _data(wifi: NetNodeState.connecting),
        controller: controller,
        brightness: b,
      );
      await tester.pump(const Duration(seconds: 2));
      expect(tester.takeException(), isNull);
    }
    await tester.pumpWidget(const SizedBox());
    controller.dispose();
  });
}
