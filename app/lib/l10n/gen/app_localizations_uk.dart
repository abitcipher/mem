// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Ukrainian (`uk`).
class AppLocalizationsUk extends AppLocalizations {
  AppLocalizationsUk([String locale = 'uk']) : super(locale);

  @override
  String get appTitle => 'MEM';

  @override
  String get conversations => 'Розмови';

  @override
  String get noContactsTitle => 'Контактів ще немає';

  @override
  String get noContactsBody =>
      'MEM спілкується лише з тими, кого ви схвалили. Зустріньтеся особисто й відскануйте коди одне одного або обміняйтеся кодами через інший застосунок.';

  @override
  String get addContact => 'Додати контакт';

  @override
  String get receive => 'Отримати';

  @override
  String get privacyLog => 'Журнал приватності';

  @override
  String get profile => 'Профіль';

  @override
  String get pairTitle => 'Додати контакт';

  @override
  String get myCodeTab => 'Мій код';

  @override
  String get scanTab => 'Сканувати або вставити';

  @override
  String get myCodeHint =>
      'Дайте контакту відсканувати цей код. Потім відскануйте його код.';

  @override
  String get copyCode => 'Копіювати код';

  @override
  String get shareCode => 'Поділитися кодом…';

  @override
  String get newCode => 'Новий код для пари';

  @override
  String get codeCopied => 'Код скопійовано';

  @override
  String get shareCodeWarning =>
      'Якщо поділитися кодом через інший застосунок, він дізнається, що ви користуєтеся MEM, і зможе пов\'язати вашу особу в MEM з вашим обліковим записом. Особиста зустріч і сканування — приватніше.';

  @override
  String get continueAction => 'Продовжити';

  @override
  String get cancel => 'Скасувати';

  @override
  String get scanHint => 'Наведіть камеру на код MEM';

  @override
  String get pasteHint => 'Вставте сюди код або повідомлення MEM';

  @override
  String get pasteFromClipboard => 'Вставити з буфера';

  @override
  String get open => 'Відкрити';

  @override
  String get cameraUnavailable =>
      'Сканування камерою недоступне на цьому пристрої. Вставте текст.';

  @override
  String get contactAddedTitle => 'Контакт додано';

  @override
  String get contactUpdatedTitle => 'Контакт оновлено';

  @override
  String get compareSas =>
      'Порівняйте ці символи з тими, що на екрані вашого контакту. Якщо вони збігаються — посередника немає.';

  @override
  String get sasMatch => 'Збігаються';

  @override
  String get sasMismatch => 'Не збігаються';

  @override
  String get sasMismatchBody =>
      'Попросіть контакт показати новий код і з\'єднайтеся ще раз. Не надсилайте нічого чутливого, доки символи не збіжуться.';

  @override
  String get fingerprint => 'Відбиток';

  @override
  String get metInPerson => 'Зустрілися особисто';

  @override
  String get remote => 'З\'єднано віддалено';

  @override
  String get messageReceived => 'Повідомлення отримано';

  @override
  String get notMem => 'Цей текст не містить коду чи повідомлення MEM.';

  @override
  String get typeMessage => 'Повідомлення';

  @override
  String get tapToSend => 'Ще не надіслано — натисніть, щоб обрати спосіб';

  @override
  String get chooseChannel => 'Як надіслати це повідомлення?';

  @override
  String get needsConsent =>
      'Нижче вашого порогу приватності — ви побачите, що буде відкрито';

  @override
  String get tooLarge => 'Задовге для цього каналу';

  @override
  String get showQrTitle => 'Покажіть це контакту';

  @override
  String get showQrHint =>
      'Контакт відкриває MEM → Отримати й наводить камеру на цей екран.';

  @override
  String get done => 'Готово';

  @override
  String get shareTitle => 'Надіслати через інший застосунок';

  @override
  String get shareHint =>
      'Надішліть цей текст контакту будь-яким застосунком. Він вставить його в MEM → Отримати.';

  @override
  String get copyText => 'Копіювати';

  @override
  String get shareText => 'Поділитися…';

  @override
  String get copied => 'Скопійовано';

  @override
  String get consentTitle => 'Перевірка приватності';

  @override
  String whoCanSee(String observers) {
    return 'Хто може бачити: $observers';
  }

  @override
  String get theyLearn => 'Вони дізнаються';

  @override
  String get theyLearnPartly => 'Вони можуть частково дізнатися';

  @override
  String get staysPrivate => 'Залишається приватним';

  @override
  String get allowOnce => 'Дозволити для цього повідомлення';

  @override
  String get allowContact => 'Дозволити для цього контакту…';

  @override
  String get dontSend => 'Не надсилати';

  @override
  String get confirmAlwaysTitle => 'Дозволити без запитання?';

  @override
  String get confirmAlwaysBody =>
      'Наступні повідомлення цьому контакту можуть іти цим каналом без цього вікна протягом обраного часу. Кожне таке надсилання все одно з\'явиться в Журналі приватності, а дозвіл можна скасувати в налаштуваннях контакту.';

  @override
  String get allow => 'Дозволити';

  @override
  String viaChannel(String channel) {
    return 'через $channel';
  }

  @override
  String get channelQr => 'QR';

  @override
  String get channelShare => 'інший застосунок';

  @override
  String get contactSettings => 'Контакт';

  @override
  String get privacyFloor => 'Поріг приватності';

  @override
  String get privacyFloorHelp =>
      'Канали не нижче цього рівня використовуються без запитання. Нижчі рівні завжди потребують дозволу й записуються в Журнал приватності.';

  @override
  String get grade0 => 'P0 · особисто (QR, радіо)';

  @override
  String get grade1 => 'P1 · пряма мережа';

  @override
  String get grade2 => 'P2 · власний ретранслятор або хаб';

  @override
  String get grade3 => 'P3 · кур\'єри MEM';

  @override
  String get grade4 => 'P4 · інші застосунки (зашифровано)';

  @override
  String get grade5 => 'P5 · відкритий текст';

  @override
  String get standingRules => 'Постійні дозволи';

  @override
  String get noRules => 'Немає';

  @override
  String get revoke => 'Скасувати';

  @override
  String get allContacts => 'усі контакти';

  @override
  String get logEmpty => 'Жодне повідомлення не йшло нижче порогу приватності.';

