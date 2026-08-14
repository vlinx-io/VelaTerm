//! Russian dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.
//! Russian nouns use three number-dependent forms (1, 2–4, and 5+), selected by the plural() helper.

import type en from "./en";

/** Select one of the Russian plural forms [1, 2–4, 5+] using ones and tens digit rules. */
function plural(n: number, one: string, few: string, many: string): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return one;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return few;
  return many;
}

const ru: typeof en = {
  // ── Common ──
  "common.cancel": "Отмена", // Cancel
  "common.confirm": "ОК", // OK
  "common.delete": "Удалить", // Delete
  "common.save": "Сохранить", // Save
  "common.create": "Создать", // Create
  "common.close": "Закрыть", // Close
  "common.copy": "Копировать", // Copy
  "common.cut": "Вырезать", // Cut
  "common.paste": "Вставить", // Paste
  "common.selectAll": "Выделить все", // Select All
  "common.copied": "Скопировано", // Copied
  "common.retry": "Повторить", // Retry
  "common.refresh": "Обновить", // Refresh
  "common.loading": "Загрузка…", // Loading…
  "common.prev": "Назад", // Previous
  "common.next": "Далее", // Next
  "common.on": "Вкл", // On
  "common.off": "Выкл", // Off
  "common.gotIt": "Понятно", // Got it
  "common.rename": "Переименовать", // Rename
  "common.edit": "Изменить", // Edit
  "common.open": "Открыть", // Open
  "common.session": "Сессия", // Session

  // ── Session types and status ──
  "kind.terminal": "Терминал", // Terminal
  "kind.browser": "Браузер", // Browser
  "status.idle": "Простаивает", // Idle
  "status.running": "Выполняется", // Running
  "status.exited": "Завершено", // Exited
  "status.error": "Ошибка", // Error
  "status.working": "В работе", // Working
  "status.asking": "Нужно подтверждение", // Needs confirmation
  "status.waiting": "Просмотрено", // Viewed
  "status.unavailable": "Статус недоступен",
  "indicator.unread": "Непрочитано · к просмотру", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `Сборка от ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `Несовпадение версий: фронтенд v${frontend} ≠ бэкенд v${backend} — пересоберите или разверните синхронно.`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `Горячая перезагрузка в ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `Как в системе (сейчас: ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Тёмная", // Dark
  "titlebar.themeLight": "Светлая", // Light
  "titlebar.browser": "Встроенный браузер (⌘⇧B)", // Built-in Browser (⌘⇧B)
  "titlebar.remoteAccess": "Удалённый доступ (браузер)", // Remote Access (Browser)
  "titlebar.connectRemote": "Подключиться к удалённому серверу", // Connect to Remote Server
  "titlebar.share": "Поделиться", // Share
  "share.title": "Поделиться VelaTerm", // Share VelaTerm
  "share.subtitle":
    "VelaTerm создаёт небольшая команда. Если вам нравится продукт, поделитесь им с другими. Для нас очень важно, чтобы больше людей узнали о VelaTerm и о нашей команде. Спасибо за поддержку! ❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "Копировать ссылку", // Copy link
  "share.copied": "Скопировано!", // Copied!
  "share.wechatMoments": "WeChat Moments",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "Xiaohongshu",
  "share.xiaohongshuAction":
    "Скопировать текст и ссылку и открыть Центр авторов Xiaohongshu",
  "share.wechatQrTitle": "Поделиться в WeChat Moments",
  "share.wechatQrHint":
    "Отсканируйте код в WeChat, откройте ссылку и выберите публикацию в Moments.",
  "share.backToPlatforms": "Вернуться к вариантам публикации",
  "titlebar.appearance": "Внешний вид", // Appearance
  "titlebar.showLeft": "Показать боковую панель", // Show sidebar
  "titlebar.hideLeft": "Скрыть боковую панель", // Hide sidebar
  "titlebar.showRight": "Показать панель информации", // Show info panel
  "titlebar.hideRight": "Скрыть панель информации", // Hide info panel

  // ── Settings ──
  "settings.title": "Настройки", // Settings
  "settings.catTerminal": "Терминал", // Terminal
  "settings.catBehavior": "Поведение", // Behavior
  "settings.catAgents": "Агенты", // Agents
  "settings.permDefault": "По умолчанию", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `Запускается с ${flag}. Пропускает все подтверждения разрешений — используйте с осторожностью.`, // YOLO flag hint
  "settings.permViaEnvHint":
    "Пропускает все подтверждения разрешений через инъекцию конфига (без CLI флага). Применяется при запуске сессии.",
  "settings.catGeneral": "Общие", // General
  "settings.cliLabel": "Команда оболочки",
  "settings.cliInstall": "Установить команду ‘vela’",
  "settings.cliUninstall": "Удалить команду ‘vela’",
  "settings.cliInstalledAt": (path: string) => `Установлена в ${path}`,
  "settings.cliConflict": (path: string) =>
    `В ${path} уже существует другая команда ‘vela’. VelaTerm не будет её перезаписывать.`,
  "settings.cliHint": "Добавляет `vela <путь-к-проекту>` в PATH, как команда `code` в VS Code.",
  "settings.agentArgsHint":
    "Аргументы запуска по умолчанию для новых сессий каждого типа агента. Аргументы, заданные для отдельной сессии при создании или редактировании, имеют приоритет. Оставьте пустым, чтобы не использовать.", // Agent default launch args hint
  "settings.agentPathLabel": "Путь к исполняемому файлу (необязательно)", // Executable path (optional)
  "settings.agentPathPlaceholder": "напр. ~/.local/bin/claude — пусто = искать в PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Если задан, сессии этого типа запускаются по этому полному пути вместо поиска команды в PATH. Полезно, когда агент установлен, но отсутствует в PATH оболочки. Заполняется автоматически после успешной установки в один клик, если место установки удалось определить.", // Agent executable path hint
  "settings.catOrchestration": "Оркестрация",
  "settings.orchProfilesTitle": "Профили воркеров",
  "settings.orchProfile": "Профиль",
  "settings.orchDescription": "Описание",
  "settings.orchDescriptionPlaceholder": "Опишите, когда следует использовать этот профиль.",
  "settings.orchNewProfile": "Имя нового профиля",
  "settings.orchAdd": "Добавить",
  "settings.orchAddNew": "Добавить новый",
  "settings.orchDelete": "Удалить",
  "settings.orchNoProfiles":
    "Профилей пока нет. Создайте один, чтобы использовать его при каждом spawn.",
  "settings.orchProfilesHint":
    "Профили сообщают ведущему агенту, какая конфигурация воркера подходит для каждой задачи. Опишите условия применения каждого профиля и выберите агента, модель, уровень усилий и worktree.",
  "settings.orchModel": "Модель",
  "settings.orchEffort": "Уровень усилий",
  "settings.orchWorktree": "Отдельный worktree",
  "settings.orchLimitsTitle": "Ограничения",
  "settings.orchMaxChildren": "Макс. дочерних",
  "settings.orchMaxParallel": "Макс. параллельно",
  "settings.orchMaxDepth": "Макс. глубина",
  "settings.orchConfirmAbove": "Подтверждать свыше",
  "settings.orchTimeout": "Тайм-аут по умолчанию (секунды)",
  "settings.orchConfirmAboveHint":
    "Если после spawn число активных дочерних сессий превысит это значение, карточка подтверждения появится даже при выключенном подтверждении spawn.",
  "settings.orchLimitsHint":
    "Бэкенд применяет эти ограничения при каждом spawn. Ведущий агент читает текущие значения во время работы командой `vagent config`.",
  "settings.orchCopyPatterns": "Шаблоны копирования в worktree",
  "settings.orchCopyPatternsHint":
    "По одному glob в строке. Неотслеживаемые или игнорируемые файлы, попавшие под шаблон, копируются из корня репозитория в каждый новый worktree. Результаты сборки, такие как node_modules, не копируются, поэтому воркеры всё равно собирают проект с нуля.",
  "settings.appearance": "Внешний вид", // Appearance
  "settings.accent": "Акцент", // Accent
  "settings.accentAuto": "Как тема", // Follow theme
  "settings.density": "Плотность", // Density
  "settings.densityCompact": "Плотно", // Compact
  "settings.densityRegular": "Обычно", // Regular
  "settings.densityComfy": "Просторно", // Comfy
  "settings.pane": "Панели", // Panes
  "settings.paneFlush": "Вплотную", // Flush
  "settings.paneCard": "Карточка", // Card
  "settings.divider": "Разделитель", // Divider
  "settings.dividerSubtle": "Тонкий", // Subtle
  "settings.dividerVisible": "Видимый", // Visible
  "settings.nav": "Боковая панель", // Sidebar
  "settings.navTree": "Дерево", // Tree
  "settings.navCompact": "Компактно", // Compact
  "settings.tabs": "Вкладки", // Tabs
  "settings.dynamicStatusFilter": "Динамическое добавление в фильтр статуса",
  "settings.tabSingle": "Одна", // Single
  "settings.tabMulti": "Несколько", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "Shell по умолчанию", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.cleanImages": "Автоочистка вставленных изображений",
  "settings.cleanImagesHint":
    "Изображения, вставленные или перетащенные в терминал, сначала сохраняются во временные файлы (путь передаётся агенту). Если включено, временные файлы этого сеанса удаляются при выходе, а остатки старше 24 ч очищаются при запуске. Изображения внутри документов не затрагиваются.",
  "settings.cleanImagesNow": "Очистить сейчас",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `Очищено временных изображений: ${n} (освобождено ${size}).`,
  "settings.cleanImagesEmpty": "Нет временных изображений для очистки.",
  "settings.imagePasteMode": "Вставка изображения",
  "settings.imagePasteUpload": "Вставить путь к файлу",
  "settings.imagePasteAgent": "Нативная вставка",
  "settings.imagePasteHint":
    "Выберите, что вставлять при вставке изображения (только локальный рабочий стол). Вставить путь к файлу: изображение временно сохраняется, а путь вставляется в Claude или Codex. Нативная вставка: Claude или Codex читает системный буфер обмена и показывает собственный маркер изображения.",
  "settings.imagePasteRemoteHint":
    "В удалённых сеансах всегда вставляется путь к файлу, чтобы агент мог прочитать изображение на своей машине. Нативная вставка доступна только локально.",
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.modelLabel": "Модель", // Model
  "spawn.effortLabel": "Уровень усилий", // Effort
  "spawn.optionDefault": "по умолчанию", // default
  "spawn.optionOther": "Другое...", // Other...
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Показать изменения…",
  "changes.title": "Изменения",
  "changes.loading": "Загрузка…",
  "changes.loadingDiff": "Загрузка diff…",
  "changes.noChanges": "Нет изменений",
  "changes.selectFile": "Выберите файл",
  "changes.binary": "Двоичный файл — построчный diff недоступен",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody": "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
  "tree.convertToNormalGroup": "Convert to normal group", // TODO translate
  "merge.title": "Merge branches", // TODO translate
  "merge.desc": "Pick a source and a target branch; the source merges into the target.", // TODO translate
  "merge.notRepo": "This session's directory is not a git repository.", // TODO translate
  "merge.loadingBranches": "Loading branches…", // TODO translate
  "merge.loadingDiff": "Loading diff…", // TODO translate
  "merge.sourceLabel": "Source branch", // TODO translate
  "merge.targetLabel": "Target branch", // TODO translate
  "merge.selectBranch": "Select a branch…", // TODO translate
  "merge.swap": "Swap direction", // TODO translate
  "merge.pickHint": "Pick both branches to preview the changes this merge brings in.", // TODO translate
  "merge.changes": (target: string) => `Changes brought into "${target}"`, // TODO translate
  "merge.noChanges": "No file changes.", // TODO translate
  "merge.sameBranch": "Source and target are the same branch.", // TODO translate
  "merge.branchGone": "A selected branch no longer exists. Pick again.", // TODO translate
  "merge.upToDate": "The target branch already contains the source branch. Nothing to merge.", // TODO translate
  "merge.targetNotCheckedOut": (target: string) =>
    `Target branch "${target}" isn't checked out in any worktree, so a local merge can't run. Check it out first.`, // TODO translate
  "merge.targetDirty":
    "The target branch's working tree has uncommitted changes; the merge may be blocked.", // TODO translate
  "merge.sourceDirtyNote":
    "The source branch's working tree has uncommitted changes; they will be committed first.", // TODO translate
  "merge.commitMsgLabel": "Commit message", // TODO translate
  "merge.commitMsgPlaceholder": "Describe this change (used as the commit message)", // TODO translate
  "merge.apply": "Merge", // TODO translate
  "merge.commitAndApply": "Commit & merge", // TODO translate
  "merge.working": "Merging…", // TODO translate
  "merge.doneMsg": (source: string, target: string) => `Merged "${source}" into "${target}".`, // TODO translate
  "merge.conflictMsg": (target: string) =>
    `Merge has conflicts. Resolve them in the terminal of "${target}"'s worktree, then commit:`, // TODO translate
  "merge.close": "Close", // TODO translate
  "gitea.title": "Gitea integration", // TODO translate
  "gitea.desc":
    "Configure a Gitea server to land worktrees by opening a pull request. The token is stored in your system keychain.", // TODO translate
  "gitea.baseUrl": "Base URL", // TODO translate
  "gitea.token": "Access token", // TODO translate
  "gitea.tokenSet": "Saved (leave blank to keep)", // TODO translate
  "gitea.tokenPlaceholder": "Personal access token", // TODO translate
  "gitea.test": "Test connection", // TODO translate
  "gitea.saved": "Saved.", // TODO translate
  "settings.renderer": "Отрисовщик терминала", // Terminal renderer
  "settings.redrawOnReveal": "Перерисовка при возврате к вкладке", // Redraw on tab switch
  "settings.catAdvanced": "Дополнительно", // Advanced
  "settings.outputScheduler": "Приоритет вывода активного терминала", // Foreground-priority output
  "settings.recordSessions": "Запись журналов сессий", // Record session logs
  "settings.recordSessionsHint":
    "По умолчанию выключено. Когда включено, вывод терминала сохраняется в файл журнала для воспроизведения из архива и поиска. Обычные сессии терминала никогда не записываются; сессии агента читают собственную расшифровку.", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "Звук уведомлений", // Notification sound
  "settings.language": "Язык", // Language
  "settings.langAuto": "Авто (система)", // Auto (system)
  "settings.skillLabel": "Vela Skills",
  "settings.skillInstall": "Install", // Install
  "settings.skillInstalled": "Reinstall", // Reinstall
  "settings.skillInvokeHint":
    "Claude: /vspawn <task> · Codex: $vspawn <task>. If Codex does not list it after installation, start a new Codex session.",
  // Notification permission guidance
  "settings.notify": "System notifications", // TODO translate
  "settings.notifyGranted": "Enabled", // TODO translate
  "settings.notifyAllow": "Allow notifications", // TODO translate
  "settings.notifyOffHint":
    "Allow VelaTerm to alert you when an agent needs your input or finishes a task.", // TODO translate
  "settings.notifyDeniedHint": "Notifications are blocked. To turn them on:", // TODO translate
  "settings.notifyStepsMac":
    "open System Settings ▸ Notifications ▸ VelaTerm and turn on Allow Notifications (Banners or Alerts recommended).", // TODO translate
  "settings.notifyStepsWin":
    "open Settings ▸ System ▸ Notifications, enable VelaTerm, and make sure Focus assist / Do not disturb isn't blocking it.", // TODO translate
  "settings.notifyStepsLinux":
    "open your desktop's Settings ▸ Notifications and allow VelaTerm.", // TODO translate
  "settings.notifyStepsBrowser":
    "click the site-permission icon in the address bar and set Notifications to Allow.", // TODO translate
  "settings.notifyUnsupported": "Notifications aren't available in this environment.", // TODO translate
  "settings.notifyOpenSettings": "Open System Settings", // TODO translate
  // Shortcut categories
  "settings.catShortcuts": "Горячие клавиши", // Shortcuts
  "settings.scOpenProject": "Открыть проект", // Open project
  "settings.scNewTab": "Новый терминал", // New terminal
  "settings.scNewBrowserTab": "Новая вкладка браузера", // New browser tab
  "settings.scClosePane": "Закрыть панель / вкладку", // Close pane / tab
  "settings.scSplitRight": "Разделить вправо", // Split right
  "settings.scSplitDown": "Разделить вниз", // Split down
  "settings.scSearch": "Поиск в терминале", // Find in terminal
  "settings.scGlobalSearch": "Поиск по всем сеансам", // Search all sessions
  "settings.scSaveDoc": "Сохранить документ", // Save document
  "settings.scRecording": "Нажмите клавиши…", // Press keys…
  "settings.scHint": "Нажмите на сочетание, затем нажмите новую комбинацию (нужен Cmd/Ctrl).", // hint
  "settings.scReset": "Сбросить по умолчанию", // Restore defaults
  "settings.scConflict": (label: string) => `Уже используется «${label}»`, // conflict

  // ── Remote access panel ──
  "remote.title": "Удалённый доступ (браузер)", // Remote Access (Browser)
  "remote.desc":
    "После включения устройства в той же локальной сети смогут открыть адрес ниже в браузере, ввести пароль и получить тот же интерфейс, что и на десктопе.", // Once enabled, devices on the same LAN…
  "remote.needPassword": "Сначала задайте пароль доступа", // Please set an access password first
  "remote.running": (port) => `Работает · порт ${port}`, // Running · port {port}
  "remote.urlsHint":
    "Откройте адрес из той же WiFi-сети / подсети, что и ваше устройство (при нескольких сетевых интерфейсах выберите нужный; адреса VPN/туннелей идут последними и обычно недоступны с других устройств):", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "Нажмите, чтобы скопировать адрес", // Click to copy address
  "remote.moreUrls": (n: number) => `Ещё ссылок: ${n}`, // N more urls
  "remote.lessUrls": "Свернуть", // Show less
  "remote.stop": "Остановить сервер", // Stop Server
  "remote.passwordPlaceholder": "Задайте пароль доступа", // Set access password
  "remote.starting": "Запуск…", // Starting…
  "remote.start": "Запустить сервер", // Start Server
  "remote.portLabel": "Порт", // Port
  "remote.portInvalid": "Порт должен быть от 1 до 65535", // Port must be between 1 and 65535
  "remote.fingerprintLabel": "Отпечаток сертификата (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "При первом подключении браузеры предупреждают, что сертификат не доверенный — это нормально для самоподписанного сертификата. Сравните этот отпечаток, чтобы убедиться, что это ваш компьютер.", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "Создать ссылку сопряжения", // Create pairing link
  "remote.pairingRegenerate": "Пересоздать ссылку (отключить всех)", // Regenerate link (disconnects all)
  "remote.pairingCreating": "Создание…", // Generating…
  "remote.pairingHint":
    "Откройте в браузере и введите пароль. Ссылка содержит учётные данные доступа — делитесь только со своими устройствами.", // Open in a browser, then enter the password…

  "remote.devicesLabel": "Сопряжённые устройства", // Paired devices
  "remote.lastSeen": "Последнее подключение", // Last seen
  "remote.revoke": "Отозвать", // Revoke
  "remote.deviceBlock": "Заблокировать", // Block
  "remote.deviceBlockConfirm": "Подтвердить блокировку", // Confirm block
  "remote.deviceBlockHint":
    "Заблокированные устройства отключаются и не могут переподключиться (нужна новая ссылка сопряжения). Другие устройства не затрагиваются.", // Block hint
  "remote.devicesEmpty": "Нет сопряжённых устройств", // No paired devices yet

  // ── Remote connection panel ──
  "connect.title": "Подключиться к удалённому серверу", // Connect to Remote Server
  "connect.pairingPlaceholder": "Вставьте ссылку сопряжения", // Paste pairing link
  "connect.confirmConnect": "Отпечаток верный, подключиться", // Fingerprint matches, connect
  "connect.desc":
    "Введите адрес и пароль удалённого VelaTerm, чтобы подключиться и управлять им в новом окне.", // Enter the address and password…
  "connect.addressPlaceholder": "IP-адрес, напр. 192.168.1.100", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "Порт", // Port
  "connect.connecting": "Подключение…", // Connecting…
  "connect.connect": "Подключиться", // Connect
  "connect.stagePreparing": "Подготовка сервера…",
  "connect.stageTransferring": "Передача сервера…",
  "connect.stageStarting": "Запуск сервера…",
  "connect.sshFingerprintLabel": (kt: string) => `Отпечаток ключа хоста SSH (${kt})`,
  "connect.sshHostNew":
    "Первое подключение к этому хосту — проверьте отпечаток, прежде чем продолжить.",
  "connect.sshHostChanged":
    "⚠ Ключ этого хоста изменился — возможно, переустановка сервера или атака «человек посередине». Продолжайте, только если уверены.",
  "connect.urlCertChanged":
    "⚠ Отпечаток сертификата этого сервера изменился с момента последнего подтверждения — возможно, переустановка сервера или атака «человек посередине». Продолжайте, только если уверены.",
  "connect.sshPasswordLabel": "Пароль SSH",
  "connect.sshPasswordPlaceholder": "Пароль учётной записи",
  "connect.savedHosts": "Недавние хосты",
  "connect.savedHostsAll": "Все недавние хосты",
  "connect.showAllHosts": (n: number) => `Показать все (${n})`,
  "connect.forgetHost": "Забыть этот хост",
  "connect.savedHasPassword": "Пароль сохранён",
  "connect.rememberPassword": "Запомнить пароль",
  "connect.urlPasswordPlaceholder": "Пароль для входа",
  "connect.shareDesktopDb": "Использовать базу данных настольного приложения на удалённой машине",
  "connect.shareDesktopDbHint":
    "Общая база данных с настольным приложением на удалённой машине (лучше, когда версии совпадают). Выкл. = отдельная база данных.",

  // ── Sidebar ──
  "tree.newSession": "Новая сессия", // New Session
  "tree.newTerminalSession": "Новая сессия терминала", // New Terminal Session
  "tree.newBrowserPage": "Новая страница браузера", // New Browser Page
  "tree.newAgentSession": (agent) => `Новая сессия ${agent}`, // New {agent} Session
  "tree.newAgentSessionGroup": "Другие сессии агента", // More Agent Session
  "tree.newAgentSessionCustom": "Создать с аргументами…", // New with launch args…
  "tree.resumeSession": "Возобновить сессию…", // Resume Session…
  "tree.newGroup": "Новая группа", // New Group
  "tree.newSubgroup": "Новая подгруппа", // New Subgroup
  "tree.newChildSession": "Новая дочерняя сессия", // New Child Session
  "tree.openSelected": "Открыть выбранные сессии", // Open Selected Sessions
  "tree.archiveSelected": "Архивировать выбранные сессии", // Archive Selected Sessions
  "tree.moveSelected": "Переместить выбранное…", // Move Selected to…
  "tree.deleteSelected": (n) =>
    `Удалить ${n} ${plural(n, "выбранный элемент", "выбранных элемента", "выбранных элементов")}`, // Delete {n} Selected Items
  "tree.removeProject": "Убрать проект", // Remove Project
  "tree.deleteGroup": "Удалить группу", // Delete Group
  "tree.deleteSession": "Удалить сессию", // Delete Session
  "tree.projectRoot": "Корень проекта (без группы)", // Project root (no group)
  "tree.moveToSession": "Переместить под сессию (сделать дочерней)", // Move under a session (as child)
  "tree.moveTo": "Переместить в…", // Move to…
  "tree.openNewTab": "Открыть в новой вкладке", // Open in New Tab
  "tree.forkSession": "Форкнуть сессию", // Fork Session
  "tree.exportSession": "Экспортировать сессию…", // Export Session…
  "tree.sessionInfo": "Сведения о сессии", // Session Info
  "tree.groupInfo": "Сведения о группе", // Group Info
  "info.branch": "Ветка", // Branch
  "info.path": "Путь", // Path
  "info.recentCommits": "Последние коммиты", // Recent Commits
  "info.noCommits": "Нет коммитов", // No commits
  "tree.killProcess": "Завершить процесс", // Kill Process
  "tree.archiveSession": "Архивировать сессию", // Archive Session
  "tree.archiveGroup": "Архивировать группу", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "врем.", // scratch
  "tree.persistSession": "Сделать постоянной сессией…", // Make Permanent Session…
  "tree.persistDoc": "Сохранить на диск…", // Save to Disk…
  "tree.closeScratch": "Закрыть черновик", // Close Scratch
  "tree.importProject": "Импортировать проект", // Import Project
  "tree.createProject": "Создать проект",
  "tree.cloneProject": "Клонировать из Git", // Clone from Git
  "createProject.title": "Создать проект",
  "createProject.name": "Название проекта",
  "createProject.namePlaceholder": "мой-проект",
  "createProject.into": "Создать в",
  "createProject.choose": "Выбрать…",
  "createProject.noParent": "Выберите родительскую папку",
  "createProject.invalidName": "Введите одно имя папки без / и \\.",
  "createProject.creating": "Создание…",
  "createProject.submit": "Создать проект",
  "clone.title": "Клонировать репозиторий Git", // Clone Git Repository
  "clone.url": "URL репозитория", // Repository URL
  "clone.urlPlaceholder": "https://… или git@…",
  "clone.branch": "Ветка (необязательно)", // Branch (optional)
  "clone.branchPlaceholder": "Пусто — ветка по умолчанию", // Default branch if empty
  "clone.folder": "Имя папки", // Folder name
  "clone.folderPlaceholder": "Автоматически из URL", // Auto from URL
  "clone.into": "Клонировать в", // Clone into
  "clone.choose": "Выбрать…", // Choose…
  "clone.noParent": "Выберите родительскую папку", // Choose a parent folder
  "clone.cloning": "Клонирование…", // Cloning…
  "clone.cancelling": "Отмена…",
  "clone.stageStarting": "Запуск Git…",
  "clone.stageConnecting": "Подключение к репозиторию…",
  "clone.stagePreparing": "Подготовка объектов…",
  "clone.stageReceiving": "Получение объектов…",
  "clone.stageResolving": "Разрешение дельт…",
  "clone.stageCheckout": "Извлечение файлов…",
  "clone.stageFinalizing": "Завершение…",
  "clone.stageImporting": "Импорт проекта…",
  "clone.elapsed": (seconds: number) => `Прошло ${seconds} с`,
  "clone.slowHint": "Нет прогресса в течение 30 секунд. Проверьте сеть или прокси удалённого компьютера; можно отменить и повторить попытку.",
  "clone.submit": "Клонировать", // Clone
  "tree.globalSearch": "Искать во всех сессиях (⌘⇧F)", // Search All Sessions (⌘⇧F)
  "tree.archivedSessions": "Архив сессий", // Archived Sessions
  "tree.searchPlaceholder": "Поиск сессий / групп…", // Search sessions / groups…
  "tree.clearSearch": "Очистить поиск", // Clear search
  "tree.filterWorking": "В работе", // Working
  "tree.filterAsking": "Ожидает", // Pending
  "tree.filterWaiting": "Просмотрено", // Viewed
  "tree.filterStatus": "Фильтр по статусу", // Filter by status
  "tree.refreshStatusFilter": "Обновить фильтр по статусу",
  "tree.refreshStatusMatch": "Обновить статус",
  "tree.filterStatusSection": "Статус", // Status
  "tree.filterMarkSection": "Метка", // Mark
  "tree.viewMainName": "Основной",
  "tree.viewUntitled": "Безымянное представление",
  "tree.viewDefaultName": (n) => `Представление ${n}`,
  "tree.viewPrimary": "Основное представление",
  "tree.viewManage": "Управление представлением",
  "tree.viewSetPrimary": "Сделать основным",
  "tree.viewRename": "Переименовать представление",
  "tree.viewName": "Название представления",
  "tree.viewDelete": "Удалить представление",
  "tree.viewDeletePrimary": "Основное представление нельзя удалить",
  "tree.viewDeleteTitle": "Удалить представление дерева",
  "tree.viewDeleteConfirm": (name) =>
    `Удалить «${name}»? Сохранённые поиск и фильтры будут удалены; проекты и сеансы не изменятся.`,
  "tree.viewSplitRight": "Разделить представление дерева вправо",
  "tree.viewSplitDown": "Разделить представление дерева вниз",
  "tree.viewAdd": "Скопировать текущее представление в новую вкладку",
  "tree.viewCount": (n) => `Представлений дерева: ${n}`,
  "mark.menu": "Метка", // Mark
  "mark.urgent": "Срочно", // Urgent
  "mark.important": "Важно", // Important
  "mark.bug": "Ошибка", // Bug
  "mark.done": "Готово", // Done
  "mark.wip": "В работе", // In progress
  "mark.pinned": "Закреплено", // Pinned
  "mark.idea": "Идея", // Idea
  "mark.caution": "Внимание", // Caution
  "tree.clearAllNotifications":
    "Сбросить все индикаторы уведомлений (точки сессий и значок в Dock)", // Clear all notification badges…
  "tree.noProjectsPre": "Проектов пока нет. Нажмите на значок папки или клавиши ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": ", чтобы импортировать каталог.", // to import a directory.
  "tree.openProject": "Открыть проект", // Open Project
  "tree.noAttention": "Нет сессий, соответствующих фильтру статуса", // No sessions match the status filter
  "tree.noMatch": "Совпадений нет", // No matches

  // Dialog fields
  "tree.groupName": "Название группы", // Group name
  "tree.sessionNameAuto": "Название сессии (пусто = автоматически)", // Session name (leave empty to auto-name)
  "tree.editSession": "Изменить сессию", // Edit Session
  "tree.sessionName": "Название сессии", // Session name
  "tree.shellLabel": "Shell (пусто = системный по умолчанию)", // Shell (leave empty for system default)
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "Скачать полный Git Bash",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "Загрузка полного Git Bash…",
  "gitbash.extracting": "Распаковка полного Git Bash…",
  "gitbash.done": "Полный Git Bash готов.",
  "gitbash.failed": "Не удалось скачать Git Bash",
  "tree.shellSystemDefault": "Системный по умолчанию", // System default
  "form.customOption": "Другой…", // Custom…
  "tree.cwdLabel": "Рабочий каталог (пусто = корень проекта)", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "Команда запуска (необязательно)", // Startup command (optional)
  "tree.agentArgsLabel": "Аргументы запуска (необязательно)", // Launch args (optional)
  "tree.permissionSkipLabel": "Пропускать все подтверждения разрешений", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "Запускает с флагом обхода этого агента (напр. Claude --dangerously-skip-permissions; Codex также отключает песочницу). Применяется при каждом запуске — используйте с осторожностью.",
  "tree.permissionUnsupported":
    "OpenCode управляет разрешениями через файл конфигурации — флага запуска нет, поэтому опция неприменима.",
  "tree.permissionUnsupportedPi":
    "Pi по замыслу выполняет инструменты без запросов разрешений — опция неприменима.",

  // Диалог «Новая сессия агента»
  "newAgent.desc":
    "При желании задайте имя сессии и добавьте свои аргументы запуска (передаются команде агента, напр. --model opus). Оставьте оба поля пустыми и нажмите Enter, чтобы запустить как обычно.", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "Пакетное удаление", // Batch Delete
  "tree.deleteProjectTitle": "Удалить проект", // Delete Project
  "tree.deleteGroupTitle": "Удалить группу", // Delete Group
  "tree.deleteSessionTitle": "Удалить сессию", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `Удалить ${n} ${plural(n, "выбранный элемент", "выбранных элемента", "выбранных элементов")} (проекты/группы каскадно удаляют свои подгруппы и сессии)? Это действие необратимо.`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `Удалить проект «${name}»? Все его подгруппы и сессии тоже будут удалены. Это действие необратимо.`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `Удалить группу «${name}»? Все её подгруппы и сессии тоже будут удалены. Это действие необратимо.`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `Удалить сессию «${name}» (и все её дочерние сессии)? Это действие необратимо.`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `Также удалить связанные git worktree (всего ${n}; удаление может не сработать, если в рабочем дереве есть изменения)`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "Название", // Name
  "info.type": "Тип", // Type
  "info.status": "Состояние", // Status
  "info.notYetCaptured": "Ещё не создан (фиксируется после первого запуска)", // Not yet generated (captured after first run)
  "info.sessionId": "ID сессии", // Session ID
  "info.cwd": "Каталог", // Working dir
  "info.initCmd": "Команда", // Startup cmd
  "info.agentArgs": "Аргументы запуска", // Launch args
  "info.launchCmd": "Полная команда запуска", // Full launch command
  "info.permission": "Разрешения", // Permission
  "info.permissionSkip": "Пропускать все подтверждения", // Skip all confirmations
  "info.parentSessionId": "ID родителя", // Parent ID
  "info.termTitle": "Заголовок терминала", // Terminal title
  "info.createdAt": "Создано", // Created at

  // Resume-session dialog
  "resume.title": "Возобновить сессию", // Resume Session
  "resume.desc":
    "Выберите тип агента и введите собственный session id агента; при открытии продолжится исходный диалог.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Тип агента", // Agent type
  "resume.sessionIdPlaceholder": "Session id диалога", // Conversation session id
  "resume.confirm": "Возобновить и открыть", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Новая сессия worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Имя worktree", // Worktree name
  "worktree.worktreeNameHint": "Используется как имя каталога worktree и ветки.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Не удалось создать worktree", // Couldn't create the worktree
  "worktree.noRepoRoot": "У этого проекта нет пригодного пути к git-репозиторию.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Без", // None
  "worktreeSel.modeNew": "Новый", // New
  "worktreeSel.modeExisting": "Существующий", // Existing
  "worktreeSel.loading": "Загрузка worktree…", // Loading worktrees…
  "worktreeSel.empty": "В этом репозитории нет существующих worktree.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "Не удалось получить список worktree (не git-репозиторий?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "Сессии, созданные в этой группе, по умолчанию будут использовать это worktree.", // Sessions created in this group will use this worktree by default.

  // ── Archive panel ──
  "archive.title": "Архив сессий", // Archived Sessions
  "archive.empty1": "Архивных сессий нет.", // No archived sessions.
  "archive.empty2":
    "Щёлкните сессию в боковой панели правой кнопкой и выберите «Архивировать сессию», чтобы убрать её сюда.", // Right-click a session in the sidebar…
  "archive.restore": "Восстановить как обычную сессию", // Restore to normal session
  "archive.export": "Экспортировать полный контекст в Markdown", // Export full context as Markdown
  "archive.deleteForever": "Удалить навсегда (вместе с записью)", // Delete permanently (with recording)
  "archive.pickOne": "Выберите архивную сессию слева, чтобы посмотреть стенограмму", // Select an archived session on the left…
  "archive.recordingEnd": "--- Конец записи ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `Не удалось прочитать запись: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "Поиск в записи…", // Search in recording…
  "archive.searchTranscript": "Поиск по стенограмме…", // Search transcript…
  "archive.searchPlaceholder": "Поиск по архиву…", // Search archived content…
  "archive.msgCountAll": (n) =>
    `${n} ${plural(n, "сообщение", "сообщения", "сообщений")}`, // {n} messages
  "archive.msgCountFiltered": (shown, total) =>
    `${shown} / ${total} ${plural(total, "сообщение", "сообщения", "сообщений")}`, // {shown} / {total} messages
  "archive.you": "Вы", // You
  "archive.toolsUsed": (tools) => `Инструменты: ${tools}`, // Tools: {tools}
  "archive.noMatch": "Совпадающих сообщений нет", // No matching messages
  "archive.emptyTranscript": "Стенограмма пуста", // Transcript is empty
  "archive.loadingTranscript": "Загрузка стенограммы…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "Поиск по содержимому всех сессий…", // Search across all session content…
  "search.hint": "Поиск по содержимому сессий. Архивные по умолчанию исключены — отметьте «Включая архив», чтобы добавить их.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Включая архив", // Include archived
  "search.includeArchivedHint": "Искать также в архивных сессиях (по умолчанию выкл.)", // Also search archived sessions (off by default)
  "search.searching": "Поиск…", // Searching…
  "search.noResults": "Совпадений не найдено", // No matches found
  "search.sessionCount": (n) =>
    `${n} ${plural(n, "сессия", "сессии", "сессий")}`, // n sessions
  "search.matchCount": (n) =>
    `${n} ${plural(n, "совпадение", "совпадения", "совпадений")}`, // n matches
  "search.pickSession": "Выберите сессию слева, чтобы увидеть совпадения", // Select a session on the left to see its matches
  "search.openSession": "Открыть сессию", // Open session
  "search.backToResults": "Назад к результатам", // Back to results
  "search.archivedBadge": "В архиве", // Archived
  "search.summary": (m, s) =>
    `${m} ${plural(m, "совпадение", "совпадения", "совпадений")} · ${s} ${plural(s, "сессия", "сессии", "сессий")}`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} из ${total}`, // N of M
  "search.roleTerminal": "Терминал", // Terminal
  "search.collapseGroup": "Свернуть", // Collapse
  "search.expandGroup": "Развернуть", // Expand
  "search.cappedNote": (l, total) => `${l} из ${total} доступно для перехода`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "Нет сессии", // No session
  "center.noSessionHintPre": "Выберите сессию в боковой панели или нажмите ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": ", чтобы создать терминал", // to create a terminal
  "center.createTerminal": "Создать терминал", // Create Terminal
  "tab.unsavedDot": "Несохранённые изменения", // Unsaved changes
  "tab.newTerminal": "Новый терминал", // New terminal
  "tab.newDocument": "Новый документ", // New document
  "tab.bgTitle": (n) =>
    `Фоновые вкладки: ${n} (процессы продолжают работать)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Фон ${n}`, // Background {n}
  "tab.scratchFallback": "(временный терминал)", // (scratch terminal)
  "tab.killBgTab": "Завершить эту фоновую вкладку (её процессы завершатся)", // Kill this background tab…
  "tab.newBrowserTab": "Новая вкладка", // New Tab
  "tab.refreshFile": "Обновить файл", // Refresh File
  "tab.closeOthers": "Закрыть другие вкладки", // Close Other Tabs
  "tab.closeRight": "Закрыть вкладки справа", // Close Tabs to the Right
  "tab.closeAll": "Закрыть все вкладки", // Close All Tabs
  "tab.sendToBackground": "Свернуть в фоновый режим", // Send to Background

  // ── Встроенный браузер ──
  "browser.back": "Назад", // Back
  "browser.forward": "Вперёд", // Forward
  "browser.reload": "Обновить", // Reload
  "browser.stop": "Остановить загрузку", // Stop loading
  "browser.openExternal": "Открыть в системном браузере", // Open in system browser
  "browser.addressPlaceholder": "Введите URL или поисковый запрос", // Enter URL or search terms
  "browser.quickAccess": "Быстрый доступ", // Quick access
  "browser.loading": "Загрузка…", // Loading…
  "overlimit.title": (max) => `Превышен лимит фоновых вкладок (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body": "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "Вставить", // Paste
  "term.pasteUseShortcut": "Вставить (нажмите ⌘V)", // Paste (press ⌘V)
  "term.selectAll": "Выделить всё", // Select All
  "term.autoCopied": (n: number) => `Скопировано ${n} симв. · ⌘V`,
  "term.clear": "Очистить", // Clear
  "term.searchMenu": "Поиск…", // Search…  ⌘F
  "term.splitRight": "Разделить вправо", // Split right (⌘D)
  "term.splitDown": "Разделить вниз", // Split down (⌘⇧D)
  "term.closePane": "Закрыть панель", // Close split
  "term.redraw": "Перерисовать", // Redraw
  "term.mirrorTooltip":
    "Зеркальный режим (размером управляет другой клиент). Нажмите, чтобы подогнать PTY под это окно", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) => `⤢ Зеркало${dims} · нажмите, чтобы подогнать под окно`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ Зеркало${dims} · подогнать под окно`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `Не удалось загрузить ${n} ${plural(n, "изображение", "изображения", "изображений")}${lastError ? `: ${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "Не удалось прочитать изображение из буфера обмена. Скопируйте его ещё раз и повторите попытку.",
  "term.starting": (agent) => `Запуск ${agent}…`, // Starting {agent}…
  "term.startFailed": (err) => `Не удалось запустить: ${err}`, // Failed to start: {err}

  // ── Карточка помощи с установкой агента ──
  "agentInstall.title": (label) => `${label} не установлен`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm не нашёл ${label} в PATH. Установите его, чтобы запустить эту сессию.`, // couldn't find {label} on PATH
  "agentInstall.install": "Установить", // Install now
  "agentInstall.retry": "Запустить снова", // Retry launch
  "agentInstall.dismiss": "Установлю сам", // I'll do it myself
  "agentInstall.docs": "Документация", // Install docs
  "agentInstall.needsNode": "Требуется Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "После установки:", // After install:
  "agentInstall.pathSaved": (label: string) => `Путь к исполняемому файлу ${label} сохранён в настройках:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} установлен`, // {label} is installed
  "agentInstall.doneDesc": "Перезапустите эту сессию, чтобы начать работу.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Перезапустить сейчас", // Relaunch now
  "agentInstall.later": "Позже", // Later
  "search.placeholder": "Поиск в терминале", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "Визуальный", // WYSIWYG
  "doc.source": "Исходник", // Source
  "doc.searchPlaceholder": "Поиск", // Find
  "doc.searchReplacePlaceholder": "Замена", // Replace
  "doc.searchReplace": "Заменить", // Replace
  "doc.searchReplaceAll": "Все", // All
  "doc.searchNoMatch": "Нет совпадений", // No results
  "doc.searchCaseSensitive": "Учитывать регистр", // Match case
  "doc.searchToggleReplace": "Переключить замену", // Toggle replace
  "doc.fileTree": "Дерево файлов", // File tree
  "doc.treeUp": "Родительская папка", // Parent folder
  "doc.sidebar": "Боковая панель", // Sidebar
  "doc.unsaved": "Не сохранено", // Unsaved
  "doc.saveAsTitle": "Сохранить как", // Save As
  "doc.saveAsName": "Имя файла", // File name
  "doc.outline": "Структура", // Outline
  "doc.outlineEmpty": "Нет заголовков", // No headings
  "doc.saving": "Сохранение…", // Saving…
  "doc.overwriteConfirm": "Файл с таким именем уже существует. Нажмите «Перезаписать», чтобы заменить его.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Сохранить (⌘S)", // Save (⌘S)
  "doc.externalChanged":
    "Файл изменён на диске (у вас есть несохранённые локальные изменения).", // The file was modified on disk…
  "doc.reloadDiscard": "Перезагрузить (отбросить мои изменения)", // Reload (discard my changes)
  "doc.externalChangedClean": "Файл изменён на диске.", // The file was modified on disk.
  "doc.reload": "Перезагрузить", // Reload
  "doc.ignore": "Игнорировать", // Ignore
  "doc.loadingFile": (title) => `Загрузка ${title}…`, // Loading {title}…
  "doc.closeTitle": "Закрыть документ", // Close Document
  "doc.unsavedBody": (title) => `В «${title}» есть несохранённые изменения.`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "Сохранить и закрыть", // Save & Close
  "doc.closeNoSave": "Закрыть без сохранения", // Close Without Saving
  "doc.conflictTitle": "Конфликт сохранения", // Save Conflict
  "doc.conflictBody":
    "Файл на диске был изменён извне. Всё равно перезаписать текущим содержимым?", // The file on disk was modified externally…
  "doc.overwrite": "Перезаписать", // Overwrite
  "doc.saveFailed": (err) => `Не удалось сохранить: ${err}`, // Save failed: {err}
  "doc.closeTab": "Закрыть вкладку", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `Только чтение: показаны первые 10 МБ из ${size}. Сохранение отключено, чтобы не перезаписать остальную часть файла.`,
  "doc.imgLoading": (title, size) => `Загрузка ${title} (${size})…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "Файл сейчас записывается; он будет перезагружен автоматически, как только запись завершится.", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed": "Не удаётся отобразить это изображение (неподдерживаемый или повреждённый формат).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Вписать", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Экспорт в PDF", // Export PDF
  "doc.diagramError": "Ошибка диаграммы", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Сессия не выбрана", // No session selected
  "panel.openInEditor": "Открыть в редакторе", // Open in Editor
  "panel.openInEditorTooltip":
    "Открыть в редакторе документов в центральной панели (как команда view)", // Open in the document editor…
  "panel.preview": "Предпросмотр", // Preview
  "panel.cantRead": "(не удаётся прочитать этот файл)", // (cannot read this file)
  "panel.binary": "(двоичный файл, предпросмотра нет)", // (binary file, no preview)
  "panel.truncated": "\n…(содержимое обрезано)", // …(content truncated)
  "panel.showHidden": "Показать скрытые файлы", // Show hidden files
  "panel.hideHidden": "Скрыть скрытые файлы", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "Новый файл", // New File
  "files.newFolder": "Новая папка", // New Folder
  "files.nameLabel": "Имя", // Name
  "files.newTooltip": "Новый файл или папка", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.deleteConfirm": (name) => `Удалить «${name}»? Это действие нельзя отменить.`, // Delete "{name}"? This can't be undone.

  // ── Status bar ──
  "statusbar.sessions": (n) =>
    `${n} ${plural(n, "сессия", "сессии", "сессий")}`, // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Нажмите, чтобы показать в боковой панели только сессии «${label}» (нажмите ещё раз, чтобы сбросить)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Фон ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Фоновые вкладки (лимит ${max}; при превышении автоматически завершается самая старая неактивная)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) => `Фоновая вкладка завершена: ${name} (превышен лимит)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Удалённый доступ через браузер включён: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Права: спрашивать", // Perms: Ask
  "statusbar.permSkip": "Права: пропускать", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip": "Режим прав этой сессии · нажмите, чтобы изменить (только эта сессия)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Права этой сессии", // This session's permissions
  "statusbar.permOptAsk": "Спрашивать каждый раз (по умолчанию)", // Ask each time (default)
  "statusbar.permScopeHint": "Применяется только к этой сессии. Для глобальных настроек перейдите в Настройки ▸ Агенты.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "Права изменены. Чтобы применить, нужно перезапустить сессию. Перезапуск продолжит текущий диалог, но прервёт выполняемую задачу. Перезапустить сейчас?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Перезапустить", // Restart now
  "statusbar.permRestartLater": "Позже", // Later
  "statusbar.permScopeTitle": "Применить к?", // Apply to?
  "statusbar.permScopeSession": "Только эта сессия", // This session only
  "statusbar.permScopeGlobal": "Глобально по умолчанию", // Global default
  "statusbar.permScopeGlobalHint": "Применяется сейчас к этой сессии и становится значением по умолчанию для будущих новых сессий этого типа (синхронизировано с настройками).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ В работе…", // ⏳ Working…
  "notify.asking": "❓ Требуется ваше подтверждение", // ❓ Needs your confirmation
  "notify.waiting": "✅ Ответ готов", // ✅ Replied
  "store.subtask": "Подзадача", // Subtask
  "store.splitPane": "Панель", // Split
  "export.failedTitle": "Не удалось экспортировать сессию", // Failed to export session
  "export.contextSuffix": "контекст", // context

  // ── Error panel ──
  "err.renderTitle": "Ошибка отрисовки", // Rendering Error
  "err.renderDesc":
    "Произошла непредвиденная ошибка. Сведения ниже помогут найти причину.", // An unexpected error occurred…
  "err.reload": "Перезагрузить", // Reload
  "err.uncaughtTitle": "Неперехваченная ошибка", // Uncaught Error
  "err.uncaughtDesc": "Сведения ниже помогут найти причину.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "Воспроизведение записей в браузере пока не поддерживается", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Не удалось загрузить изображение (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "Подключение…", // Connecting…
  "login.remoteAccess": "Удалённый доступ", // Remote Access
  "login.desc": "Введите пароль доступа, чтобы подключиться к этому терминалу.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Пароль доступа", // Access password
  "login.connect": "Подключиться", // Connect
  "login.wrongPassword": "Неверный пароль", // Wrong password
  "login.failed": "Не удалось войти, попробуйте ещё раз", // Login failed, please try again
  "login.pairingRequired": "Этот сервер требует ссылку для сопряжения. Откройте ссылку, созданную в панели «Удалённый доступ» настольного приложения.", // This server requires a pairing link
  "login.authFailed": "Неверный пароль или ссылка для сопряжения устарела. Подключитесь снова по новой ссылке.", // Wrong password or pairing link expired
  "dir.title": "Выбор каталога проекта", // Choose Project Directory
  "dir.pathPlaceholder": "Поиск или введите путь и нажмите Enter (поддерживается ~)", // Search, or type a path and press Enter (supports ~)
  "dir.up": "На уровень вверх", // Up one level
  "dir.newFolder": "Новая папка", // New Folder
  "dir.newFolderPlaceholder": "Имя папки", // Folder name
  "dir.goInput": "Перейти по введённому пути", // Go to typed path
  "dir.noSubdirs": "(подкаталогов нет)", // (no subdirectories)
  "dir.empty": "(пустая папка)", // (empty folder)
  "dir.noMatch": "Нет совпадений", // No matching items
  "dir.target": "Целевая папка", // Target
  "dir.showHidden": "Показать скрытые элементы", // Show hidden items
  "dir.importing": "Импорт…", // Importing…
  "dir.choose": "Выбрать этот каталог", // Choose This Directory
  "conn.reconnecting": "Соединение потеряно, переподключение…", // Connection lost, reconnecting…
  "conn.reconnectNow": "Переподключиться сейчас", // Reconnect now
  "conn.retrying": "Переподключение…", // Reconnecting…
  "conn.sshReconnecting": "SSH-соединение потеряно, туннель восстанавливается…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown": "SSH-соединение разорвано — нажмите «Переподключиться сейчас», чтобы повторить", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Ошибка запроса", // Request failed
  "reqerr.dismiss": "Закрыть", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Журнал ошибок", // Error Log
  "errlog.empty": "Нет записанных ошибок.", // No errors recorded.
  "errlog.copyAll": "Копировать всё", // Copy all
  "errlog.clear": "Очистить", // Clear
  "errlog.close": "Закрыть", // Close

  // ── Mobile ──
  "mobile.toDesktop": "Перейти к версии для ПК", // Switch to desktop
  "mobile.empty1": "Сессий нет.", // No sessions.
  "mobile.noMatch": "Нет подходящих сессий", // No matching sessions
  "mobile.empty2":
    "Создайте сессию в настольном приложении или в браузере компьютера — она появится здесь автоматически.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Назад", // ‹ Back
  "mobile.selCopy": "Копировать", // Copy
  "mobile.selCancel": "Отмена", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "Перетащите, чтобы изменить размер", // Drag to resize
  "transport.wsDisconnected": "WebSocket отключён", // WebSocket disconnected
  "transport.wsConnectFailed": "Не удалось подключиться по WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "Команда не выполнена", // Command failed

  // ── Crepe（WYSIWYG-редактор）──
  "crepe.placeholder": "Введите текст или нажмите / для меню вставки", // Type text, or press / for the insert menu
  "crepe.textGroup": "Текст", // Text
  "crepe.paragraph": "Текст", // Text
  "crepe.h1": "Заголовок 1", // Heading 1
  "crepe.h2": "Заголовок 2", // Heading 2
  "crepe.h3": "Заголовок 3", // Heading 3
  "crepe.h4": "Заголовок 4", // Heading 4
  "crepe.h5": "Заголовок 5", // Heading 5
  "crepe.h6": "Заголовок 6", // Heading 6
  "crepe.quote": "Цитата", // Quote
  "crepe.divider": "Разделитель", // Divider
  "crepe.listGroup": "Список", // List
  "crepe.bulletList": "Маркированный список", // Bullet List
  "crepe.orderedList": "Нумерованный список", // Ordered List
  "crepe.taskList": "Список задач", // Task List
  "crepe.advancedGroup": "Вставка", // Insert
  "crepe.image": "Изображение", // Image
  "crepe.codeBlock": "Блок кода", // Code Block
  "crepe.table": "Таблица", // Table
  "crepe.math": "Формула", // Math
  "crepe.linkPlaceholder": "Вставьте или введите ссылку…", // Paste or type a link…
  "crepe.upload": "Загрузить", // Upload
  "crepe.uploadImage": "Загрузить изображение", // Upload Image
  "crepe.orPasteImageLink": "или вставьте ссылку на изображение", // or paste an image link
  "crepe.imageCaption": "Подпись к изображению", // Image caption
  "crepe.confirm": "Подтвердить", // Confirm
  "crepe.searchLanguage": "Поиск языка", // Search language
  "crepe.noResult": "Ничего не найдено", // No results
  "crepe.edit": "Редактировать", // Edit
  "crepe.collapse": "Свернуть", // Collapse
  // ── Правая панель / нижняя строка ──
  "info.project": "Проект", // Project
  "panel.sessionInfo": "Сведения о сессии", // Session info
  "panel.gitTitle": "Статус Git", // Git status
  "panel.gitProbing": "Проверка…", // Checking…
  "panel.gitNotRepo": "Не репозиторий Git", // Not a Git repository
  "panel.gitBranch": "Ветка", // Branch
  "panel.gitStaged": "Подготовлено", // Staged
  "panel.gitUnstaged": "Изменено", // Changed
  "panel.gitUntracked": "Неотслеживаемые", // Untracked
  "bottombar.running": "Выполняется", // Running
  "bottombar.collapseTasks": "Свернуть задачи", // Collapse tasks
  "bottombar.expandTasks": "Развернуть задачи", // Expand tasks
  "bottombar.sound": "🔔 Звук", // 🔔 Sound
  "bottombar.muted": "🔕 Без звука", // 🔕 Muted
  "bottombar.overview": "Обзор сессий", // Sessions overview
  "bottombar.noSessions": "Нет сессий", // No sessions
  "doc.pdfFilter": "Файл PDF", // PDF file
  // ── auto update ──
  "updater.title": "Check for Updates", // TODO translate
  "updater.upToDate": "You're already on the latest version.", // TODO translate
  "updater.failed": (err) => `Update check failed: ${err}`, // TODO translate
  "updater.available": "Update available", // TODO translate
  "updater.versionLine": (version, current) =>
    `Version ${version} — you're on ${current}`, // TODO translate
  "updater.noNotes": "No release notes were published for this version.", // TODO translate
  "updater.updateNow": "Update now", // TODO translate
  "updater.later": "Later", // TODO translate
  "updater.skipVersion": "Skip this version", // TODO translate
  "updater.skipVersionHint":
    "Stop reminding me about this version. You can still install it later from Check for Updates.", // TODO translate
  "updater.downloadingPct": (pct) => `Downloading… ${pct}%`, // TODO translate
  "updater.downloadingBytes": (mb) => `Downloading… ${mb} MB`, // TODO translate
  "updater.installing": "Installing…", // TODO translate
  "updater.installed": "Update installed. Restart to finish.", // TODO translate
  "updater.restartNow": "Restart now", // TODO translate
  "updater.retry": "Try again", // TODO translate
  "updater.downloadFailed": (err) => `Update failed: ${err}`, // TODO translate
  "updater.hide": "Hide", // TODO translate
  "updater.hideHint": "Keep downloading in the background. Progress stays in the status bar.", // TODO translate
  "updater.downloadManually": "Download manually", // TODO translate
  "updater.downloadManuallyHint": "Open the installer download in your browser.", // TODO translate
  "updater.windowsNotice":
    "VelaTerm will close while the installer runs, then reopen on its own.", // TODO translate
  "updater.installingWindows":
    "Installing… VelaTerm is about to close. The installer will finish the update and reopen it.", // TODO translate
  "statusbar.updateAvailable": (version) => `Update ${version}`, // TODO translate
  "statusbar.updateDownloading": (pct) => `Updating… ${pct}%`, // TODO translate
  "statusbar.updateInstalling": "Installing…", // TODO translate
  "statusbar.updateReady": "Restart to update", // TODO translate
  "statusbar.updateFailed": "Update failed", // TODO translate
  "statusbar.updateTooltip": "Click for details", // TODO translate
};

export default ru;
