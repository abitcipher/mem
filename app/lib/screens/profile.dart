import 'package:flutter/material.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';

class ProfileScreen extends StatefulWidget {
  const ProfileScreen({super.key});

  @override
  State<ProfileScreen> createState() => _ProfileScreenState();
}

class _ProfileScreenState extends State<ProfileScreen> {
  late final TextEditingController _name;

  @override
  void initState() {
    super.initState();
    _name = TextEditingController(
      text: AppScope.read(context).node.profile().displayName ?? '',
    );
  }

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final profile = state.node.profile();
    return Scaffold(
      appBar: AppBar(title: Text(l.profile)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextField(
            controller: _name,
            maxLength: 64,
            decoration: InputDecoration(
              labelText: l.yourName,
              border: const OutlineInputBorder(),
            ),
          ),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton(
              onPressed: () async {
                try {
                  await state.node.setDisplayName(name: _name.text);
                  state.changed();
                  if (context.mounted) showSnack(context, l.saved);
                } on Object catch (e) {
                  if (context.mounted) {
                    showSnack(context, reasonText(context, e));
                  }
                }
              },
              child: Text(l.save),
            ),
          ),
          const SizedBox(height: 24),
          Text(
            l.yourFingerprint,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          SelectableText(
            profile.fingerprint,
            style: const TextStyle(fontFamily: 'monospace'),
          ),
        ],
      ),
    );
  }
}