  @override
  String get basisOnce => 'дозволено один раз';

  @override
  String get basisNewRule => 'дозволено, створено дозвіл';

  @override
  String get basisRule => 'дозволено постійним дозволом';

  @override
  String logEntry(String grade, String floor, String basis) {
    return '$grade нижче порогу $floor · $basis';
  }

  @override
  String get yourName => 'Ваше ім\'я (необов\'язково, видно у вашому коді)';

  @override
  String get save => 'Зберегти';

  @override
  String get saved => 'Збережено';

  @override
  String get yourFingerprint => 'Ваш відбиток';

  @override
  String get about => 'Про цю збірку';

  @override
  String get aboutBody =>
      'MEM v0.2 — експериментальна версія. Повідомлення наскрізно зашифровані (HPKE, Ed25519). Вони передаються анімованим QR-кодом при зустрічі, напряму через ту саму мережу Wi-Fi або як зашифрований текст через інший застосунок. Кур\'єри й хаб з\'являться пізніше. Не покладайтеся на неї для ризикованого спілкування до зовнішнього аудиту безпеки.';

  @override
  String error(String details) {
    return 'Щось пішло не так: $details';
  }

  @override
  String get reason_malformed => 'Це не дані MEM.';

  @override
  String get reason_unsupported => 'Створено новішою версією MEM.';

  @override
  String get reason_not_for_us =>
      'Адресовано не вам або від того, кого ви не додали.';

  @override
  String get reason_undecryptable =>
      'Не вдалося розшифрувати. Попросіть відправника з\'єднатися ще раз.';

  @override
  String get reason_bad_signature => 'Не вдалося перевірити відправника.';

  @override
  String get reason_duplicate => 'Це повідомлення у вас уже є.';

  @override
  String get reason_purged =>
      'Ви вже отримали це повідомлення (або відправник його відкликав).';

  @override
  String get reason_expired => 'Термін дії повідомлення минув.';

  @override
  String get reason_own_card => 'Це ваш власний код.';

  @override
  String get reason_unknown_contact => 'Невідомий контакт.';

  @override
  String get reason_unknown_message =>
      'Це повідомлення вже не можна надіслати.';

  @override
  String get reason_too_large_for_channel => 'Задовге для цього каналу.';

  @override
  String get reason_consent_mismatch => 'Дозвіл не збігся. Спробуйте ще раз.';

  @override
  String get reason_invalid_text => 'Повідомлення порожнє або задовге.';

  @override
  String get reason_file_too_large =>
      'Файл завеликий. MEM надсилає файли до 100 МБ, не більше 16 за раз.';

  @override
  String get reason_invalid_attachment =>
      'Цей файл не можна прикріпити: його назва або тип задовгі.';

  @override
  String attachmentSending(int sent, int total) {
    return 'Надсилаю $sent з $total';
  }

  @override
  String attachmentArriving(int have, int total) {
    return 'Надходить $have з $total';
  }

  @override
  String get attachmentMissing => 'Цього файлу немає на цьому пристрої.';

  @override
  String get attachmentOpenElsewhere => 'Відкрити в іншому застосунку';

  @override
  String get attachmentOpenElsewhereBody =>
      'Обраний застосунок отримає цей файл незашифрованим і може зберегти або вивантажити його.';

  @override
  String get attachmentOpenElsewhereAction => 'Обрати застосунок';

  @override
  String get playerPlay => 'Відтворити';

  @override
  String get playerPause => 'Пауза';

  @override
  String get playerCannotPlay =>
      'Цей файл не відтворюється тут. Відкрийте його в іншому застосунку кнопкою «Поділитися».';

  @override
  String get mediaPlayer => 'Вбудований медіаплеєр';

  @override
  String get mediaPlayerHelp =>
      'Відео й аудіо відтворюються всередині MEM: їх розшифровано в приватний тимчасовий файл, який видаляється, коли плеєр закривається. Якщо вимкнено, вони відкриваються в іншому застосунку, який тоді отримує файл.';

  @override
  String get reason_invalid_name => 'Ім\'я задовге.';

  @override
  String get channelLan => 'Wi-Fi';

  @override
  String get nearby => 'поруч';

