import 'dart:io';

import 'package:cupertino_ui/cupertino_ui.dart'
    show GlobalCupertinoLocalizations;
import 'package:flutter_localizations/flutter_localizations.dart'
    show GlobalWidgetsLocalizations;
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

import 'app_scope.dart';
import 'l10n/gen/app_localizations.dart';
import 'screens/home.dart';
import 'src/rust/api.dart';
import 'src/rust/frb_generated.dart';
import 'src/ui_settings.dart';
import 'widgets/attachments.dart';
import 'widgets/reachable.dart';
import 'theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  // Decrypted copies never outlive the run that made them.
  await clearDecryptedCopies();
  final dir = await getApplicationSupportDirectory();
  final node = await openNode(
    dir: '${dir.path}${Platform.pathSeparator}profile',
  );
  runApp(
    MemApp(
      state: AppState(
        node,
        themeFile: File('${dir.path}${Platform.pathSeparator}ui_theme'),
        backupFile: File('${dir.path}${Platform.pathSeparator}ui_backup'),
        settingsFile: File('${dir.path}${Platform.pathSeparator}ui_settings'),
      ),
    ),
  );
}

class MemApp extends StatelessWidget {
  const MemApp({required this.state, super.key});

  final AppState state;

  @override
  Widget build(BuildContext context) {
    return AppScope(
      state: state,
      child: ListenableBuilder(
        listenable: state,
        builder: (context, _) => MaterialApp(
          navigatorKey: appNavigator,
          onGenerateTitle: (context) => AppLocalizations.of(context).appTitle,
          theme: memTheme(AppTheme.white),
          darkTheme: memTheme(
            state.theme == AppTheme.gray ? AppTheme.gray : AppTheme.black,
          ),
          themeMode: switch (state.theme) {
            AppTheme.system => ThemeMode.system,
            AppTheme.white => ThemeMode.light,
            AppTheme.gray || AppTheme.black => ThemeMode.dark,
          },
          localizationsDelegates: const [
            AppLocalizations.delegate,
            GlobalMaterialLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
            GlobalCupertinoLocalizations.delegate,
          ],
          supportedLocales: AppLocalizations.supportedLocales,
          locale: switch (state.settings.language) {
            AppLanguage.system => null,
            AppLanguage.en => const Locale('en'),
            AppLanguage.uk => const Locale('uk'),
          },
          // A fixed text size replaces the system's; otherwise the system's applies.
          // The same widgets above the navigator whatever the setting: a different wrapper
          // would rebuild the navigator and close every open screen and dialog (seen when the
          // saved settings load just after a cold start).
          builder: (context, child) {
            final percent = state.settings.textPercent;
            final media = MediaQuery.of(context);
            return MediaQuery(
              data: percent == null
                  ? media
                  : media.copyWith(
                      textScaler: TextScaler.linear(percent / 100),
                    ),
              child: ReachableSync(child: child ?? const SizedBox.shrink()),
            );
          },
          home: const HomeScreen(),
        ),
      ),
    );
  }
}
