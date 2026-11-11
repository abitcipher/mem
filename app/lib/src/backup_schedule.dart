import 'dart:async';
import 'dart:io';

import '../src/platform.dart';
import 'rust/api.dart';

/// When automatic backups run: a user setting, off by default.
enum BackupSchedule { off, everyDays, afterNewContact }

/// Automatic-backup settings, stored as `key=value` lines in a small UI file next to the profile.
/// No secret material: the folder location and timing only.
class BackupSettings {
  BackupSettings({
    this.schedule = BackupSchedule.off,
    this.days = 7,
    this.folder,
    this.history = true,
    this.lastUnixMs,
    this.lastFailed = false,
  });

  /// Bounds for the "every N days" period.
  static const minDays = 1;
  static const maxDays = 90;

  BackupSchedule schedule;
  int days;
  String? folder;
  bool history;
  int? lastUnixMs;
  bool lastFailed;

  static BackupSettings read(File? file) {
    final s = BackupSettings();
    try {
      for (final line in file?.readAsLinesSync() ?? const <String>[]) {
        final i = line.indexOf('=');
        if (i <= 0) continue;
        final (k, v) = (line.substring(0, i), line.substring(i + 1));
        switch (k) {
          case 'schedule':
            s.schedule = BackupSchedule.values.firstWhere(
              (e) => e.name == v,
              orElse: () => BackupSchedule.off,
            );
          case 'days':
            s.days = (int.tryParse(v) ?? 7).clamp(minDays, maxDays);
          case 'folder':
            s.folder = v.isEmpty ? null : v;
          case 'history':
            s.history = v == 'true';
          case 'last':
            s.lastUnixMs = int.tryParse(v);
          case 'failed':
            s.lastFailed = v == 'true';
        }
      }
    } on FileSystemException {
      // Defaults.
    }
    return s;
  }

  void write(File? file) {
    try {
      file?.writeAsStringSync(
        [
          'schedule=${schedule.name}',
          'days=$days',
          'folder=${folder ?? ''}',
          'history=$history',
          if (lastUnixMs != null) 'last=$lastUnixMs',
          'failed=$lastFailed',
        ].join('\n'),
      );
    } on FileSystemException {
      // Keeps working in memory.
    }
  }

  /// Whether a scheduled backup is due now.
  bool due({required bool contactAdded}) {
    if (folder == null) return false;
    return switch (schedule) {
      BackupSchedule.off => false,
      BackupSchedule.afterNewContact => contactAdded,
      BackupSchedule.everyDays =>
        lastUnixMs == null ||
            DateTime.now().millisecondsSinceEpoch - lastUnixMs! >=
                days * Duration.millisecondsPerDay,
    };
  }
}

/// Runs scheduled backups: at start, hourly while the app is open, and after a new contact.
class BackupScheduler {
  BackupScheduler(this.node, this.file, this.onChanged) {
    settings = BackupSettings.read(file);
    _timer = Timer.periodic(const Duration(hours: 1), (_) => maybeRun());
    unawaited(maybeRun());
  }

  final MemNode node;
  final File? file;
  final void Function() onChanged;
  late BackupSettings settings;
  late final Timer _timer;
  bool _running = false;

  void save() {
    settings.write(file);
    onChanged();
  }

  /// Makes a backup into the chosen folder if one is due. Failures are only recorded locally.
  Future<void> maybeRun({bool contactAdded = false}) async {
    if (_running || !settings.due(contactAdded: contactAdded)) return;
    if (!node.backupStatus().enabled) return;
    _running = true;
    try {
      await runNow();
    } finally {
      _running = false;
    }
  }

  /// Writes a backup into the chosen folder now. Returns whether it worked.
  Future<bool> runNow() async {
    final folder = settings.folder;
    if (folder == null) return false;
    var ok = false;
    try {
      final bytes = await node.makeBackup(history: settings.history);
      ok = await PlatformServices.writeToFolder(
        folder,
        backupFileName(),
        bytes,
      );
    } on Object {
      ok = false;
    }
    settings.lastFailed = !ok;
    if (ok) settings.lastUnixMs = DateTime.now().millisecondsSinceEpoch;
    save();
    return ok;
  }

  void dispose() => _timer.cancel();
}

/// `mem-backup-2026-10-15-1703.membackup` (local time).
String backupFileName() {
  final t = DateTime.now();
  String two(int v) => v.toString().padLeft(2, '0');
  return 'mem-backup-${t.year}-${two(t.month)}-${two(t.day)}-'
      '${two(t.hour)}${two(t.minute)}.membackup';
}