  @override
  String nearbyDevices(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Поруч $count пристрою MEM',
      many: 'Поруч $count пристроїв MEM',
      few: 'Поруч $count пристрої MEM',
      one: 'Поруч 1 пристрій MEM',
      zero: 'Поруч немає пристроїв MEM',
    );
    return '$_temp0';
  }

  @override
  String get lanOffShort => 'Доставку через Wi-Fi вимкнено';

  @override
  String get notNearby => 'Зараз не поруч';

  @override
  String get sendingNearby => 'Надсилання на пристрій поруч…';

  @override
  String get waitForWifi => 'Дочекатися зустрічі';

  @override
  String get waitForWifiHint =>
      'Надішлеться автоматично, коли ви будете в одній Wi-Fi або поруч з увімкненим Bluetooth';

  @override
  String get channels => 'Канали';

  @override
  String get channelsIntro =>
      'Що можуть дізнатися інші, коли MEM використовує кожен канал. Вміст повідомлень завжди наскрізно зашифрований, окрім запасного варіанта P5, який ви підтверджуєте для кожного повідомлення.';

  @override
  String get lanToggle => 'Доставляти через локальну мережу';

  @override
  String get lanToggleHelp =>
      'Поки ввімкнено, інші пристрої в тій самій Wi-Fi бачать, що на цьому пристрої є MEM, і його мережеві адреси. Вимикайте в мережах, яким не довіряєте.';

  @override
  String scanProgress(int received, int needed) {
    return 'Отримання… $received з $needed';
  }

  @override
  String animatedQrHint(int needed) {
    return 'Тримайте на екрані, доки телефон контакту не отримає. Пропущені кадри не важливі: досить будь-яких $needed.';
  }

  @override
  String get reason_unreachable => 'Вашого контакту зараз немає поруч.';

  @override
  String get reason_channel_disabled =>
      'Цей канал вимкнено в розділі «Канали».';

  @override
  String get reason_quota =>
      'Забагато даних від пристрою поруч; з\'єднання розірвано.';

  @override
  String get reason_unknown => 'Сталося щось неочікуване.';

  @override
  String get appearance => 'Вигляд';

  @override
  String get themeSystem => 'Системна';

  @override
  String get themeWhite => 'Біла';

  @override
  String get themeGray => 'Сіра';

  @override
  String get themeBlack => 'Чорна';

  @override
  String get notConfirmed =>
      'Ще не підтверджено — натисніть, щоб надіслати ще раз';

  @override
  String get awaitingConfirmation => 'Очікує підтвердження';

  @override
  String get retryTitle => 'Повторювати непідтверджені повідомлення';

  @override
  String retryDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '$days дня',
      many: '$days днів',
      few: '$days дні',
      one: '$days день',
    );
    return '$_temp0';
  }

  @override
  String get retryHelp =>
      'Повідомлення, які контакт не підтвердив, надсилаються знову щоразу, коли ви на зв\'язку, протягом цього часу. Діє для нових повідомлень.';

  @override
  String get backupTitle => 'Резервна копія та відновлення';

  @override
  String get backupIntro =>
      'Резервна копія містить вашу особу, контакти, ключі та, якщо бажаєте, повідомлення. Вона зашифрована: відкрити її може лише ваша фраза відновлення. MEM ніде не зберігає копій.';

  @override
  String get backupOffBody =>
      'Увімкніть резервні копії, щоб отримати фразу відновлення. Запишіть її на папері й зберігайте окремо від файлів копій: той, хто має і те, і те, може читати ваші повідомлення та писати від вашого імені.';

  @override
  String get phraseFormTitle => 'Фраза відновлення';

  @override
  String get formEnglish => 'Англійські слова';

  @override
  String get formSyllables => 'Латинські склади';

  @override
  String get turnOnBackups => 'Увімкнути резервні копії';

  @override
  String get phraseTitle => 'Ваша фраза відновлення';

  @override
  String get phraseHint =>
      'Запишіть їх по порядку на папері. Знімки екрана тут заблоковано.';

  @override
  String get phraseWritten => 'Записано';

  @override
  String get confirmTitle => 'Перевірте фразу';

  @override
  String get confirmHint => 'Введіть ці слова зі свого запису.';

  @override
  String get confirmWrong => 'Не збігається. Перевірте свій запис.';

  @override
  String get confirmDone => 'Резервні копії увімкнено.';

  @override
  String wordN(int n) {
    return 'Слово $n';
  }

  @override
  String get includeHistory => 'Разом із повідомленнями';

  @override
  String get saveBackupFile => 'Зберегти файл копії';

  @override
  String get backupSaved => 'Копію збережено';

  @override
  String get showBackupQr => 'Показати як QR-код';

  @override
  String get backupQrHint =>
      'На новому телефоні відкрийте MEM → Профіль → Резервна копія та відновлення → Відновити з копії і наведіть камеру на цей екран.';

  @override
  String get backupTooLargeForQr =>
      'Із повідомленнями завелика для QR-коду: показую без повідомлень.';

  @override
  String get shareBackup => 'Надіслати через інший застосунок…';

  @override
  String get shareBackupWarning =>
      'Обраний застосунок побачить, що у вас є резервна копія MEM, її приблизний розмір і коли ви її надіслали. Прочитати її він не зможе.';

  @override
  String get showPhrase => 'Показати фразу відновлення';

  @override
  String get newPhrase => 'Нова фраза відновлення';

  @override
  String get newPhraseWarning =>
      'Нові копії відкриватимуться лише новою фразою. Старі файли копій і надалі відкриваються лише старою.';

  @override
  String get scheduleTitle => 'Автоматичні копії';

  @override
  String get scheduleOff => 'Вимкнено';

  @override
  String scheduleEveryDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Кожні $days дня',
      many: 'Кожні $days днів',
      few: 'Кожні $days дні',
      one: 'Кожен $days день',
    );
    return '$_temp0';
  }

  @override
  String get scheduleAfterContact => 'Після кожного нового контакту';

  @override
  String get backupFolder => 'Тека';

  @override
  String get chooseFolder => 'Вибрати';

  @override
  String get noFolder => 'Теку не вибрано';

  @override
  String get cloudFolderNote =>
      'Якщо ця тека синхронізується з хмарним сервісом, він бачитиме розмір і час кожної копії. Прочитати їх він не зможе.';

  @override
  String lastBackup(String when) {
    return 'Остання копія: $when';
  }

  @override
  String get backupFailed => 'Остання автоматична копія не вдалася.';

  @override
  String get restoreTitle => 'Відновити з копії';

  @override
  String get restoreHint =>
      'Відскануйте QR-код зі старого телефону, відкрийте файл копії або вставте текст копії. Потім введіть фразу відновлення.';

  @override
  String get openBackupFile => 'Відкрити файл копії';

  @override
  String get pasteBackup => 'Вставити текст копії';

  @override
  String get backupReceived => 'Копію отримано. Введіть фразу відновлення.';

  @override
  String get phraseInput => 'Фраза відновлення';

  @override
  String get restoreAction => 'Відновити';

  @override
  String get restoreReplaceTitle => 'Замінити цю особу?';

  @override
  String get restoreReplaceBody =>
      'На цьому телефоні вже є особа MEM з контактами. Відновлення її замінить. Поточний стан збережеться зашифрованим у сховищі застосунку; його не буде видалено.';

  @override
  String get restoreDone =>
      'Відновлено. Якщо старий телефон ще у вас, скиньте на ньому MEM: доти обидва можуть читати нові повідомлення.';

  @override
  String restoreCorrected(String phrase) {
    return 'У фразі виправлено одну літеру: $phrase';
  }

  @override
  String get restoreWrongPhrase => 'Ця фраза не відкриває цю копію.';

  @override
  String get restoreNotBackup => 'Це не резервна копія MEM.';

  @override
  String get working => 'Зачекайте…';

  @override
  String get qrPaceEasy => 'Легко';

  @override
  String get qrPaceNormal => 'Звичайно';

  @override
  String get qrPaceFast => 'Швидко';

  @override
  String get qrPaceHint => 'Якщо інший телефон не зчитує код, оберіть «Легко».';

  @override
  String get settings => 'Налаштування';

  @override
  String get language => 'Мова';

  @override
  String get followSystem => 'Як у системі';

  @override
  String get textSize => 'Розмір тексту як у телефоні';

  @override
  String percent(int value) {
    return '$value %';
  }

  @override
  String get sectionMessages => 'Повідомлення';

  @override
  String get showMessageTimes => 'Показувати час повідомлень';

  @override
  String get badgeDetail => 'Позначки каналів';

  @override
  String get badgeCompact => 'Стисло';

  @override
  String get badgeDetailed => 'Докладно';

  @override
  String get badgeDetailHelp =>
      'Докладні повні позначки показують ще й коли кожен канал переніс повідомлення та чи його доставлено.';

  @override
  String get messageBadges => 'Позначки повідомлень';

  @override
  String get messageBadgesHelp =>
      'Позначки каналів стоять унизу ліворуч у кожному повідомленні, у рядку з його часом. Згодом вони можуть зменшуватися до символу; торкніться області позначок, щоб побачити їх повністю. Інші застосунки (P4), відкритий текст (P5) і канали нижче за поріг приватності контакту завжди лишають свій символ.';

  @override
  String get badgeModeMinimal => 'Символи лише для P4, P5 і нижче порогу';

  @override
  String get badgeModeSymbols => 'Лише символи';

  @override
  String badgeModeCollapse(String delay) {
    return 'Повністю, потім символи через $delay';
  }

  @override
  String get badgeModeFull => 'Завжди повністю';

  @override
  String durationSeconds(int n) {
    return '$n с';
  }

  @override
  String durationMinutes(int n) {
    return '$n хв';
  }

  @override
  String durationHours(int n) {
    return '$n год';
  }

  @override
  String get badgeSymbols => 'Символи позначок';

  @override
  String get badgeSymbolsByGrade => 'За рівнем';

  @override
  String get badgeSymbolsOneShape => 'Одна форма';

  @override
  String get badgeSymbolsHelp =>
      '«За рівнем» дає кожному рівню приватності власну форму. «Одна форма» використовує обрану вами форму для P0–P3; P4 і P5 завжди мають свої.';

  @override
  String get sectionQr => 'QR-коди';

  @override
  String get qrPaceTitle => 'Швидкість анімованого QR';

  @override
  String get sectionPrivacy => 'Приватність';

  @override
  String get defaultFloor => 'Поріг приватності для нових контактів';

  @override
  String get defaultFloorHelp =>
      'Канали нижче порогу потребують вашого дозволу, перш ніж повідомлення піде. Наявні контакти зберігають свій поріг; змінити його можна на сторінці контакту.';

  @override
  String get blockScreenshots => 'Блокувати знімки екрана';

  @override
  String get blockScreenshotsHelp =>
      'Без знімків і запису екрана, а вміст MEM прихований у списку недавніх застосунків. Фраза відновлення захищена завжди.';

  @override
  String get showNetworkStatus => 'Показувати стан мережі на головному екрані';

  @override
  String get statusDelivered => 'Доставлено';

  @override
  String get statusReceived => 'Отримано';

  @override
  String get reason_invalid_floor =>
      'P5 не може бути порогом приватності: відкритий текст завжди потребує вашого дозволу для кожного повідомлення.';

  @override
  String get phraseNotConfirmed =>
      'Нову фразу не підтверджено, тож нічого не змінилося.';

  @override
  String get scanFocusHint =>
      'Розмито? Торкніться зображення, щоб сфокусувати.';

  @override
  String get helpTooltip => 'Як це працює';

  @override
  String get scanBrightness => 'Яскравість зображення камери';

  @override
  String get codeBrightness => 'Яскравість екрана для коду';

  @override
  String get stayReachable => 'Бути на зв\'язку у фоні';

  @override
  String get stayReachableHelp =>
      'Доставка через Wi-Fi і Bluetooth працює й з вимкненим екраном. Показує постійне сповіщення, тож кожен, хто бачить ваш телефон, знатиме, що MEM запущено, і витрачає більше заряду.';

  @override
  String get reachableNotification => 'На зв\'язку через Wi-Fi';

  @override
  String get batteryRestricted =>
      'Android усе ще може призупиняти MEM для економії заряду. Торкніться, щоб дозволити роботу без обмежень.';

  @override
  String get batteryUnrestricted => 'MEM може працювати без обмежень.';

  @override
  String get sectionCourier => 'Кур\'єр';

  @override
  String get courierMode => 'Переносити повідомлення інших користувачів MEM';

  @override
  String get courierModeHelp =>
      'Телефон зберігає запечатані повідомлення людей поруч і передає їх, коли зустрічає їхні телефони. Прочитати їх він не може: бачить лише зашифровані адреси, розміри й час. Займає місце та трохи заряду.';

  @override
  String courierLimit(int mb) {
    return 'До $mb МБ';
  }

  @override
  String courierUsage(int count, String used, String limit) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Переносить $count повідомлення',
      many: 'Переносить $count повідомлень',
      few: 'Переносить $count повідомлення',
      one: 'Переносить $count повідомлення',
    );
    return '$_temp0 · $used з $limit МБ';
  }

  @override
  String get channelCourier => 'кур\'єри';

  @override
  String get courierQueued =>
      'Повідомлення піде з наступним пристроєм MEM, який ви зустрінете.';

  @override
  String get courierHint =>
      'Передається через пристрої MEM поруч, які не можуть його прочитати.';

  @override
  String get reason_waiting_for_key =>
      'Очікуємо новий ключ шифрування вашого контакту. Будь-яке повідомлення від нього принесе ключ, і це повідомлення піде саме.';

  @override
  String get requestTab => 'Запит';

  @override
  String get requestHint =>
      'Для того, з ким не можете зустрітися: вставте його код MEM. Він отримає запит на контакт із вашим кодом і приміткою та вирішить, чи прийняти. Запит іде через пристрої MEM поруч, тож це може зайняти час.';

  @override
  String get requestCodeLabel => 'Його код (memcard1:…)';

  @override
  String get requestNoteLabel => 'Примітка (необов\'язково)';

  @override
  String get requestNoteHelp =>
      'Показується із запитом: хто ви і звідки він вас знає.';

  @override
  String get sendRequest => 'Надіслати запит';

  @override
  String get requestPreparing => 'Готуємо запит. Це займе кілька секунд.';

  @override
  String requestPendingBanner(String name) {
    return 'Очікуємо, поки $name прийме ваш запит на контакт. Писати можна буде після цього.';
  }

  @override
  String get contactRequestLabel => 'Запит на контакт';

  @override
  String get requestNoNote => '(без примітки)';

  @override
  String get requestsTitle => 'Запити на контакт';

  @override
  String requestsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count запитів на контакт',
      few: '$count запити на контакт',
      one: '1 запит на контакт',
    );
    return '$_temp0';
  }

  @override
  String get requestsEmpty => 'Немає запитів на контакт.';

  @override
  String get requestsHelp =>
      'Ці люди мають ваш код MEM і хочуть стати контактами. Імена не перевірені: порівняйте символи безпеки при зустрічі.';

  @override
  String nameNotVerified(String name) {
    return '$name (ім\'я не перевірене)';
  }

  @override
  String get acceptRequest => 'Прийняти';

  @override
  String get declineRequest => 'Відхилити';

  @override
  String get blockRequest => 'Заблокувати';

  @override
  String get blockConfirm =>
      'Заблокувати цього відправника? Його запити на контакт відтепер ігноруватимуться.';

  @override
  String get requestAccepted => 'Контакт додано';

  @override
  String get requestReceived => 'Отримано запит на контакт';

  @override
  String get requestsNearby =>
      'Приймати запити від людей, яких ви не зустрічали';

  @override
  String get requestsNearbyHelp =>
      'Запити надходять через пристрої MEM поруч і, якщо ви ним користуєтеся, через хаб. Будь-хто з вашим кодом MEM може помітити, що ваш телефон поруч, а хаб може пов\'язати вашу інтернет-адресу з вашим кодом. Вставлені чи відскановані запити надходять у будь-якому разі.';

  @override
  String get requestedSubtitle => 'Запит надіслано · очікуємо відповіді';

  @override
  String get reason_awaiting_approval => 'Контакт ще не прийняв ваш запит.';

  @override
  String get reason_bad_proof_of_work => 'Цей запит на контакт недійсний.';

  @override
  String get reason_unknown_request => 'На цей запит уже відповіли.';

  @override
  String get reason_already_contact =>
      'Ви вже контакти, або запит уже в дорозі.';

  @override
  String get reason_invalid_note => 'Примітка задовга (не більше 280 байтів).';

  @override
  String get reason_not_a_card => 'Це не код MEM.';

  @override
  String get channelBle => 'Bluetooth';

  @override
  String get bleToggle => 'Доставляти через Bluetooth';

  @override
  String get bleToggleHelp =>
      'Телефони поруч обмінюються повідомленнями без жодної мережі. Поки ввімкнено, будь-який Bluetooth-сканер поруч може побачити, що тут є пристрій з MEM, але не чий.';

  @override
  String get bleLocationExplain =>
      'У цій версії Android шукати Bluetooth-пристрої можна лише з дозволом на місцезнаходження та ввімкненим «Місцезнаходженням». MEM ніколи не читає ваше місцезнаходження.';

  @override
  String get bleStatusRunning => 'Шукає пристрої MEM поруч';

  @override
  String get bleStatusOff => 'Bluetooth вимкнено';

  @override
  String get bleStatusPermission => 'Немає дозволу на Bluetooth';

  @override
  String get bleStatusLocation =>
      'Місцезнаходження вимкнено, тож Android приховує пристрої поруч';

  @override
  String get bleStatusUnsupported => 'Цей телефон не має Bluetooth LE';

  @override
  String get bleTurnOn => 'Увімкнути';

  @override
  String get bleOpenSettings => 'Відкрити налаштування';

  @override
  String get bleBackgroundNote =>
      'У фоні — лише поки ввімкнено «Бути на зв\'язку у фоні».';

  @override
  String get reachableNotificationBle => 'На зв\'язку через Wi-Fi і Bluetooth';

  @override
  String get newGroup => 'Нова група';

  @override
  String get groupName => 'Назва групи';

  @override
  String get groupMembersPick => 'Кого запросити (до 15 контактів)';

  @override
  String get groupCreate => 'Створити';

  @override
  String get groupCreateHelp =>
      'Ви будете адміністратором групи: лише ви додаєте й видаляєте учасників. Усі запрошені бачать імена й коди MEM учасників, які приєдналися.';

  @override
  String groupMembersCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count учасника',
      many: '$count учасників',
      few: '$count учасники',
      one: '1 учасник',
    );
    return '$_temp0';
  }

  @override
  String get groupJoining => 'Приєднання… чекаємо на адміністратора';

  @override
  String get groupInvitedBanner => 'Вас запрошено до цієї групи.';

  @override
  String get groupLeftBanner => 'Ви вийшли з цієї групи.';

  @override
  String get groupRemovedBanner => 'Адміністратор видалив вас із цієї групи.';

  @override
  String get groupJoiningBanner =>
      'Чекаємо, поки адміністратор додасть вас. Писати можна буде після цього.';

  @override
  String get groupJoin => 'Приєднатися';

  @override
  String get groupDecline => 'Відхилити';

  @override
  String get groupInvitationsTitle => 'Запрошення до груп';

  @override
  String groupInvitedBy(String name) {
    return 'Запрошує $name';
  }

  @override
  String get groupInvitationHelp =>
      'Якщо приєднаєтеся, кожен учасник побачить ваше ім\'я та код MEM, а ви — їхні.';

  @override
  String get groupInfo => 'Про групу';

  @override
  String get groupAdmin => 'адміністратор';

  @override
  String get groupInvitedRole => 'запрошено';

  @override
  String get groupYou => 'ви';

  @override
  String get groupNotAContact => 'не ваш контакт';

  @override
  String get groupAddAsContact => 'Додати до контактів';

  @override
  String groupAddAsContactHelp(String name) {
    return '$name стане вашим контактом. Писати одне одному напряму зможете, коли вас теж додадуть.';
  }

  @override
  String get groupInvite => 'Запросити контакт';

  @override
  String get groupRemove => 'Видалити з групи';

  @override
  String get groupLeave => 'Вийти з групи';

  @override
  String get groupEnd => 'Завершити групу для всіх';

  @override
  String get groupLeaveConfirm =>
      'Вийти з цієї групи? Ви більше не отримуватимете її повідомлень.';

  @override
  String get groupEndConfirm =>
      'Завершити цю групу? Усіх учасників буде видалено.';

  @override
  String deliveredToCount(int done, int total) {
    return 'доставлено $done з $total';
  }

  @override
  String get groupSentAutomatically =>
      'Надсилається кожному учаснику автоматично, щойно з\'явиться шлях до нього';

  @override
  String get groupsSection => 'Групи';

  @override
  String get contactsSection => 'Контакти';

  @override
  String get reason_unknown_group => 'Ця група недоступна.';

  @override
  String get reason_not_group_admin => 'Це може лише адміністратор групи.';

  @override
  String get reason_group_full =>
      'У групі щонайбільше 16 учасників (і можна мати щонайбільше 64 групи).';

  @override
  String get reason_group_empty => 'У групі ще немає інших учасників.';

  @override
  String get reason_not_a_contact =>
      'Ця людина — лише учасник спільної групи, а не ваш контакт.';

  @override
  String get channelHub => 'Хаб';

  @override
  String get hubToggle => 'Доставляти через хаб MEM';

  @override
  String get hubToggleHelp =>
      'Повідомлення чекають у поштовій скриньці в інтернеті, доки контакт їх забере, тож навіть далекі телефони обмінюються повідомленнями. Оператор хабу бачить вашу IP-адресу, коли ви під\'єднуєтеся, розміри повідомлень і короткі частини адрес ваших скриньок, але ніколи — вміст. Будь-хто, хто стежить за вашою мережею, може побачити, що ви користуєтеся MEM.';

  @override
  String get hubUrl => 'Адреса хабу';

  @override
  String get hubUrlHint => 'wss://hub.example.org/v1';

  @override
  String get hubUrlSave => 'Зберегти';

  @override
  String get hubNeedsUrl =>
      'Спершу введіть адресу хабу. Типового хабу поки немає.';

  @override
  String get hubOnline => 'Під\'єднано до хабу';

  @override
  String get hubOffline => 'Не під\'єднано; повторюємо спробу';

  @override
  String get devices => 'Пристрої';

  @override
  String get devicesHelp =>
      'Ваш MEM може бути на чотирьох пристроях. Головний пристрій зберігає вашу особу: лише він додає контакти й пристрої. Ті, хто переносить вашу пошту, не розрізнять пристрої, але можуть помітити, що одне повідомлення йде кілька разів одночасно.';

  @override
  String get thisDevice => 'Цей пристрій';

  @override
  String get mainDevice => 'Головний пристрій';

  @override
  String get otherDevice => 'Інший пристрій';

  @override
  String get deviceWaiting =>
      'Ще не приєднався. Якщо додавання не завершилося, видаліть його й додайте пристрій знову.';

  @override
  String get addDevice => 'Додати пристрій';

  @override
  String get addDeviceHelp =>
      'На новому пристрої виберіть на початковому екрані «Використати як інший пристрій». Потім скануйте код, який він покаже.';

  @override
  String get addDeviceConfirmTitle => 'Додати цей пристрій?';

  @override
  String get addDeviceConfirmBody =>
      'Він отримає всі ваші контакти й ваші повідомлення. Додавайте лише власний пристрій, який зараз поруч із вами.';

  @override
  String get addDeviceShowSetup => 'Покажіть це новому пристрою';

  @override
  String get addDeviceSetupHint =>
      'Тримайте камеру нового пристрою навпроти цього коду, доки він не повідомить, що готовий.';

  @override
  String get removeDevice => 'Вилучити';

  @override
  String get removeDeviceTitle => 'Вилучити цей пристрій?';

  @override
  String get removeDeviceBody =>
      'Він перестане отримувати ваші повідомлення й видалить свою копію вашого MEM, щойно буде в мережі.';

  @override
  String get removeWaitingDeviceBody =>
      'Він так і не приєднався, тож не має ваших повідомлень. Якщо він усе ж зчитає код налаштування, то дізнається, що його видалено.';

  @override
  String get joinAsDevice => 'Використати як інший пристрій';

  @override
  String get joinTitle => 'Інший пристрій мого MEM';

  @override
  String get joinHelp =>
      'На головному пристрої відкрийте ⋮ → Пристрої → Додати пристрій і скануйте цей код. Потім наведіть цю камеру на код, який покаже головний пристрій.';

  @override
  String get joinDone => 'Цей пристрій тепер частина вашого MEM.';

  @override
  String get deviceRetiredTitle => 'Цей пристрій вилучено';

  @override
  String get deviceRetiredBody =>
      'Головний пристрій вилучив його з вашого MEM. Його копію зараз буде видалено.';

  @override
  String get addContactOnMain =>
      'Нові контакти додаються на головному пристрої; тут вони з\'являться самі.';

  @override
  String get reason_main_device_only =>
      'Це може зробити лише ваш головний пристрій.';

  @override
  String get reason_too_many_devices => 'У вашого MEM уже чотири пристрої.';

  @override
  String get reason_device_known => 'Цей пристрій уже ваш.';

  @override
  String get reason_unknown_device => 'Це не ваш пристрій.';

  @override
  String get reason_not_empty =>
      'На цьому пристрої вже є контакти. Іншим пристроєм може стати лише нове встановлення.';

  @override
  String get reason_not_for_this_device => 'Цей код для іншого пристрою.';

  @override
  String get joinWrongSetup =>
      'Цей код зроблено для попереднього запиту, тож тут його не відкрити. На головному пристрої видаліть пристрій із позначкою «Ще не приєднався», а потім додайте цей пристрій знову, скануючи код вище.';

  @override
  String get channelHubPartners => 'Хаби кількох операторів';

  @override
  String hubPartners(String names) {
    return 'Цей хаб також обмінюється повідомленнями з хабами: $names. Повідомлення контакту, який користується одним із них, несуть обидва оператори; обидва бачать те, що бачить хаб. Такі повідомлення позначено P3.';
  }

  @override
  String get ownOperatorOnly => 'Лише хаби мого оператора';

  @override
  String get ownOperatorOnlyHelp =>
      'Ваші повідомлення ніколи не потрапляють на хаби інших операторів. Контакти, які користуються хабом іншого оператора, тоді не отримають їх через хаб.';

  @override
  String get reason_invalid_hub_url =>
      'Використовуйте адресу wss:// (ws:// — лише для хабу на цьому пристрої).';

  @override
  String get pushNotification => 'Нові повідомлення';

  @override
  String get pushToggle => 'Будити для нових повідомлень';

  @override
  String get pushToggleHelp =>
      'Застосунок UnifiedPush на цьому телефоні (наприклад ntfy) будить MEM, коли в хабі щось є для вас, тож повідомлення приходять і з закритим MEM. Сервіс push дізнається лише, коли MEM будять. Потрібен хаб.';

  @override
  String get pushNoDistributor =>
      'Спершу встановіть застосунок UnifiedPush (наприклад ntfy).';

  @override
  String get pushPick => 'Який застосунок push?';

  @override
  String get channelPlain => 'Відкритий текст';

  @override
  String get plainTitle => 'Надіслати відкритим текстом?';

  @override
  String get plainWarning =>
      'Цей текст залишить MEM без шифрування. Застосунок, який ви виберете, його оператор і кожен, хто може читати той чат, побачать його. Скасувати це неможливо.';

  @override
  String get plainSend => 'Надіслати відкритим текстом';

  @override
  String get reason_not_plain_text =>
      'Відкритим текстом можна надіслати лише особисте текстове повідомлення.';

  @override
  String get channelEmail => 'Пошта';

  @override
  String get emailToggle => 'Доставляти електронною поштою';

  @override
  String get emailToggleHelp =>
      'MEM надсилає запечатані повідомлення з вашого поштового акаунта й забирає з нього листи MEM. Ваш поштовий провайдер бачить обидві адреси, коли, скільки і що це лист MEM, але ніколи — вміст. Це нижче за ваш типовий поріг приватності, тож кожне надсилання питає, якщо ви не дозволите пошту для контакту.';

  @override
  String get emailAddress => 'Ваша адреса пошти';

  @override
  String get emailImapHost => 'Сервер IMAP';

  @override
  String get emailSmtpHost => 'Сервер SMTP';

  @override
  String get emailPort => 'Порт';

  @override
  String get emailUsername => 'Логін';

  @override
  String get emailPassword => 'Пароль застосунку';

  @override
  String get emailPasswordKept =>
      'Пароль застосунку (збережено; залиште порожнім, щоб не змінювати)';

  @override
  String get emailSaveAccount => 'Зберегти акаунт';

  @override
  String get emailRemoveAccount => 'Видалити акаунт';

  @override
  String get emailOnline => 'Під\'єднано до поштової скриньки';

  @override
  String get emailOffline => 'Не під\'єднано; повторюємо спробу';

  @override
  String get emailSaved => 'Поштовий акаунт збережено';

  @override
  String get contactEmail => 'Адреса пошти';

  @override
  String get contactEmailHelp =>
      'Лише на цьому телефоні; MEM надсилає сюди запечатані повідомлення, коли ви дозволите пошту для цього контакту.';

  @override
  String get contactEmailSave => 'Зберегти';

  @override
  String get contactAliasEdit => 'Перейменувати';

  @override
  String get contactAliasTitle => 'Ім\'я для цього контакту';

  @override
  String get contactAliasHelp =>
      'Це ім\'я бачите лише ви; контакт про нього не дізнається. Залиште порожнім, щоб показувати ім\'я, яке обрав контакт.';

  @override
  String contactTheirName(String name) {
    return 'Власне ім\'я: $name';
  }

  @override
  String get contactTheirNameNone => 'Контакт не обрав імені';

  @override
  String get reason_invalid_email_account =>
      'Перевірте поштовий акаунт: потрібні адреса, сервери, порти, логін і пароль застосунку.';

  @override
  String get reason_invalid_email_address => 'Це не адреса електронної пошти.';

  @override
  String get alreadyContact => 'Уже є у ваших контактах';

  @override
  String get compareSymbols => 'Звірити символи';

  @override
  String get channelWifiDirect => 'Wi-Fi Direct';

  @override
  String get wifiDirectToggle => 'Доставляти через Wi-Fi Direct';

  @override
  String get wifiDirectToggleHelp =>
      'Телефони поруч обмінюються повідомленнями через Wi-Fi без роутера й інтернету. Поки це ввімкнено, телефони поруч бачать, що ваш користується MEM, а Android показує їм назву вашого телефона у Wi-Fi Direct. На Android 9 і старіших інший телефон отримує запит прийняти кожне з\'єднання. Android 12 і старіші шукають телефони лише з увімкненою геолокацією; MEM ніколи не читає ваше місцезнаходження.';

  @override
  String get wifiDirectUnsupported => 'Цей телефон не підтримує Wi-Fi Direct.';

  @override
  String get inboundCardTitle => 'Додати цей контакт?';

  @override
  String get inboundCardBody =>
      'Інший застосунок передав MEM код контакту. Додавайте його, лише якщо ви цього хотіли: ця людина зможе писати вам. Якщо можете, звірте з нею відбиток.';

  @override
  String get wifiDirectPermission => 'Немає дозволу шукати телефони поруч';

  @override
  String get wifiDirectLocation =>
      'Геолокацію вимкнено: ця версія Android потребує її, щоб шукати телефони через Wi-Fi Direct';

  @override
  String get removeContact => 'Видалити контакт';

  @override
  String get removeContactTitle => 'Видалити цей контакт?';

  @override
  String get removeContactBody =>
      'Розмову та повідомлення, що ще чекають надсилання, буде видалено, і повідомлення від цієї людини більше не надходитимуть. Спільні групи працюватимуть далі. Щоб знову спілкуватися, треба буде знову з\'єднатися.';

  @override
  String get ruleHowLong => 'На який час?';

  @override
  String get ruleDays1 => '1 день';

  @override
  String get ruleDays7 => '7 днів';

  @override
  String get ruleDays30 => '30 днів';

  @override
  String get ruleUntilRevoked => 'Доки не скасую';

  @override
  String ruleUntil(String date) {
    return 'до $date';
  }

  @override
  String get soundPairButton => 'З\'єднати звуком';

  @override
  String get soundPairTitle => 'З\'єднання звуком';

  @override
  String get soundPairHint =>
      'Тримайте телефони поруч. На одному натисніть «Слухати», на іншому «Відтворити мій код». Далі телефони обмінюються по черзі самі; наприкінці обидва покажуть однакові символи. Порівняйте їх і натисніть «З\'єднати» на обох.';

  @override
  String soundPairProgress(int received, int needed) {
    return 'Почуто $received з $needed частин';
  }

  @override
  String get soundListen => 'Слухати';

  @override
  String get soundPlay => 'Відтворити мій код';

  @override
  String get soundStop => 'Зупинити';

  @override
  String get soundIdle =>
      'Оберіть, що робить цей телефон: слухає чи відтворює свій код.';

  @override
  String get soundListening => 'Слухаю інший телефон…';

  @override
  String soundPlaying(int round, int rounds) {
    return 'Відтворюю свій код, раунд $round з $rounds…';
  }

  @override
  String get soundListenTimeout =>
      'Дві хвилини нічого не почуто. Наблизьте телефони або зробіть тихіше, і слухайте знову.';

  @override
  String get soundPlayDone =>
      'Мій код відтворено тричі. Якщо інший телефон його не отримав, відтворіть ще раз.';

  @override
  String get soundTurnWait =>
      'Їхній код отримано. Чекаю тиші, щоб відтворити свій…';

  @override
  String get soundNoAnswer =>
      'Інший телефон не відповів. Наблизьте телефони або зробіть тихіше, і почніть знову.';

  @override
  String get soundDecline => 'Відхилити';

  @override
  String get soundConnect => 'З\'єднати';

  @override
  String get soundPaired => 'З\'єднано. Тепер ви можете писати одне одному.';

  @override
  String get soundDeclined =>
      'Відхилено. Контакт, доданий цим обміном, вилучено.';

  @override
  String get soundDeclinedKept =>
      'Відхилено. Контакт, який уже був, залишився без змін.';

  @override
  String get soundPairGot => 'Їхній код отримано.';

  @override
  String get soundPairNoMicrophone =>
      'Для з\'єднання звуком потрібен мікрофон. Дозвольте його в налаштуваннях телефона або з\'єднайтеся QR-кодом.';

  @override
  String get soundPairFailed =>
      'Звук не вдалося запустити на цьому телефоні. З\'єднайтеся QR-кодом.';

  @override
  String get soundToneTitle => 'Звук';

  @override
  String get soundToneAudible => 'Чутний';

  @override
  String get soundToneInaudible => 'Майже нечутний';

  @override
  String get soundPairExposure =>
      'Будь-хто поруч, або будь-який запис, може вловити ваш код, так само як будь-хто, хто бачить ваш QR-код. Повідомлення звуком ніколи не передаються. Обидва телефони мають використовувати однаковий звук.';

  @override
  String get channelAudio => 'Звук';

  @override
  String get nearbyTitle => 'Поруч';

  @override
  String get connectBySound => 'З\'єднатися звуком';

  @override
  String get connectByCode => 'З\'єднатися кодом';

  @override
  String get nearbyHelp =>
      'Телефони в одній Wi-Fi зазвичай знаходять один одного самі. Деякі мережі (гостьові, готельні, офісні) цьому заважають: тоді з\'єднайтеся звуком або кодом. Один телефон підказує іншому, де його знайти в цій Wi-Fi; самі повідомлення йдуть через Wi-Fi.';

  @override
  String get connectSoundHint =>
      'Тримайте телефони поруч. На одному натисніть «Слухати», на іншому «Відтворити мій код». Телефон, що слухає, з\'єднається з тим, що відтворює, через цю Wi-Fi; один код триває близько 6 секунд.';

  @override
  String get connectSoundExposure =>
      'Люди поруч або запис можуть почути, що тут є телефон з MEM, його адресу в цій мережі й тимчасовий ключ, який змінюється після перезапуску MEM. Нічого, що вас ідентифікує, і жодних повідомлень.';

  @override
  String get connectGot =>
      'З\'єднуюся з іншим телефоном. Повідомлення для ваших контактів підуть самі.';

  @override
  String get connectNoMicrophone =>
      'Для з\'єднання звуком потрібен мікрофон. Дозвольте його в налаштуваннях телефона або з\'єднайтеся кодом.';

  @override
  String get connectFailed =>
      'Звук не вдалося запустити на цьому телефоні. З\'єднайтеся кодом.';

  @override
  String get connectCodeHint =>
      'Дайте іншому телефону відсканувати цей код або відскануйте його код нижче.';

  @override
  String get reason_no_local_network =>
      'Спершу ввімкніть Wi-Fi і канал локальної мережі (Канали).';

  @override
  String get attestTitle => 'Підтвердити особисто';

  @override
  String get attestScanRequest =>
      'Скануйте код на сторінці сервісу або вставте його. Ви підтверджуєте разом з іншою людиною, кожен на своєму телефоні.';

  @override
  String get attestMainOnly =>
      'Підтверджувати особисто можна лише на основному пристрої.';

  @override
  String get attestAskedBy => 'Хто просить';

  @override
  String get attestWhat => 'Що';

  @override
  String get attestRole => 'Ваша роль';

  @override
  String attestCheckCode(String code) {
    return 'Перевірте, що сторінка показує той самий код: $code';
  }

  @override
  String get attestExchangeHint =>
      'Покажіть свій код іншому телефону, потім скануйте його код.';

  @override
  String get attestCompare =>
      'Порівняйте ці символи з тими, що на іншому телефоні. Підтверджуйте, лише якщо вони збігаються й інша людина перед вами.';

  @override
  String get attestSending => 'Надсилання…';

  @override
  String attestSent(String origin) {
    return 'Надіслано. $origin отримав ваше підтвердження.';
  }

  @override
  String get attestSendFailed =>
      'Не вдалося надіслати. Скопіюйте цей код і вставте його на сторінці сервісу.';

  @override
  String get attestTryAgain => 'Спробувати ще раз';

  @override
  String get attestCancelled => 'Скасовано. Нічого не підписано.';

  @override
  String get attestErrWrongObject => 'Тут потрібен інший код.';

  @override
  String get attestErrInvalid => 'Цей код пошкоджений або недозволений.';

  @override
  String get attestErrOtherSubject => 'Інший телефон підтверджує щось інше.';

  @override
  String get attestErrSameRole =>
      'Обидва телефони мають ту саму роль. Кожен має сканувати свій код на сторінці сервісу.';

  @override
  String get attestErrOwnCode => 'Це ваш власний код. Скануйте інший телефон.';

  @override
  String get attestLog => 'Підтверджено особисто';

  @override
  String attestLogEntry(String origin, String how) {
    return '$origin · $how';
  }

  @override
  String get attestLogSent => 'надіслано';

  @override
  String get attestLogCopied => 'скопійовано';
}
