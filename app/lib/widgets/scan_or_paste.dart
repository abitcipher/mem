import 'dart:io';

import 'package:camera/camera.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import 'glow.dart';
import '../screens/backup.dart';

/// Camera scanner (where supported) plus a paste box.
///
/// Camera images go to the Rust core as luma planes (decoded with rxing, no
/// third-party scanning SDK), which reassembles animated QR transfers and hands complete ones
/// to the node over the QR channel (P0); [onScanned] gets the outcome. Pasted text came
/// through some other app (P4) and goes to [onText].
class ScanOrPaste extends StatefulWidget {
  const ScanOrPaste({required this.onText, required this.onScanned, super.key});

  final Future<void> Function(String text, Channel channel) onText;
  final Future<void> Function(ReceiveOutcome outcome) onScanned;

  static bool get cameraSupported => Platform.isAndroid || Platform.isIOS;

  @override
  State<ScanOrPaste> createState() => _ScanOrPasteState();
}

class _ScanOrPasteState extends State<ScanOrPaste> {
  final _controller = TextEditingController();
  bool _busy = false;

  Future<void> _submit(String text, Channel channel) async {
    if (_busy || text.trim().isEmpty) return;
    setState(() => _busy = true);
    try {
      await widget.onText(text, channel);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        if (ScanOrPaste.cameraSupported)
          CameraScanner(onScanned: widget.onScanned)
        else
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  const Icon(Icons.no_photography_outlined),
                  const SizedBox(width: 12),
                  Expanded(child: Text(l.cameraUnavailable)),
                ],
              ),
            ),
          ),
        const SizedBox(height: 24),
        TextField(
          controller: _controller,
          minLines: 2,
          maxLines: 5,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            hintText: l.pasteHint,
          ),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.content_paste),
              label: Text(l.pasteFromClipboard),
              onPressed: () async {
                final data = await Clipboard.getData(Clipboard.kTextPlain);
                if (data?.text != null) _controller.text = data!.text!;
              },
            ),
            const Spacer(),
            FilledButton(
              onPressed: _busy
                  ? null
                  : () => _submit(_controller.text, Channel.share),
              child: Text(l.open),
            ),
          ],
        ),
      ],
    );
  }
}

/// Live camera preview feeding luma frames to a [QrScanner], one at a time (frames that
/// arrive while one is being decoded are skipped).
class CameraScanner extends StatefulWidget {
  const CameraScanner({
    required this.onScanned,
    this.onBackup,
    this.onDeviceSetup,
    this.onDeviceRequest,
    this.onAttest,
    super.key,
  });

  final Future<void> Function(ReceiveOutcome outcome) onScanned;

  /// An encrypted backup arrived. Defaults to opening the restore screen with it.
  final Future<void> Function(Uint8List backup)? onBackup;

  /// A sealed device setup: only the screen of a device waiting to be added
  /// passes this; elsewhere such a code is ignored.
  final Future<void> Function(Uint8List setup)? onDeviceSetup;

  /// A new device's request: only the main device's "Add a device" screen
  /// passes this; elsewhere such a code is ignored.
  final Future<void> Function(Uint8List request)? onDeviceRequest;

  /// An attestation code (a service's request or another phone's offer): only the
  /// "Confirm in person" screen passes this; elsewhere such a code is ignored.
  final Future<void> Function(String text)? onAttest;

  @override
  State<CameraScanner> createState() => _CameraScannerState();
}

class _CameraScannerState extends State<CameraScanner>
    with WidgetsBindingObserver {
  CameraController? _camera;
  final QrScanner _scanner = newQrScanner();
  bool _decoding = false;
  bool _handling = false;
  int _received = 0;
  int _needed = 0;
  Object? _error;

  /// Exposure correction in EV, within the camera's range; no range, no slider.
  double _exposureMin = 0;
  double _exposureMax = 0;
  double _exposure = 0;

  /// Where the user last tapped to focus, in the preview box; shown briefly as a ring.
  Offset? _focusRing;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _start();
  }

  Future<void> _start() async {
    try {
      final cameras = await availableCameras();
      final back = cameras.firstWhere(
        (c) => c.lensDirection == CameraLensDirection.back,
        orElse: () => cameras.first,
      );
      // The sharpest picture the camera can stream alongside its preview: some cameras (older
      // phones, emulated webcams) refuse the high resolution with "no supported surface
      // combination", and work one step down.
      Object? failure;
      for (final preset in const [
        ResolutionPreset.high,
        ResolutionPreset.medium,
        ResolutionPreset.low,
      ]) {
        final camera = CameraController(
          back,
          preset,
          enableAudio: false,
          // Plane 0 is the luma (Y) plane on both Android and iOS.
          imageFormatGroup: ImageFormatGroup.yuv420,
        );
        try {
          await camera.initialize();
          if (!mounted) {
            await camera.dispose();
            return;
          }
          await camera.startImageStream(_onImage);
        } on Object catch (e) {
          // Why it did not start (no personal data in it): visible in the device log.
          debugPrint('MEM camera ($preset): $e');
          failure = e;
          try {
            await camera.dispose();
          } on Object {
            // A half-opened camera may fail to close; the next attempt opens it afresh.
          }
          continue;
        }
        setState(() => _camera = camera);
        // Meter and focus on the centre, where codes are decoded: auto-exposure over the
        // whole picture brightens a dim room and washes out a bright phone screen.
        await _meter(camera, const Offset(0.5, 0.5));
        await _exposureRange(camera);
        return;
      }
      if (mounted) setState(() => _error = failure);
    } on Object catch (e) {
      debugPrint('MEM camera: $e');
      if (mounted) setState(() => _error = e);
    }
  }

  /// The camera's exposure correction range, and the user's remembered correction applied
  /// within it; no slider when the camera has none.
  Future<void> _exposureRange(CameraController camera) async {
    try {
      final min = await camera.getMinExposureOffset();
      final max = await camera.getMaxExposureOffset();
      if (!mounted || max <= min) return;
      final wanted = AppScope.read(context).settings.scanExposure
          .clamp(min, max);
      setState(() {
        _exposureMin = min;
        _exposureMax = max;
        _exposure = wanted;
      });
      if (wanted != 0) await camera.setExposureOffset(wanted);
    } on CameraException {
      // No exposure control here: the camera keeps its automatic exposure.
    }
  }

  /// Sets the brightness correction; remembered when the user lets go of the slider.
  Future<void> _setExposure(double value, {bool remember = false}) async {
    setState(() => _exposure = value);
    if (remember) {
      AppScope.read(context).updateSettings((s) => s.scanExposure = value);
    }
    try {
      await _camera?.setExposureOffset(value);
    } on CameraException {
      // Ignored: the picture stays as it is.
    }
  }

  /// Focuses and meters exposure at `point` (0..1 in the preview), where the camera supports it.
  Future<void> _meter(CameraController camera, Offset point) async {
    try {
      if (camera.value.focusPointSupported) {
        await camera.setFocusPoint(point);
        await camera.setFocusMode(FocusMode.auto);
      }
      if (camera.value.exposurePointSupported) {
        await camera.setExposurePoint(point);
      }
    } on CameraException {
      // Some devices refuse; the camera keeps its own automatic settings.
    }
  }

  /// Tap to focus: maps the tap through the cropped (cover) preview to camera coordinates.
  void _tapToFocus(TapUpDetails tap, Size box) {
    final camera = _camera;
    final preview = camera?.value.previewSize;
    if (camera == null || preview == null) return;
    // The preview is shown upright: its width is the sensor's height.
    final child = Size(preview.height, preview.width);
    final scale = [
      box.width / child.width,
      box.height / child.height,
    ].reduce((a, b) => a > b ? a : b);
    final shown = child * scale;
    final origin = Offset(
      (box.width - shown.width) / 2,
      (box.height - shown.height) / 2,
    );
    final p = tap.localPosition - origin;
    final point = Offset(
      (p.dx / shown.width).clamp(0.0, 1.0),
      (p.dy / shown.height).clamp(0.0, 1.0),
    );
    setState(() => _focusRing = tap.localPosition);
    Future<void>.delayed(const Duration(milliseconds: 700), () {
      if (mounted) setState(() => _focusRing = null);
    });
    _meter(camera, point);
  }

  Future<void> _stop() async {
    final camera = _camera;
    _camera = null;
    if (camera == null) return;
    if (camera.value.isStreamingImages) await camera.stopImageStream();
    await camera.dispose();
  }

  void _onImage(CameraImage image) {
    if (_decoding || _handling || !mounted) return;
    _decoding = true;
    final y = image.planes.first;
    AppScope.read(context).node
        .scanFrame(
          scanner: _scanner,
          luma: y.bytes,
          width: image.width,
          height: image.height,
          rowStride: y.bytesPerRow,
        )
        .then(_onUpdate, onError: (Object e) => _error = e)
        .whenComplete(() => _decoding = false);
  }

  /// Hands a device or attestation code to the screen that asked for it; ignored elsewhere.
  Future<void> _handle<T>(Future<void> Function(T)? handler, T data) async {
    if (handler == null) return;
    _handling = true;
    // The transfer is over, whatever the screen makes of it: later frames of the same code
    // are ignored, so a stale count would never move again.
    setState(() {
      _received = 0;
      _needed = 0;
    });
    HapticFeedback.mediumImpact();
    try {
      await handler(data);
    } finally {
      _handling = false;
    }
  }

  Future<void> _onUpdate(ScanUpdate update) async {
    if (!mounted) return;
    switch (update) {
      case ScanUpdate_Nothing():
        return;
      case ScanUpdate_DeviceSetup(:final field0):
        await _handle(widget.onDeviceSetup, field0);
      case ScanUpdate_Connecting():
        // A connect hint was read and is being dialled.
        setState(() {
          _received = 0;
          _needed = 0;
        });
        HapticFeedback.mediumImpact();
        AppScope.read(context).changed();
        showSnack(context, AppLocalizations.of(context).connectGot);
      case ScanUpdate_DeviceRequest(:final field0):
        await _handle(widget.onDeviceRequest, field0);
      case ScanUpdate_Attest(:final field0):
        await _handle(widget.onAttest, field0);
      case ScanUpdate_Progress(:final received, :final needed):
        setState(() {
          _received = received;
          _needed = needed;
        });
      case ScanUpdate_Backup(:final field0):
        _handling = true;
        setState(() {
          _received = 0;
          _needed = 0;
        });
        HapticFeedback.mediumImpact();
        try {
          final onBackup = widget.onBackup;
          if (onBackup != null) {
            await onBackup(field0);
          } else {
            await Navigator.of(context).push(
              MaterialPageRoute<void>(
                builder: (_) => RestoreScreen(initial: field0),
              ),
            );
          }
        } finally {
          _handling = false;
        }
      case ScanUpdate_Received(:final field0):
        _handling = true;
        setState(() {
          _received = 0;
          _needed = 0;
        });
        AppScope.read(context).changed();
        HapticFeedback.mediumImpact();
        try {
          await widget.onScanned(field0);
        } finally {
          _handling = false;
        }
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.inactive) {
      _stop();
    } else if (state == AppLifecycleState.resumed && _camera == null) {
      _start();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final camera = _camera;
    return Column(
      children: [
        if (camera != null && _exposureMax > _exposureMin)
          // A phone screen is much brighter than the room: the camera washes it out. The user
          // darkens (or brightens) the picture here; the choice is remembered.
          Row(
            children: [
              Tooltip(
                message: l.scanBrightness,
                child: const Icon(Icons.brightness_6, size: 20),
              ),
              Expanded(
                child: Slider(
                  min: _exposureMin,
                  max: _exposureMax,
                  value: _exposure.clamp(_exposureMin, _exposureMax),
                  label: _exposure.toStringAsFixed(1),
                  semanticFormatterCallback: (v) =>
                      '${l.scanBrightness} ${v.toStringAsFixed(1)}',
                  onChanged: _setExposure,
                  onChangeEnd: (v) => _setExposure(v, remember: true),
                ),
              ),
            ],
          ),
        RainbowGlow(
          // A transfer in progress is an event.
          active: _needed > 0,
          animate: true,
          radius: 16,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(16),
            child: SizedBox(
              width: double.infinity,
              height: 320,
              child: camera == null
                  ? Center(
                      child: _error == null
                          ? const CircularProgressIndicator()
                          : Padding(
                              padding: const EdgeInsets.all(16),
                              child: Text(l.cameraUnavailable),
                            ),
                    )
                  : LayoutBuilder(
                      builder: (context, box) => GestureDetector(
                        onTapUp: (tap) => _tapToFocus(tap, box.biggest),
                        child: Stack(
                          children: [
                            Positioned.fill(
                              child: FittedBox(
                                fit: BoxFit.cover,
                                clipBehavior: Clip.hardEdge,
                                child: SizedBox(
                                  width:
                                      camera.value.previewSize?.height ?? 320,
                                  height:
                                      camera.value.previewSize?.width ?? 320,
                                  child: CameraPreview(camera),
                                ),
                              ),
                            ),
                            if (_focusRing case final at?)
                              Positioned(
                                left: at.dx - 28,
                                top: at.dy - 28,
                                child: IgnorePointer(
                                  child: Container(
                                    width: 56,
                                    height: 56,
                                    decoration: BoxDecoration(
                                      shape: BoxShape.circle,
                                      border: Border.all(
                                        color: Colors.white,
                                        width: 1.5,
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
            ),
          ),
        ),
        const SizedBox(height: 8),
        if (_needed > 0) ...[
          LinearProgressIndicator(value: (_received / _needed).clamp(0.0, 1.0)),
          const SizedBox(height: 4),
          Text(l.scanProgress(_received, _needed)),
        ] else
          Text(
            '${l.scanHint}\n${l.scanFocusHint}',
            textAlign: TextAlign.center,
          ),
      ],
    );
  }
}
