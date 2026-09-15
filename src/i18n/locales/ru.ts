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
  "tree.newPlanExecuteSession": "Новая сессия планирования и выполнения…",
  "launch.splitTasks": "Автоматически разделить на несколько задач",
  "launch.splitTasksHint": "Сеанс планирования предложит независимые задачи. Перед запуском проверьте инструкции, агентов, модели и глубину рассуждений.",
  "launch.splitReview": "Проверка задач для выполнения",
  "launch.splitReviewHint": "Один сеанс планирования получает все отчёты и проверяет каждую задачу отдельно. Выполнение начинается только после вашего подтверждения.",
  "launch.splitConfirmed": "Эти задачи уже подтверждены.",
  "launch.splitClosed": "Это предложение больше не ожидает подтверждения.",
  "launch.splitRetry": "Повторить отправку недоставленных сообщений",
  "launch.splitSharedDirectory": "Все сеансы выполнения используют рабочий каталог сеанса планирования и его рабочее дерево, если оно включено.",
  "launch.createIn": "Создать в",
  "launch.workingDirectory": "Путь к рабочему каталогу",
  "launch.createAndStart": "Создать и запустить",
  "launch.planExecuteTaskHint": "Опишите задачу, требования и критерии приёмки для составления плана.",
  "launch.planExecuteResult": "Сначала запускается сессия планирования. Когда план готов, она создаёт сессию выполнения.",
  "launch.planExecuteWorktreeHint": "Новые рабочие деревья создаются из текущего коммита без незакоммиченных изменений. Если создать дерево не удалось, соответствующая сессия не запускается.",
  "launch.workflowDirectorySharedHint": "Сессия планирования и все сессии выполнения используют один новый каталог и общую ветку.",
  "launch.workflowDirectoryEachHint": "Сессия планирования и каждая сессия выполнения получают собственное рабочее дерево и отдельную ветку.",
  "launch.planTitle": "Планирование и проверка",
  "launch.execTitle": "Выполнение",
  "launch.planExecuteIntro": "Отдельная сессия планирует работу, проверяет результат и запрашивает исправления.",
  "chat.origin.plan": "Планирование",
  "chat.origin.exec": "Выполнение",

  // Project code intelligence and knowledge entry associations.
  "knowledge.callers": "Вызывающие символы",
  "knowledge.callees": "Вызываемые символы",
  "knowledge.explore": "Исследование кода",
  "knowledge.exploreHint": "Опишите функцию или последовательность вызовов либо укажите файл или символ…",
  "knowledge.impact": "Анализ влияния",
  "knowledge.path": "Путь вызовов",
  "knowledge.target": "Найти целевой символ…",
  "knowledge.depth": "Глубина обхода",
  "knowledge.noPath": "В индексе не найден направленный путь вызовов.",
  "knowledge.watching": "Автосинхронизация включена",
  "knowledge.onDemand": "Синхронизация перед запросами",
  "knowledge.overview": "Обзор",
  "knowledge.uncertain": "Предполагаемая связь",
  "knowledge.kind": "Тип символа",
  "knowledge.language": "Язык",
  "knowledge.results": "Результаты",
  "knowledge.resultLarge": "Результат слишком велик для отображения. Сузьте запрос или уменьшите глубину обхода.",
  "knowledge.queryFailed": "Запрос кода завершился ошибкой. Повторите запрос или синхронизируйте индекс.",
  "knowledge.liveHelp": "Изменения файлов синхронизируются, пока работает процесс запросов. После его завершения из-за простоя следующий запрос сначала учтёт накопленные изменения.",
  "knowledge.startHelp": "Включите индексацию для поиска кода, отслеживания вызовов и анализа влияния изменений. Анализ выполняется на сервере без модели ИИ.",
  "knowledge.title": "Граф кода",
  "knowledge.intro": "Изучайте связи в коде и связывайте их с сохранёнными проектными решениями.",
  "knowledge.setup": "Установите CodeGraph на этом сервере, чтобы включить индексацию проектов.",
  "knowledge.downloadNotice": "Проверенная среда выполнения CodeGraph будет загружена с GitHub. Индексация выполняется на этом компьютере; телеметрия и проверка обновлений отключены.",
  "knowledge.install": "Скачать CodeGraph",
  "knowledge.installing": "Загрузка и установка…",
  "knowledge.directory": "Рабочий каталог",
  "knowledge.enable": "Включить индексацию",
  "knowledge.disable": "Отключить индексацию",
  "knowledge.sync": "Синхронизировать",
  "knowledge.ready": "Готово",
  "knowledge.disabled": "Отключено",
  "knowledge.indexing": "Индексация…",
  "knowledge.syncing": "Синхронизация…",
  "knowledge.failed": "Ошибка",
  "knowledge.symbols": "Символы",
  "knowledge.files": "Файлы",
  "knowledge.edges": "Связи",
  "knowledge.search": "Поиск символов или путей к файлам…",
  "knowledge.searchButton": "Найти",
  "knowledge.noResults": "Подходящих символов нет.",
  "knowledge.selectSymbol": "Выберите символ, чтобы увидеть исходный код, связи и связанные статьи базы знаний.",
  "knowledge.source": "Исходный код",
  "knowledge.incoming": "Входящие связи",
  "knowledge.outgoing": "Исходящие связи",
  "knowledge.noEdges": "В индексе нет связей.",
  "knowledge.analysisNote": "Связи получены статическим анализом и могут быть неполными или неточными.",
  "knowledge.changed": "Файл изменился во время запроса. Повторите синхронизацию, прежде чем использовать номера строк или подтверждать проверку.",
  "knowledge.truncated": "Объём отображаемых данных ограничен. Некоторые связи или строки кода пропущены.",
  "knowledge.linkMemory": "Связать со статьёй",
  "knowledge.chooseMemory": "Выберите статью базы знаний",
  "knowledge.noLinks": "Связей с кодом пока нет. Статью можно связать с кодом в представлении символа.",
  "knowledge.inspect": "Проверить код и статью",
  "knowledge.unlink": "Удалить связь",
  "knowledge.codeReferences": "Ссылки на код",
  "knowledge.refresh": "Обновить",
  "knowledge.current": "Без изменений",
  "knowledge.review": "Требуется проверка",
  "knowledge.unavailable": "Недоступно",
  "knowledge.reviewHelp": "Сопоставьте эту статью с показанным кодом. Подтверждение сохраняет текущую версию файла, не изменяя текст статьи.",
  "knowledge.confirmReview": "Подтвердить проверку",
  "knowledge.agentHint": "Агенты могут выполнять vkb search \"тема\" в этом каталоге. Запросы синхронизируют включённые индексы и возвращают код и статьи базы знаний отдельно.",
  "knowledge.busy": "Выполняется индексация. Можно закрыть эту страницу или отключить индексацию, чтобы остановить задачу.",
  "knowledge.disabledHelp": "Включите индексацию этого каталога для запросов к коду. При отключении индекс и связи со статьями сохраняются.",
  "knowledge.conflict": "Код или статья изменились. Загрузите их заново перед сохранением связи.",
  "knowledge.symbolMissing": "Символ или исходный код больше недоступны. Синхронизируйте индекс и повторите поиск.",
  "knowledge.directoryMissing": "Рабочий каталог отсутствует или изменился. Проверьте пути проекта и сеанса.",
  "knowledge.partial": "Индекс неполон. Повторите синхронизацию и проверьте доступность исходных файлов для чтения.",
  "knowledge.interrupted": "Предыдущая задача была прервана. Запустите синхронизацию для повторной попытки.",
  "knowledge.checksum": "Контрольная сумма загрузки не совпала. Среда выполнения не установлена.",
  "knowledge.downloadFailed": "Не удалось скачать CodeGraph. Проверьте подключение сервера к GitHub и повторите попытку.",
  "knowledge.timeout": "Время индексации истекло. Проверьте размер репозитория и повторите попытку.",
  "knowledge.error": "Операция не выполнена. Проверьте доступ сервера к каталогам и среду выполнения, затем повторите попытку.",

  // Knowledge base: saved knowledge organized by project and session.
  "memory.hierarchy": "Проекты и сеансы",
  "memory.up": "На уровень выше",
  "memory.manualGroup": "Создано вручную",
  "memory.legacyGroup": "Ранее объединённые записи",
  "memory.unknownProject": "Исходный проект неизвестен",
  "nb.addLink": "Вставить ссылку",
  "nb.attach": "Прикрепить файл",
  "nb.browse": "Обзор",
  "nb.chooseNote": "Начните с заметки",
  "nb.closeHint": "Убрать этот блокнот из списка. Файлы останутся на диске.",
  "nb.closeVault": "Закрыть блокнот",
  "nb.conflict": "Файл изменён вне этого редактора. Ваш черновик сохранён. Загрузите файл заново или сохраните черновик как новую заметку.",
  "nb.copyTo": "Копировать в локальный блокнот",
  "nb.createVault": "Создать базу знаний",
  "nb.destination": "Путь назначения",
  "nb.download": "Скачать",
  "nb.downloadHint": "Скачайте вложение, чтобы открыть его в другом приложении.",
  "nb.empty": "Откройте папку или создайте блокнот, чтобы начать писать.",
  "nb.emptyImport": "Не выбраны файлы, доступные для импорта.",
  "nb.emptyNotes": "Заметки сохраняются в формате Markdown.",
  "nb.emptyOutline": "Здесь появятся заголовки документа.",
  "nb.emptyTrash": "Корзина пуста.",
  "nb.error": "Не удалось получить доступ к блокноту. Проверьте подключение и папку, затем повторите попытку.",
  "nb.exists": "Такой файл или папка уже существует. Выберите другое имя или папку.",
  "nb.favorites": "Избранное",
  "nb.files": "Файлы",
  "nb.folder": "Папка",
  "nb.generatedHint": "Знания, собранные из ваших сеансов, вместе с источниками и историей изменений.",
  "nb.homeHint": "Просматривайте базу знаний сеансов и локальные базы знаний.",
  "nb.homeSearch": "Поиск по знаниям сеансов и локальным заметкам…",
  "nb.loadMore": "Загрузить ещё",
  "nb.import": "Импорт",
  "nb.importFiles": "Выбрать файлы",
  "nb.importFolder": "Выбрать папку",
  "nb.importHint": "Файлы копируются в выбранную папку. Существующие файлы не перезаписываются, а скрытые папки настроек пропускаются.",
  "nb.imported": "Импортировано",
  "nb.imports": "История импорта",
  "nb.importsEmpty": "Импорта пока не было.",
  "nb.importRoot": "Корень базы знаний",
  "nb.importBusy": "Для этой базы знаний импорт уже выполняется.",
  "nb.importDelete": "Удалить запись",
  "nb.importDeleteConfirm": "Удалить эту запись об импорте? Уже импортированные файлы останутся.",
  "nb.importDone": "Импорт завершён",
  "nb.importDuration": (seconds: string) => `${seconds} с`,
  "nb.importFailed": "Не удалось выполнить импорт",
  "nb.importFilePending": "Не импортировано",
  "nb.importHideFiles": "Скрыть файлы",
  "nb.importInterruptedHint": "Импорт остановился до завершения.",
  "nb.importProgress": (done: string, total: string) => `${done} / ${total} файлов`,
  "nb.importShowFiles": (count: string) => `Файлы (${count})`,
  "nb.importSkipHidden": "Скрытый файл или папка",
  "nb.importStatusCancelled": "Отменён",
  "nb.importStatusCompleted": "Завершён",
  "nb.importStatusFailed": "Ошибка",
  "nb.importStatusInterrupted": "Прерван",
  "nb.importStatusRunning": "Импорт",
  "nb.incomplete": "Не удалось завершить операцию. Проверьте файлы и повторите попытку.",
  "nb.info": "Сведения о заметке",
  "nb.invalid": "Недопустимое имя или путь.",
  "nb.links": "Исходящие ссылки",
  "nb.local": "Локальные файлы",
  "nb.localVaults": "Локальные базы знаний",
  "nb.move": "Переименовать или переместить",
  "nb.moveHint": "Укажите путь относительно корня блокнота. При перемещении файла или папки существующие ссылки в заметках обновляются.",
  "nb.name": "Имя",
  "nb.newFolder": "Новая папка",
  "nb.newNote": "Новая заметка",
  "nb.noLinks": "Связанных заметок пока нет.",
  "nb.tags": "Теги",
  "nb.notes": "Заметки",
  "nb.openVault": "Открыть базу знаний",
  "nb.outline": "Структура",
  "nb.quickOpen": "Быстрое открытие",
  "nb.readOnly": "Этот файл нельзя редактировать как заметку Markdown в кодировке UTF-8.",
  "nb.recent": "Недавние заметки",
  "nb.restore": "Восстановить",
  "nb.reload": "Загрузить с диска",
  "nb.root": "Путь к папке",
  "nb.rootHint": "Выберите папку на подключённом компьютере. Существующие файлы Markdown и вложения останутся на своих местах.",
  "nb.saveCopy": "Сохранить как новую заметку",
  "nb.saved": "Сохранено на диске",
  "nb.saving": "Сохранение…",
  "nb.search": "Поиск заметок…",
  "nb.searchAllVaults": "Все базы знаний",
  "nb.searchCount": (count: string) => `${count} результатов`,
  "nb.searchEmpty": "Нет заметок, соответствующих этому запросу.",
  "nb.searchEmptyAll": "Ничего не найдено по этому запросу.",
  "nb.searchFuzzy": "Точных совпадений нет. Показаны приблизительные результаты.",
  "nb.searchLine": (line: string) => `Строка ${line}`,
  "nb.searchMatches": (count: string) => `${count} совпадений`,
  "nb.searchMore": "Показаны только первые результаты. Уточните запрос, чтобы увидеть остальные.",
  "nb.searchRelated": "Связанные заметки",
  "nb.searchResults": "Результаты поиска",
  "nb.searchScope": "Область поиска",
  "nb.searchThisVault": "Эта база знаний",
  "nb.skipped": "Пропущено",
  "nb.split": "Раздельный вид",
  "nb.tooLarge": "Файл или выбранные данные превышают ограничения блокнота.",
  "nb.trash": "Корзина",
  "nb.trashHint": "Переместить этот элемент в корзину блокнота. Позже его можно будет восстановить.",
  "nb.unsaved": "Несохранённые изменения",
  "nb.vaults": "Базы знаний",
  "nb.view": "Режим просмотра",
  "nb.welcome": "Ваши блокноты",
  "nb.welcomeText": "Записывайте мысли, связывайте идеи и храните заметки в обычных локальных файлах. Откройте существующую папку Markdown или импортируйте документы в новый блокнот.",
  "memory.globalMemory": "База знаний сеансов",
  "memory.collections": "Архивные сессии",
  "memory.collectionConversation": "Диалог",
  "memory.collectionEmptyEntries": "В этом диалоге пока нет статей базы знаний.",
  "memory.title": "База знаний",
  "memory.add": "Подготовить для базы знаний сеансов",
  "memory.intro": "Упорядочивайте знания по проектам и сеансам. Сохранённые статьи не зависят от изменений источников и доступны для ручного редактирования.",
  "memory.entries": "Статьи базы знаний",
  "memory.emptyJobs": "История обработки пока пуста.",
  "memory.jobs": "История обработки",
  "memory.search": "Поиск по заголовкам и содержимому…",
  "memory.empty": "Подходящих статей нет. Создайте статьи из сеанса или добавьте статью вручную.",
  "memory.emptyDetail": "Выберите статью, чтобы прочитать её и просмотреть связи и источники.",
  "memory.new": "Новая статья",
  "memory.titleField": "Заголовок",
  "memory.summary": "Краткое описание",
  "memory.content": "Содержимое (Markdown)",
  "memory.tags": "Метки (через запятую)",
  "memory.related": "Связанные статьи",
  "memory.backlinks": "Ссылки на эту статью",
  "memory.sources": "Источники",
  "memory.history": "История версий",
  "memory.restore": "Восстановить эту версию",
  "memory.restoreConfirm": "Восстановить эту редакцию как новую версию? Текущая версия останется в истории.",
  "memory.deleteConfirm": "Удалить эту статью и историю её версий? Исходные сеансы сохранятся.",
  "memory.groupDeleteConfirm": (count: string) => `Удалить все статьи базы знаний этой группы (${count} шт.)? Сам проект или сеанс сохранится.`,
  "memory.export": "Экспорт в Markdown",
  "memory.selectAgent": "Агент",
  "memory.model": "Модель (необязательно)",
  "memory.modelHint": "Оставьте поле пустым, чтобы использовать модель из настроек агента.",
  "memory.compile": "Упорядочить и сохранить",
  "memory.compileHelp": "Выбранный агент обработает этот сеанс. Повторное создание заменит ранее созданные для этого сеанса записи, включая ручные правки. Текст сеанса будет отправлен модели через настроенного агента.",
  "memory.unavailable": "Не установлен или не настроен",
  "memory.allTags": "Все метки",
  "memory.updated": "Недавно обновлённые",
  "memory.titleSort": "По заголовку",
  "memory.sourceNote": "Этот снимок сохраняет текст беседы, использованный при обработке, даже после удаления исходного сеанса.",
  "memory.noKnowledge": "Знания для повторного использования не найдены. Ранее созданные записи этого сеанса удалены.",
  "memory.queued": "Ожидает запуска",
  "memory.cancelling": "Отмена",
  "memory.schedulingHint": "Разные сеансы можно обрабатывать параллельно. Повторная отправка отменяет незавершённую задачу этого сеанса и заменяет её новой.",
  "memory.waitingHint": "Эта задача запустится автоматически после остановки предыдущей задачи этого сеанса.",
  "memory.running": "Выполняется",
  "memory.completed": "Завершено",
  "memory.failed": "Ошибка",
  "memory.cancelled": "Отменено",
  "memory.extract": "Извлечение тем",
  "memory.merge": "Объединение знаний",
  "memory.commit": "Сохранение статей",
  "memory.done": "Сохранено",
  "memory.closeHint": "Во время обработки окно можно закрыть. Следить за ходом работы можно в истории обработки.",
  "memory.conflict": "Статья изменилась во время операции. Загрузите её заново и повторите попытку. Ваши изменения не сохранены.",
  "memory.duplicate": "Статья с таким заголовком уже существует. Откройте её, чтобы объединить содержимое.",
  "memory.notFound": "Эта статья, источник или задача больше не существует.",
  "memory.noTranscript": "Для этого сеанса нет доступной для чтения беседы.",
  "memory.agentUnavailable": "Выбранный агент недоступен. Проверьте путь к его исполняемому файлу в настройках.",
  "memory.invalid": "Некоторые поля или ссылки недействительны. Проверьте заголовок, содержимое и связанные статьи.",
  "memory.processFailed": "Агент не смог завершить обработку. Проверьте авторизацию, модель и настройки CLI, затем повторите попытку.",
  "memory.timeout": "Время ожидания агента истекло. Выберите доступную модель или более короткую беседу и повторите попытку.",
  "memory.interrupted": "Обработка прервана. Её можно повторить, используя сохранённый снимок источника.",
  "memory.tooLarge": "Источник, контекст или ответ превышает допустимый размер. Содержимое не обрезано и не сохранено.",
  "memory.invalidOutput": "Агент вернул неверные структурированные данные. Ничего не сохранено. Повторите попытку или выберите другого агента.",
  "memory.loadError": "Не удалось загрузить базу знаний. Проверьте соединение и повторите попытку.",
  "memory.unsaved": "Отменить несохранённые изменения?",
  "memory.source": "Снимок источника",

  // ── Common ──
  "common.cancel": "Отмена", // Cancel
  "common.confirm": "ОК", // OK
  "common.delete": "Удалить", // Delete
  "common.save": "Сохранить", // Save
  "common.create": "Создать", // Create
  "common.close": "Закрыть", // Close
  "chat.imageViewOriginal": "Открыть исходное изображение",
  "chat.imageCopy": "Копировать изображение",
  "chat.imageSave": "Сохранить изображение",
  "chat.imageActionFailed": "Не удалось выполнить операцию с изображением. Повторите попытку.",
  "common.copy": "Копировать", // Copy
  "common.cut": "Вырезать", // Cut
  "common.paste": "Вставить", // Paste
  "common.selectAll": "Выделить все", // Select All
  "common.copied": "Скопировано", // Copied
  "common.copyFailed": "Не удалось скопировать. Повторите попытку.",
  "chat.sync.loading": "Синхронизация переписки…",
  "chat.sync.failed": "Не удалось синхронизировать. Загруженные сообщения по-прежнему доступны.",
  "chat.sync.history": "Загрузить более ранние сообщения",
  "chat.submission.updateRequired": "Обновите сервер, прежде чем отправлять сообщения из этого клиента.",
  "chat.submission.sending": "Отправка…",
  "chat.submission.sent": "Отправлено",
  "chat.submission.queued": "В очереди",
  "chat.submission.failed": "Ошибка отправки",
  "chat.submission.unknown": "Доставка не подтверждена",
  "chat.submission.check": "Проверить статус",
  "common.retry": "Повторить", // Retry
  "common.experimental": "Экспериментальная функция",
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
  "titlebar.gameCenter": "Игровой центр",
  "titlebar.browser": "Встроенный браузер", // Built-in Browser
  "titlebar.remoteAccess": "Удалённый доступ (браузер)", // Remote Access (Browser)
  "titlebar.connectRemote": "Подключиться к удалённому серверу", // Connect to Remote Server
  "titlebar.mirrored": "Зеркало", // Mirrored
  "titlebar.mirroredHint":
    "Зеркалирование включено: вкладки, разделения и активная сессия следуют за хостом. Переключатель находится на хосте.", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `Зеркалят: ${n}`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `Подключено удалённых клиентов: ${n}. Вкладки, разделения и активная сессия общие, менять их может любая сторона.`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "Подключённые клиенты", // Attached clients
  "titlebar.clientUnnamed": "Клиент без имени", // Unnamed client
  "titlebar.clientSince": (time: string) => `с ${time}`, // since {time}
  "titlebar.feedback": "Обратная связь", // Feedback
  "titlebar.share": "Поделиться", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "Файл", // File
  "menubar.terminal": "Терминал", // Terminal
  "menubar.help": "Справка", // Help
  "menubar.newTerminal": "Новый терминал", // New Terminal
  "menubar.visitWebsite": "Открыть сайт", // Visit Website
  "menubar.sendFeedback": "Отправить отзыв", // Send Feedback
  "menubar.clearBadges": "Очистить значки уведомлений", // Clear Notification Badges
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
  "settings.agentDefaultsTitle": "Настройки новых сеансов по умолчанию",
  "settings.referSummaryTitle": "Контекст ссылок на сеансы",
  "settings.referSummaryMode": "Режим контекста",
  "settings.referSummaryFull": "Полная запись",
  "settings.referSummaryFirst": "Сначала резюме",
  "settings.referSummaryAgent": "Агент для резюме",
  "settings.referSummaryHint":
    "По умолчанию vrefer --ask передаёт отвечающему агенту полную запись. Режим «Сначала резюме» сжимает её с помощью выбранных здесь агента, модели и уровня рассуждения; итоговый ответ также получает релевантные фрагменты исходного текста.",
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
  "settings.cliHint":
    "Добавляет `vela <путь-к-проекту>` в PATH, как команда `code` в VS Code.",
  "settings.agentArgsHint":
    "Аргументы запуска по умолчанию для новых сессий каждого типа агента. Аргументы, заданные для отдельной сессии при создании или редактировании, имеют приоритет. Оставьте пустым, чтобы не использовать.", // Agent default launch args hint
  "settings.agentPathLabel": "Путь к исполняемому файлу (необязательно)", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "напр. ~/.local/bin/claude — пусто = искать в PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Если задан, сессии этого типа запускаются по этому полному пути вместо поиска команды в PATH. Полезно, когда агент установлен, но отсутствует в PATH оболочки. Заполняется автоматически после успешной установки в один клик, если место установки удалось определить.", // Agent executable path hint
  "settings.agentDefaultView": "Вид по умолчанию", // Default view
  "settings.agentDefaultViewHint":
    "Вид, в котором открываются новые сессии этого агента. Уже созданные сессии сохраняют вид, с которым были созданы.", // Agent default view hint
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
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.autoContinue": "Продолжать после сброса лимита", // Continue after limit resets
  "settings.autoContinueHint": "Если Claude или Codex останавливается из-за 5-часового или недельного лимита использования, задача автоматически продолжится после сброса лимита.", // When a 5-hour or weekly usage limit stops Claude or Codex, the task continues automatically after the limit resets.
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
  "spawn.title": "Запуск дочерней сессии",
  "spawn.fromSession": "Исходная сессия",
  "spawn.promptLabel": "Описание задачи",
  "spawn.agentLabel": "Тип сессии",
  "spawn.worktreeLabel": "Отдельное рабочее дерево",
  "spawn.modelLabel": "Модель",
  "spawn.effortLabel": "Уровень рассуждения",
  "spawn.modelDefault": "По умолчанию у агента",
  "spawn.modelLoading": "Загрузка моделей…",
  "spawn.modelListUnavailable": "Список недоступен. Можно ввести идентификатор вручную.",
  "spawn.launch": "Запустить сессию",
  "spawn.remaining": (n: number) => `Других запросов на проверку: ${n}`,
  "spawn.notifyTitle": "Дочерняя сессия ожидает подтверждения",
  "orch.title": "Запуск нескольких сессий",
  "orch.notifyTitle": "Запуск сессий ожидает подтверждения",
  "orch.coordinatorName": "Состояние сессий",
  "orch.sharedSettings": "Общие настройки",
  "orch.agentLabel": "Агент",
  "orch.modelLabel": "Модель",
  "orch.effortLabel": "Уровень рассуждения",
  "orch.nameLabel": "Название сессии",
  "orch.promptLabel": "Описание задачи",
  "orch.worktreeLabel": "Рабочее дерево Git",
  "orch.worktreeNone": "Текущий каталог",
  "orch.worktreeShared": "Общее рабочее дерево",
  "orch.worktreeEach": "Отдельное дерево для каждой сессии",
  "orch.follow": "Использовать общие настройки",
  "orch.overridden": "Индивидуальные настройки",
  "orch.remove": "Убрать задачу",
  "orch.launch": (n: number) => `Запустить сессии (${n})`,
  "orch.modelPlaceholder": "По умолчанию у агента",
  "orch.effortPlaceholder": "По умолчанию у агента",
  "launch.terminalHint": "Обычный терминал открывает рабочий каталог. Инструкции задачи не выполняются автоматически.",
  "launch.optionsError": "Не удалось загрузить параметры запуска. Повторите попытку перед запуском.",
  "launch.singleIntro": "Проверьте задачу и настройки перед запуском дочерней сессии.",
  "launch.taskHint": "Эти инструкции станут первым сообщением дочерней сессии.",
  "launch.runtime": "Параметры запуска",
  "launch.directory": "Рабочий каталог",
  "launch.directoryCurrentHint": "Сессии изменяют файлы в исходном каталоге.",
  "launch.directorySharedHint": "Все сессии используют один новый каталог и одну ветку.",
  "launch.directoryEachHint": "Каждая сессия получает собственный каталог и ветку.",
  "launch.worktreeHint": "Рабочие деревья создаются из текущего коммита без незакоммиченных изменений. При ошибке создания используется исходный каталог.",
  "launch.singleResult": "Дочерняя сессия появится под исходной сессией на боковой панели.",
  "launch.startError": "Не удалось запустить сессии. Проверьте настройки и повторите попытку.",
  "launch.starting": "Запуск…",
  "launch.batchIntro": "Проверьте общие настройки, затем выберите каждую задачу для редактирования инструкций.",
  "launch.sessionCount": (n: number) => `Сессий: ${n}`,
  "launch.batchName": "Название группы задач",
  "launch.sharedHint": "Применяется к сессиям без индивидуальных настроек.",
  "launch.tasks": "Задачи",
  "launch.incomplete": "Нужно дополнить",
  "launch.undoRemove": "Отменить удаление",
  "launch.taskNumber": (n: number) => `Задача ${n}`,
  "launch.taskSettings": "Настройки этой сессии",
  "launch.taskAgent": "Агент этой сессии",
  "launch.sharedDirectoryLocked": "Все сессии этой группы используют одно общее рабочее дерево.",
  "launch.resetSettings": "Восстановить общие настройки",
  "launch.monitorHint": "Терминал «Состояние сессий» покажет, какие сессии работают или ожидают ввода. Он не показывает процент выполнения задач.",
  "launch.taskIncomplete": (n: number) => `Заполните название и описание задачи ${n}.`,
  "launch.batchResult": "Каждая задача запускается в отдельной интерактивной сессии.",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Показать изменения…",
  "changes.title": "Изменения",
  "changes.loading": "Загрузка…",
  "changes.loadingDiff": "Загрузка diff…",
  "changes.noChanges": "Нет изменений",
  "changes.refresh": "Обновить",
  "changes.notRepo": "Не git-репозиторий",
  "changes.selectFile": "Выберите файл",
  "changes.binary": "Двоичный файл — построчный diff недоступен",
  "changes.commitTitle": (hash: string) => `Коммит ${hash}`,

  "git.staged": "В индексе",
  "git.changes": "Изменения",
  "git.untracked": "Неотслеживаемые файлы",
  "git.committed": "Закоммиченные изменения",
  "git.stage": "Добавить в индекс",
  "git.unstage": "Убрать из индекса",
  "git.stageAll": "Добавить всё",
  "git.unstageAll": "Убрать всё",
  "git.discard": "Отменить изменения",
  "git.deleteFile": "Удалить",
  "git.viewAll": "Показать всё",
  "git.detached": "(отсоединённый HEAD)",
  "git.aheadBehind": "Коммиты впереди и позади вышестоящей ветки",
  "git.commitPlaceholder": "Сообщение коммита",
  "git.amend": "Изменить последний коммит",
  "git.amendCommit": "Изменить коммит",
  "git.commitCount": (n: number) => `Закоммитить файлов: ${n}`,
  "git.commitNoFiles": "В этом коммите нет изменений файлов",
  "git.noCommits": "Коммитов пока нет",
  "git.loadMore": "Загрузить ещё",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody":
    "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
  "tree.moveGroupToWorktree": "Переместить в worktree…",
  "tree.convertToNormalGroup": "Convert to normal group", // TODO translate
  "merge.title": "Merge branches", // TODO translate
  "merge.desc":
    "Pick a source and a target branch; the source merges into the target.", // TODO translate
  "merge.notRepo": "This session's directory is not a git repository.", // TODO translate
  "merge.loadingBranches": "Loading branches…", // TODO translate
  "merge.loadingDiff": "Loading diff…", // TODO translate
  "merge.sourceLabel": "Source branch", // TODO translate
  "merge.targetLabel": "Target branch", // TODO translate
  "merge.selectBranch": "Select a branch…", // TODO translate
  "merge.swap": "Swap direction", // TODO translate
  "merge.pickHint":
    "Pick both branches to preview the changes this merge brings in.", // TODO translate
  "merge.changes": (target: string) => `Changes brought into "${target}"`, // TODO translate
  "merge.noChanges": "No file changes.", // TODO translate
  "merge.sameBranch": "Source and target are the same branch.", // TODO translate
  "merge.branchGone": "A selected branch no longer exists. Pick again.", // TODO translate
  "merge.upToDate":
    "The target branch already contains the source branch. Nothing to merge.", // TODO translate
  "merge.targetNotCheckedOut": (target: string) =>
    `Target branch "${target}" isn't checked out in any worktree, so a local merge can't run. Check it out first.`, // TODO translate
  "merge.targetDirty":
    "The target branch's working tree has uncommitted changes; the merge may be blocked.", // TODO translate
  "merge.sourceDirtyNote":
    "The source branch's working tree has uncommitted changes; they will be committed first.", // TODO translate
  "merge.commitMsgLabel": "Commit message", // TODO translate
  "merge.commitMsgPlaceholder":
    "Describe this change (used as the commit message)", // TODO translate
  "merge.apply": "Merge", // TODO translate
  "merge.commitAndApply": "Commit & merge", // TODO translate
  "merge.working": "Merging…", // TODO translate
  "merge.doneMsg": (source: string, target: string) =>
    `Merged "${source}" into "${target}".`, // TODO translate
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
  "settings.termLineHeight": "Высота строки в терминале",
  "settings.chatTypography": "Просмотр диалога",
  "settings.chatTypographyHint": "Эти настройки шрифта не зависят от терминала и применяются сразу.",
  "settings.chatFont": "Шрифт диалога",
  "settings.chatFontSize": "Размер шрифта диалога",
  "settings.chatLineHeight": "Высота строки диалога",
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontListUnavailable": "Не удалось получить список системных шрифтов. Название шрифта можно ввести вручную.",
  "settings.fontUnconfirmed": "Не удалось подтвердить доступность этого шрифта.",
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
  "settings.notifyUnsupported":
    "Notifications aren't available in this environment.", // TODO translate
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
  "settings.scHint":
    "Нажмите на сочетание, затем нажмите новую комбинацию (нужен Cmd/Ctrl).", // hint
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
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "Автоматически (первый LAN-адрес)", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "Отсканируйте телефоном, чтобы открыть ссылку для сопряжения по выбранному адресу.", // Scan with your phone to open the pairing link on the selected address.
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
  "remote.autoRestartHint":
    "Удалённый доступ автоматически возобновляется при повторном открытии приложения. «Остановить сервер» отключает это.", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "Сбой автоматического запуска:", // Automatic start failed:
  "remote.mirror": "Зеркалировать раскладку на всех устройствах", // Mirror layout across devices
  "remote.mirrorHint":
    "Вкладки, разделения и активная сессия одинаковы на всех подключённых устройствах. Фокус клавиатуры на каждом остаётся на месте.", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

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
  "connect.sshFingerprintLabel": (kt: string) =>
    `Отпечаток ключа хоста SSH (${kt})`,
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
  "connect.showPassword": "Показать пароль",
  "connect.hidePassword": "Скрыть пароль",
  "connect.urlPasswordPlaceholder": "Пароль для входа",
  "connect.mirror": "Зеркалировать удалённое настольное приложение", // Mirror the remote desktop app
  "connect.mirrorHint":
    "Вкладки, разделения и активная сессия совпадают с настольным приложением на удалённой машине; изменения с любой стороны видны на обеих. Если настольное приложение не запущено, это подключение открывает его базу данных напрямую, а при её отсутствии — отдельную базу данных.", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb":
    "Использовать базу данных настольного приложения на удалённой машине",
  "connect.shareDesktopDbHint":
    "Общая база данных с настольным приложением на удалённой машине (лучше, когда версии совпадают). Выкл. = отдельная база данных.",

  // ── Sidebar ──
  "tree.newSession": "Новая сессия", // New Session
  "tree.newTerminalSession": "Новый терминал", // New Terminal
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
  "tree.collectionInfo": "Сведения о коллекции", // Collection Info
  "tree.projectInfo": "Сведения о проекте", // Project Info
  "info.branch": "Ветка", // Branch
  "info.path": "Путь", // Path
  "info.recentCommits": "Последние коммиты", // Recent Commits
  "info.noCommits": "Нет коммитов", // No commits
  "tree.killProcess": "Завершить процесс", // Kill Process
  "tree.killProcessConfirm": (name: string) => `Завершить процесс сеанса «${name}»? Текущая задача будет прервана. Сохранённая история переписки и файлы останутся.`,
  "tree.archiveSession": "Архивировать сессию", // Archive Session
  "tree.archiveGroup": "Архивировать группу", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "врем.", // scratch
  "tree.persistSession": "Сделать постоянной сессией…", // Make Permanent Session…
  "tree.persistDoc": "Сохранить на диск…", // Save to Disk…
  "tree.closeScratch": "Закрыть черновик", // Close Scratch
  "tree.importProject": "Импортировать проект", // Import Project
  "tree.createProject": "Создать проект",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "Новая коллекция",
  "tree.deleteCollection": "Удалить коллекцию",
  "collection.title": "Новая коллекция",
  "collection.name": "Название коллекции",
  "collection.namePlaceholder": "research",
  "collection.submit": "Создать коллекцию",
  "collection.tag": "Без папки",
  "collection.deleteTitle": "Удалить коллекцию",
  "collection.deleteBody": (name) =>
    `Удалить коллекцию «${name}»? Все её группы и сессии тоже будут удалены. Это действие нельзя отменить.`,
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
  "clone.slowHint":
    "Нет прогресса в течение 30 секунд. Проверьте сеть или прокси удалённого компьютера; можно отменить и повторить попытку.",
  "clone.submit": "Клонировать", // Clone
  "tree.globalSearch": "Искать во всех сессиях", // Search All Sessions
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
  "tree.noProjectsPre":
    "Проектов пока нет. Нажмите на значок папки или клавиши ", // No projects yet. Click the folder button, or press
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
  "tree.engineLabel": "Открывается в",
  "tree.engineTui": "Вид терминала",
  "tree.engineChat": "Вид беседы",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "Агент работает в собственном терминальном интерфейсе.",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "Представление в виде сообщений и карточек инструментов; запросы разрешений обрабатываются в интерфейсе.",
  "tree.agentArgsLabel": "Аргументы запуска (необязательно)", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "Рабочий каталог",
  "tree.workingDirPlaceholder": "Оставьте пустым для каталога по умолчанию",
  "preset.execPathLabel": "Исполняемый файл (необязательно)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "Оставьте пустым, чтобы использовать команду, настроенную для агента. Укажите путь, и только этот сеанс запустится с совместимой заменой.",
  "preset.saveLabel": "Сохранить как пресет",
  "preset.namePlaceholder": "Название пресета",
  "preset.iconChoose": "Выбрать значок",
  "preset.iconClear": "Убрать",
  "preset.iconHint":
    "Лучше всего подходят квадратные изображения; остальные обрезаются и масштабируются до 64x64.",
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
  "info.projectId": "ID проекта", // Project ID
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
  "importSessions.results": ({ count }: { count: number }) => `Результатов: ${count}`,
  "importSessions.selected": ({ count }: { count: number }) => `Выбрано: ${count}`,
  "importSessions.clearSelection": "Снять выделение",
  "importSessions.clearSearch": "Очистить поиск",
  "importSessions.noHistory": "Для каталога этого проекта не найдено предыдущих сессий.",
  "importSessions.title": "Импорт сессий",
  "importSessions.description": "Найдите существующие сессии Codex, Claude и OpenCode, рабочий каталог которых совпадает с каталогом проекта. Выберите сессии для добавления в проект, затем откройте нужную сессию, чтобы продолжить разговор.",
  "importSessions.search": "Поиск по названию, агенту или ID сессии",
  "importSessions.empty": "Подходящие сессии не найдены.",
  "importSessions.imported": "Уже импортирована",
  "importSessions.confirm": ({ count }: { count: number }) => `Импортировать (${count})`,
  "importSessions.success": ({ count }: { count: number }) => `Количество сессий, добавленных в проект: ${count}.`,
  "resume.title": "Возобновить сессию", // Resume Session
  "resume.desc":
    "Выберите тип агента и введите собственный session id агента; при открытии продолжится исходный диалог.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Тип агента", // Agent type
  "resume.sessionIdPlaceholder": "Session id диалога", // Conversation session id
  "resume.confirm": "Возобновить и открыть", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Новая сессия worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Имя worktree", // Worktree name
  "worktree.worktreeNameHint":
    "Используется как имя каталога worktree и ветки.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Не удалось создать worktree", // Couldn't create the worktree
  "worktree.noRepoRoot":
    "У этого проекта нет пригодного пути к git-репозиторию.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Без", // None
  "worktreeSel.modeNew": "Новый", // New
  "worktreeSel.modeExisting": "Существующий", // Existing
  "worktreeSel.loading": "Загрузка worktree…", // Loading worktrees…
  "worktreeSel.empty": "В этом репозитории нет существующих worktree.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "Не удалось получить список worktree (не git-репозиторий?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "Сессии, созданные в этой группе, по умолчанию будут использовать это worktree.", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "Переместить группу в worktree",
  "worktree.moveGroupHint":
    "Сессии, созданные в этой группе с этого момента, будут использовать этот worktree. Уже существующие сохранят свой каталог.",

  // ── Archive panel ──
  "archive.title": "Архив сессий", // Archived Sessions
  "archive.empty1": "Архивных сессий нет.", // No archived sessions.
  "archive.empty2":
    "Щёлкните сессию в боковой панели правой кнопкой и выберите «Архивировать сессию», чтобы убрать её сюда.", // Right-click a session in the sidebar…
  "archive.restore": "Восстановить как обычную сессию", // Restore to normal session
  "archive.export": "Экспортировать полный контекст в Markdown", // Export full context as Markdown
  "archive.deleteForever": "Удалить навсегда (вместе с записью)", // Delete permanently (with recording)
  "archive.pickOne":
    "Выберите архивную сессию слева, чтобы посмотреть стенограмму", // Select an archived session on the left…
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
  "search.hint":
    "Поиск по содержимому сессий. Архивные по умолчанию исключены — отметьте «Включая архив», чтобы добавить их.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Включая архив", // Include archived
  "search.includeArchivedHint":
    "Искать также в архивных сессиях (по умолчанию выкл.)", // Also search archived sessions (off by default)
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
  "tab.bgTitle": (n) => `Фоновые вкладки: ${n} (процессы продолжают работать)`, // Background keep-alive tabs: {n}…
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
  "browser.desktopOnly":
    "Вкладки браузера открываются только в настольном приложении.", // Browser tabs open in the desktop app only.
  "browser.stop": "Остановить загрузку", // Stop loading
  "browser.openExternal": "Открыть в системном браузере", // Open in system browser
  "browser.addressPlaceholder": "Введите URL или поисковый запрос", // Enter URL or search terms
  "browser.quickAccess": "Быстрый доступ", // Quick access
  "browser.loading": "Загрузка…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "Закрыть VelaTerm?", // Quit VelaTerm?
  "quit.body": "Все запущенные сеансы терминала и агента будут остановлены.", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Сохранить рабочее пространство", // Save workspace
  "quit.saveWorkspaceHint":
    "В следующий раз откроются те же вкладки и разделения. Терминалы восстанавливаются, но не запускаются заново.", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Закрыть", // Quit
  "dormant.body":
    "Восстановлено из сохранённого рабочего пространства. Процесс ещё не запущен.", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Запустить", // Start
  "overlimit.title": (max) => `Превышен лимит фоновых вкладок (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
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
  "term.mirrorBadge": (dims) =>
    `⤢ Зеркало${dims} · нажмите, чтобы подогнать под окно`, // ⤢ Mirror{dims} · click to fit this window
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
  "agentInstall.pathSaved": (label: string) =>
    `Путь к исполняемому файлу ${label} сохранён в настройках:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} установлен`, // {label} is installed
  "agentInstall.doneDesc": "Перезапустите эту сессию, чтобы начать работу.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Перезапустить сейчас", // Relaunch now
  "agentInstall.later": "Позже", // Later
  "agentInstall.pathLabel": "Путь к исполняемому файлу", // Executable path
  "agentInstall.pathPlaceholder": (bin: string) => `~/.local/bin/${bin}`,
  "agentInstall.pathHint": "Установлено вне PATH? Укажите полный путь к исполняемому файлу.", // Already installed outside PATH?
  "agentInstall.pathSave": "Использовать этот путь", // Use this path
  "agentInstall.pathBrowse": "Обзор…", // Browse…
  "search.placeholder": "Поиск в терминале", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "Визуальный", // WYSIWYG
  "doc.visual": "Визуальный",
  "doc.source": "Исходный код",
  "doc.compare": "Сравнение",
  "doc.editorLoadFailed": "Не удалось загрузить редактор Markdown.",
  "doc.imageOnly": "Здесь можно вставлять только файлы изображений.",
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
  "doc.overwriteConfirm":
    "Файл с таким именем уже существует. Нажмите «Перезаписать», чтобы заменить его.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Сохранить", // Save
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
  "doc.imgDecodeFailed":
    "Не удаётся отобразить это изображение (неподдерживаемый или повреждённый формат).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Вписать", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Экспорт в PDF", // Export PDF
  "doc.diagramError": "Ошибка диаграммы", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Сессия не выбрана", // No session selected
  "panel.collapseSection": "Свернуть раздел", // Collapse section
  "panel.expandSection": "Развернуть раздел", // Collapse section
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
  "files.dblClickOpen": "Двойной клик, чтобы открыть",
  "files.deleteConfirm": (name) =>
    `Удалить «${name}»? Это действие нельзя отменить.`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "Загрузки на сервер", // Uploads
  "transfer.download": "Скачать", // Download
  "transfer.upload": "Загрузить файлы…", // Upload Files…
  "transfer.uploadTooltip": "Загрузить файлы в эту папку", // Upload files to this folder
  "transfer.clear": "Очистить", // Clear
  "transfer.cancelled": "Отменено", // Cancelled
  "transfer.failed": "Ошибка", // Failed
  "transfer.stalled": "Переподключение…", // Reconnecting…
  "transfer.foldersUnsupported": "Папки загрузить нельзя.", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) =>
    `${n} ${plural(n, "сессия", "сессии", "сессий")}`, // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Нажмите, чтобы показать в боковой панели только сессии «${label}» (нажмите ещё раз, чтобы сбросить)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Фон ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Фоновые вкладки (лимит ${max}; при превышении автоматически завершается самая старая неактивная)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `Фоновая вкладка завершена: ${name} (превышен лимит)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) =>
    `Удалённый доступ через браузер включён: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Права: спрашивать", // Perms: Ask
  "statusbar.permSkip": "Права: пропускать", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "Режим прав этой сессии · нажмите, чтобы изменить (только эта сессия)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Права этой сессии", // This session's permissions
  "statusbar.permOptAsk": "Спрашивать каждый раз (по умолчанию)", // Ask each time (default)
  "statusbar.permScopeHint":
    "Применяется только к этой сессии. Для глобальных настроек перейдите в Настройки ▸ Агенты.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "Права изменены. Чтобы применить, нужно перезапустить сессию. Перезапуск продолжит текущий диалог, но прервёт выполняемую задачу. Перезапустить сейчас?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Перезапустить", // Restart now
  "statusbar.permRestartLater": "Позже", // Later
  "statusbar.permScopeTitle": "Применить к?", // Apply to?
  "statusbar.permScopeSession": "Только эта сессия", // This session only
  "statusbar.permScopeGlobal": "Глобально по умолчанию", // Global default
  "statusbar.permScopeGlobalHint":
    "Применяется сейчас к этой сессии и становится значением по умолчанию для будущих новых сессий этого типа (синхронизировано с настройками).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

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
  "transport.imgUploadHttp": (status) =>
    `Не удалось загрузить изображение (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.showPassword": "Показать",
  "login.hidePassword": "Скрыть",
  "login.passwordSaveFailed": "Соединение установлено, но не удалось сохранить пароль на этом устройстве. Повторите попытку.",
  "login.connecting": "Подключение…", // Connecting…
  "login.remoteAccess": "Удалённый доступ", // Remote Access
  "login.desc": "Введите пароль доступа, чтобы подключиться к этому терминалу.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Пароль доступа", // Access password
  "login.connect": "Подключиться", // Connect
  "login.wrongPassword": "Неверный пароль", // Wrong password
  "login.rateLimited":
    "Слишком много попыток. Подождите минуту и попробуйте снова.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Не удалось войти, попробуйте ещё раз", // Login failed, please try again
  "login.pairingRequired":
    "Этот сервер требует ссылку для сопряжения. Откройте ссылку, созданную в панели «Удалённый доступ» настольного приложения.", // This server requires a pairing link
  "login.authFailed":
    "Ошибка аутентификации. Проверьте пароль доступа или откройте новую ссылку для сопряжения, если её создали заново.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Выбор каталога проекта", // Choose Project Directory
  "dir.pathPlaceholder":
    "Поиск или введите путь и нажмите Enter (поддерживается ~)", // Search, or type a path and press Enter (supports ~)
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
  "conn.sshDown":
    "SSH-соединение разорвано — нажмите «Переподключиться сейчас», чтобы повторить", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Ошибка запроса", // Request failed
  "reqerr.dismiss": "Закрыть", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Журнал ошибок", // Error Log
  "errlog.empty": "Нет записанных ошибок.", // No errors recorded.
  "errlog.copyAll": "Копировать всё", // Copy all
  "errlog.clear": "Очистить", // Clear
  "errlog.close": "Закрыть", // Close

  // ── Mobile ──
  "mobile.backConnections": "К списку подключений",
  "mobile.loadSlow": "Загрузка занимает больше времени, чем ожидалось. Можно повторить попытку или вернуться к списку подключений.",
  "mobile.connectionUnavailable": "Подключение недоступно",
  "mobile.pushTitle": "Уведомления о задачах",
  "mobile.pushHint": "Уведомления показывают название сеанса и краткий фрагмент ответа, в том числе в фоновом режиме и при заблокированном экране. Этот текст передаётся на velaterm.com и в сервис push-уведомлений. Пароли подключения и закрытые ключи SSH не отправляются.",
  "mobile.pushEnable": "Включить уведомления",
  "mobile.pushDisable": "Выключить уведомления",
  "mobile.pushTest": "Отправить тестовое уведомление",
  "mobile.pushTestSent": "Тестовое уведомление добавлено в очередь. Проверьте центр уведомлений системы.",
  "mobile.pushDisabled": "Фоновые уведомления выключены.",
  "mobile.pushEnabled": "Фоновые уведомления включены.",
  "mobile.pushNotConfigured": "В этой сборке не настроен сервис push-уведомлений.",
  "mobile.pushDenied": "Разрешите уведомления в настройках системы.",
  "mobile.pushRegistrationFailed": "Не удалось зарегистрировать устройство. Повторите попытку.",
  "mobile.pushRelayUnavailable": "Сервис доставки уведомлений недоступен. Повторите попытку.",
  "mobile.pushHostUnavailable": "На удалённом хосте ещё не включены фоновые уведомления. Обновите хост и подключитесь заново.",
  "mobile.pushDisclosure": "Для фоновых уведомлений используются Getui и push-сервис производителя устройства. Для доставки они обрабатывают идентификаторы устройства, сведения о сети, названия сеансов и краткие фрагменты ответов. Пароли подключения и закрытые ключи SSH не отправляются.",
  "mobile.pushConnectHint": "После включения уведомлений откройте каждое нужное подключение один раз, чтобы оформить подписку.",
  "mobile.pushTarget": "Подключение для проверки",
  "mobile.copyConnection": "Копировать и изменить",
  "mobile.copyConnectionHint": "Измените настройки на основе этого подключения. Сохранённые учётные данные будут безопасно перенесены. Исходное подключение не изменится; при совпадении настроек будет использовано существующее подключение.",
  "mobile.copyConnectionReused": "Эти настройки уже сохранены. Существующее подключение оставлено без изменений.",
  "mobile.inputOptions": "Параметры сообщения",
  "mobile.connections": "Управление подключениями",
  "mobile.more": "Другие действия",
  "mobile.toDesktop": "Перейти к версии для ПК", // Switch to desktop
  "mobile.empty1": "Сессий нет.", // No sessions.
  "mobile.noMatch": "Нет подходящих сессий", // No matching sessions
  "mobile.empty2":
    "Создайте сессию в настольном приложении или в браузере компьютера — она появится здесь автоматически.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Назад", // ‹ Back
  "mobile.selCopy": "Копировать", // Copy
  "mobile.selCancel": "Отмена", // Cancel

  // ── Mobile connection client (apps/mobile start page) ──
  "mobile.phaseConnecting": "Connecting over SSH…", // TODO translate
  "mobile.phaseConfirming": "Confirm the host fingerprint", // TODO translate
  "mobile.phasePreparing": "Checking or preparing the remote service…", // TODO translate
  "mobile.phaseForwarding": "Opening the SSH tunnel…", // TODO translate
  "mobile.phaseReady": "Connected", // TODO translate
  "mobile.phaseDisconnected": "Disconnected", // TODO translate
  "mobile.phaseError": "Connection failed", // TODO translate
  "mobile.accountAndLogin": "Account and sign-in", // TODO translate
  "mobile.connectionService": "Connection service unavailable", // TODO translate
  "mobile.nativeOnly": "Connecting is only available in the iOS or Android app. The browser is only for previewing the interface.", // TODO translate
  "mobile.managedRemotely": "Projects and sessions are managed by the remote service.", // TODO translate
  "mobile.buildInfo": (version: string, time: string) => `App v${version} · Built ${time}`, // TODO translate
  "mobile.myDevices": "My devices", // TODO translate
  "mobile.account": "Account", // TODO translate
  "mobile.signedInHint": "Signed in. You can view the workspaces, projects, and sessions shared by devices on this account.", // TODO translate
  "mobile.manageAccount": "Manage account", // TODO translate
  "mobile.signOut": "Sign out", // TODO translate
  "mobile.viewMyDevices": "View my devices", // TODO translate
  "mobile.noDevices": "No devices are signed in to this account yet.", // TODO translate
  "mobile.online": "Online", // TODO translate
  "mobile.offline": "Offline", // TODO translate
  "mobile.deviceNotSharing": "The device is not sharing anything yet.", // TODO translate
  "mobile.scopeMachine": "Entire workspace", // TODO translate
  "mobile.scopeProject": "Project", // TODO translate
  "mobile.scopeSession": "Session", // TODO translate
  "mobile.sharingNotReady": "Shared content is not ready yet. Check the sharing settings on that device.", // TODO translate
  "mobile.deviceOffline": "The device is offline. Open VelaTerm on that device and keep it connected to the network.", // TODO translate
  "mobile.viewShared": "View shared content →", // TODO translate
  "mobile.devicesUnavailable": "Could not load the device list. Please try again.", // TODO translate
  "mobile.accountUnavailable": "Could not load the account status. Check your network and try again.", // TODO translate
  "mobile.signInTitle": "Sign in to VelaTerm", // TODO translate
  "mobile.signInHint": "Sign in with your email and password or a third-party account to see your devices and shared content.", // TODO translate
  "mobile.signIn": "Sign in", // TODO translate
  "mobile.checkSignIn": "Check sign-in status", // TODO translate
  "mobile.waitingSignIn": "Waiting for sign-in confirmation…", // TODO translate
  "mobile.workspaceTitle": "Your workspace", // TODO translate
  "mobile.workspaceHint": "Connect to a remote host and pick up where you left off.", // TODO translate
  "mobile.newSsh": "+ SSH connection", // TODO translate
  "mobile.newUrl": "+ URL connection", // TODO translate
  "mobile.remote": "My devices", // TODO translate
  "mobile.scanToConnect": "Scan QR code to connect", // TODO translate
  "mobile.noConnections": "No saved connections yet. Add an SSH or URL connection, or open Remote to see content shared by devices on your account.", // TODO translate
  "mobile.tapToConnect": "Tap to connect →", // TODO translate
  "mobile.webPasswordSaved": "Access password saved", // TODO translate
  "mobile.deleteConnectionTitle": "Delete connection", // TODO translate
  "mobile.deleteConnectionConfirm": (name: string) => `Delete “${name}” and its saved credentials? Remote projects are not deleted.`, // TODO translate
  "mobile.connectionMissing": "Connection not found", // TODO translate
  "mobile.editConnection": "Edit connection", // TODO translate
  "mobile.addSshHost": "Add SSH connection", // TODO translate
  "mobile.addUrlConnection": "Add URL connection", // TODO translate
  "mobile.connectionName": "Connection name", // TODO translate
  "mobile.serviceUrl": "Service address", // TODO translate
  "mobile.scanToFill": "Fill in from QR code", // TODO translate
  "mobile.openingCamera": "Opening the camera…", // TODO translate
  "mobile.scanCancelled": "Scan cancelled", // TODO translate
  "mobile.scanDone": "Service address detected. Check it, then save and connect.", // TODO translate
  "mobile.scanNativeOnly": "QR scanning is only available in the iOS or Android app.", // TODO translate
  "mobile.webPasswordOptional": "Access password (optional)", // TODO translate
  "mobile.keepPassword": "Leave empty to keep the current password", // TODO translate
  "mobile.webPasswordLater": "You can also enter it after connecting", // TODO translate
  "mobile.webPasswordSavedHint": "The access password is saved and used automatically when you reconnect. Leaving the field empty keeps the saved password.", // TODO translate
  "mobile.webPasswordStorageHint": "The password is kept in the phone’s secure storage. You can also choose to remember it when you enter it after connecting.", // TODO translate
  "mobile.sshHost": "SSH host", // TODO translate
  "mobile.sshHostPlaceholder": "Hostname or IP address", // TODO translate
  "mobile.sshPort": "SSH port", // TODO translate
  "mobile.username": "Username", // TODO translate
  "mobile.authMethod": "Authentication", // TODO translate
  "mobile.authPassword": "Password", // TODO translate
  "mobile.authKeyAndroid": "Private key (OpenSSH Ed25519 / RSA)", // TODO translate
  "mobile.authKey": "Private key (OpenSSH Ed25519)", // TODO translate
  "mobile.sshPassword": "SSH password", // TODO translate
  "mobile.privateKey": "Private key", // TODO translate
  "mobile.keepPrivateKey": "Leave empty to keep the saved private key", // TODO translate
  "mobile.pastePrivateKey": "Paste an OpenSSH private key", // TODO translate
  "mobile.passphraseOptional": "Key passphrase (optional)", // TODO translate
  "mobile.keepPassphrase": "Leave empty to keep the current passphrase", // TODO translate
  "mobile.sshSecretSavedHint": "SSH credentials are kept in the phone’s secure storage. Leave the fields empty while editing to keep them.", // TODO translate
  "mobile.remoteService": "Remote service", // TODO translate
  "mobile.serviceAuto": "Find the VelaTerm service automatically", // TODO translate
  "mobile.serviceManual": "Use an existing service port", // TODO translate
  "mobile.remotePort": "Remote loopback HTTP port", // TODO translate
  "mobile.webPasswordAutoHint": "The access password is saved and used automatically when you reconnect.", // TODO translate
  "mobile.prepareService": "Download and start the VelaTerm service when none is available", // TODO translate
  "mobile.prepareServiceHint": "Automatic preparation writes a signature-verified binary, configuration, and logs to ~/.velaterm/ on the remote host and keeps the service running. It needs Python 3 and an OpenSSL with Ed25519 support; reusing an existing service or specifying its port does not.", // TODO translate
  "mobile.saveConnection": "Save connection", // TODO translate
  "mobile.saveAndConnect": "Save and connect", // TODO translate
  "mobile.loginOpening": "Opening the sign-in page in your browser…", // TODO translate
  "mobile.loginFinishInBrowser": "Complete the sign-in in the browser window, then return to the app.", // TODO translate
  "mobile.loginChecking": "Checking sign-in status…", // TODO translate
  "mobile.loginSuccess": "Signed in.", // TODO translate
  "mobile.loginWaiting": "Waiting for sign-in confirmation. Your account and device list update automatically once sign-in completes.", // TODO translate
  "mobile.loginExpired": "The sign-in request has expired. Please sign in again.", // TODO translate
  "mobile.loginRetrying": "The account service is temporarily unreachable. Retrying. You do not need to sign in again.", // TODO translate

  // ── Other shared components ──
  "splitter.dragToResize": "Перетащите, чтобы изменить размер", // Drag to resize
  "transport.wsDisconnected": "WebSocket отключён", // WebSocket disconnected
  "transport.wsConnectFailed": "Не удалось подключиться по WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "Команда не выполнена", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `Команда недоступна для удалённых клиентов: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `Ключ настроек недоступен для записи удалёнными клиентами: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `Удалённые клиенты не могут обращаться к файлам в каталоге данных приложения: ${path}`, // Remote clients cannot access files in the app data directory

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
  "info.collection": "Коллекция", // Collection
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
  "updater.hideHint":
    "Keep downloading in the background. Progress stays in the status bar.", // TODO translate
  "updater.downloadManually": "Download manually", // TODO translate
  "updater.downloadManuallyHint": "Open the download page in your browser.", // TODO translate
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

  // ── Вид беседы (сессия агента, прочитанная как разговор) ──
  "session.showConversation": "Вид беседы",
  "session.showTerminal": "Вид терминала",
  "session.switchTitle": "Смена вида перезапускает агента",
  "session.switchBody": "Текущий ход будет прерван. Разговор сохранится.",
  "session.switchConfirm": "Переключить",
  "session.terminalViewHint": "Нажмите здесь, чтобы вернуться к виду терминала.",
  "session.loading": "Читаем беседу…",
  "session.unavailable": "Эту беседу пока не удаётся прочитать",
  "session.working": "Работает…",
  "session.thinking": "Рассуждение",
  "session.toolRunning": "выполняется",
  "session.toolUnknown": "Инструмент",
  "session.toolFailed": "Не удалось",
  "session.toolNoDetail": "Больше ничего не записано",
  "session.showMore": (n: number) => `Показать ещё ${n} символов`,
  "session.showLess": "Свернуть",
  "session.composerHint": "Сообщение агенту · Enter отправляет, Shift+Enter переносит строку",
  "session.send": "Отправить",

  // ── Движок беседы (сессия, управляемая по протоколу) ──
  "chat.empty": "Введите сообщение в поле ниже, чтобы начать разговор.",
  "chat.interrupt": "Остановить",
  "chat.interruptTooltip": "Остановить · Esc",
  "chat.allow": "Разрешить",
  "chat.deny": "Отклонить",
  "chat.permissionAsk": (tool: string) => `${tool} просит разрешения на запуск`,
  "chat.exited": (code: number) => `Агент завершился (код ${code})`,
  "chat.modeNextTurn": "Со следующего хода",
  "chat.modePendingHint": (current: string, next: string) =>
    `Текущие разрешения: ${current}. Режим ${next} будет применён со следующего хода; текущий ход продолжится без изменений.`,
  "chat.modeTooltip": "Режим разрешений",
  "chat.collaborationModeTooltip": "Режим взаимодействия",
  "chat.collaborationMode.default": "Обычный",
  "chat.collaborationMode.defaultHint":
    "Сразу выполняет задачу и задаёт вопросы только при необходимости принять решение",
  "chat.collaborationMode.plan": "Планирование",
  "chat.collaborationMode.planHint":
    "Сначала изучает задачу и составляет план; вопросы могут отображаться как интерактивные карточки",
  "chat.moreOptions": "Ещё",
  "chat.modelTooltip": "Модель",
  "chat.keepChoice": "По умолчанию",
  "chat.keepChoiceFor": (model) => `По умолчанию для ${model}`,
  "chat.followModelDefault": (agent: string) => `Использовать модель по умолчанию ${agent}`,
  "chat.followModelDefaultHint": "Модель определяется настройками агента.",
  "chat.savedModelDefault": "По умолчанию в приложении",
  "chat.catalogWebsite": "Каталог моделей с сайта",
  "chat.catalogCache": "Каталог моделей из кеша",
  "chat.catalogBundled": "Встроенный каталог моделей",
  "chat.catalogChecked": (time: string) => `Последняя проверка: ${time}`,
  "chat.catalogFailed": "Не удалось обновить каталог. Предыдущий каталог остаётся доступным.",
  "chat.catalogRefresh": "Обновить",
  "chat.modelDefault": "Модель по умолчанию",
  "chat.mode.default": "Всегда спрашивать",
  "chat.mode.agentDefault": "По умолчанию агента",
  "chat.mode.acceptEdits": "Принимать правки",
  "chat.mode.plan": "Режим плана",
  "chat.permissionRestart.unconfirmed": "Соединение потеряно. Не удалось подтвердить изменение разрешений. Подключитесь снова, чтобы проверить текущие разрешения сеанса.",
  "permission.stateUnavailable": "Статус разрешений недоступен",
  "permission.currentUnknown": "Текущие разрешения не подтверждены",
  "permission.notRunning": "Не запущено",
  "permission.applied": "Применено",
  "permission.nextTurn": "Применится к следующему сообщению",
  "permission.restart": "Применится после перезапуска этой сессии",
  "permission.nextStart": "При следующем запуске",
  "permission.defaultHint": "Права по умолчанию для новых сеансов. Существующие сеансы сохраняют собственные настройки разрешений.",
  "chat.permissionRestart.title": "Перезапустить и отключить подтверждения?",
  "chat.permissionRestart.body": "Для отключения подтверждений нужно перезапустить Claude. Текущий ответ будет прерван, история разговора сохранится. После успешного переключения разрешения будут предоставляться без подтверждения.",
  "chat.permissionRestart.confirm": "Перезапустить и применить",
  "chat.permissionRestart.busy": "Перезапуск…",
  "chat.permissionRestart.failed": (detail: string) => "Не удалось изменить разрешения. Сохранён предыдущий режим. " + detail,
  "chat.permissionRestart.tasks": "Перед перезапуском обработайте или удалите сообщения из очереди и остановите фоновые задачи.",
  "chat.permissionRestart.stale": "Процесс сеанса изменился. Снова выберите «Без вопросов».",
  "chat.permissionRestart.noHistory": "Этот разговор пока нельзя возобновить. Дождитесь завершения инициализации и повторите попытку.",
  "chat.mode.bypassPermissions": "Без вопросов",
  "chat.mode.readOnly": "Только чтение",
  "chat.mode.fullAccess": "Полный доступ",
  "chat.placeholder": "Сообщение агенту, доступны /команды, /навыки и @файлы",
  "chat.command.clearDescription": "Архивировать эту сессию и начать новый диалог",
  "chat.command.rewindDescription": "Выбрать, что откатить от последнего сообщения пользователя",
  "chat.command.rewindUnavailable":
    "Для отката нужно завершённое сообщение пользователя; не должно быть активного хода, сообщений в очереди или запросов разрешений.",
  "chat.effortTooltip": "Глубина рассуждения",
  "chat.effortDefault": "Рассуждение",
  "chat.effort.auto": "Автоматически",
  "chat.effort.low": "Низкая",
  "chat.effort.medium": "Средняя",
  "chat.effort.high": "Высокая",
  "chat.effort.xhigh": "Очень высокая",
  "chat.effort.max": "Максимальная",
  "chat.effort.ultra": "Предельная",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "Агент",
  "chat.effort.minimal": "Минимальный",
  "chat.filterPlaceholder": "Фильтр",
  "chat.placeholderOpencode": "Напишите агенту; доступны /команды и @файлы, а сообщение, начинающееся с !, выполняется как команда оболочки",
  "chat.command.compactDescription": "Сжать беседу, чтобы освободить контекст",
  "chat.command.undoDescription": "Отменить последнее сообщение и вызванные им изменения файлов",
  "chat.command.redoDescription": "Вернуть то, что отменила последняя отмена",
  "chat.command.shareDescription": "Создать ссылку для доступа к этой беседе",
  "chat.command.unshareDescription": "Закрыть доступ к этой беседе",
  "chat.mode.auto": "Автоматически",

  // ── Вопрос агента, на который отвечают формой ──
  "chat.question.heading": "У агента есть вопрос",
  "chat.question.submit": "Отправить",
  "chat.question.next": "Далее",
  "chat.question.dismiss": "Закрыть",
  "chat.question.answerPlaceholder": "Введите ответ",
  "chat.question.otherPlaceholder": "Другой ответ",
  "chat.question.answeredHeading": (n: number) =>
    `Отвечено на ${n} ${plural(n, "вопрос", "вопроса", "вопросов")}`, // N questions answered
  "chat.question.blankAnswer": "Без ответа", // Left blank

  // ── План, ожидающий одобрения ──
  "chat.plan.heading": "План ожидает одобрения",
  "chat.plan.implement": "Одобрить и выполнить",
  "chat.plan.reject": "Отклонить",

  // ── Сообщения, написанные во время работы агента ──
  "chat.placeholderBusy": "Введите сообщение; оно будет отправлено по завершении текущего хода",
  "chat.queueTooltip": (combo: string) => `Будет отправлено по завершении хода · ${combo} — отправить сейчас`,
  "chat.queue.pending": "Сообщения в очереди",
  "chat.queue.view": "Показать сообщение полностью",
  "chat.queue.edit": "Изменить",
  "chat.queue.remove": "Удалить",

  // ── Изображения, вставленные или перетащенные в поле ввода ──
  "chat.attach.remove": "Удалить это изображение",
  "chat.attach.tooMany": (max: number) => `К одному сообщению можно приложить не более ${max} изображений`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} превышает ${mb} МБ и не был приложен`,
  "chat.attach.unreadable": (name: string) => `Не удалось прочитать ${name}`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "Сжимаем диалог…",
  "chat.compaction.manual": "Контекст сжат",
  "chat.compaction.auto": "Контекст сжат автоматически",
  "chat.compaction.from": (tokens: string) => `было ${tokens} токенов`,
  // N steps
  "chat.subagent.steps": (n: number) => {
    const tail = n % 100 >= 11 && n % 100 <= 14 ? 0 : n % 10;
    const word = tail === 1 ? "шаг" : tail >= 2 && tail <= 4 ? "шага" : "шагов";
    return `${n} ${word}`;
  },
  "chat.subagent.tokens": (tokens: string) => `${tokens} токенов`,
  "chat.rewind.edit": "Редактировать",
  "chat.rewind.editSend": "Проверить и отправить повторно",
  "chat.rewind.editConfirm": "Удалить и отправить повторно",
  "chat.rewind.editWarning": "Исходное сообщение и все последующие сообщения будут удалены без возможности восстановления. Изменённое сообщение будет отправлено с этого места. Изменения в файлах не будут отменены.",
  "chat.rewind.inactive": "Процесс диалога не запущен. Эти действия станут доступны после его запуска.",
  "chat.rewind.unsupported": "Подключённый агент пока не предоставляет это действие.",
  "chat.rewind.title": "Откатить отсюда",
  "chat.rewind.warning": "Это действие нельзя отменить.",
  "chat.rewind.conversation": "Откатить диалог",
  "chat.rewind.files": "Восстановить файлы",
  "chat.rewind.both": "Откатить диалог и восстановить файлы",
  "chat.rewind.confirm.conversation": "Удалить это сообщение и всё, что следует за ним?",
  "chat.rewind.confirm.files": "Восстановить файлы до состояния перед этим сообщением?",
  "chat.rewind.confirm.both": "Удалить этот ход и восстановить изменённые им файлы?",
  "chat.rewind.unavailable": "Для этого сообщения нет контрольной точки файлов.",
  "chat.rewind.previewing": "Проверка контрольной точки файлов…",
  "chat.rewind.cancel": "Оставить как есть",
  "chat.rewind.apply": "Откатить",
  "chat.rewind.applying": "Выполняется откат…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `Будет изменено файлов: ${files} (+${insertions} −${deletions}). Это действие нельзя отменить.`,
  // ── Постоянные правила, которые предлагает запрос разрешения; принимаются одним нажатием ──
  "chat.suggest.modeSession": (mode: string) => `${mode} в этой сессии`,
  "chat.suggest.mode": (mode: string) => `Переключить на «${mode}»`,
  "chat.suggest.allowSession": (rule: string) => `Разрешить ${rule} в этой сессии`,
  "chat.suggest.allowAlways": (rule: string) => `Всегда разрешать ${rule}`,
  "chat.suggest.dirSession": (dirs: string) => `Разрешить доступ к ${dirs} в этой сессии`,
  "chat.suggest.dirAlways": (dirs: string) => `Всегда разрешать доступ к ${dirs}`,
  // ── Codex: постоянные сетевые правила, вмешательство, собственные команды и чипы скорости и тона ──
  "chat.suggest.networkAlways": (host: string) => `Всегда разрешать сетевой доступ к ${host}`,
  "chat.steer": "Дополнить",
  "chat.stopping": "Остановка текущего хода…",
  "chat.stopped": "Текущий ход остановлен",
  "chat.steerAccepted": "Дополнительное указание отправлено",
  "chat.steerTooltip": (combo: string) => `${combo} — добавить к текущему ходу`,
  "chat.command.reviewDescription": "Проверить код и сообщить, что требует внимания",
  "chat.command.reviewHint": "[branch <имя> | commit <sha> | указания]",
  "chat.command.startTimeout": "Агент не открыл сеанс вовремя",
  "chat.serviceTierTooltip": "Скорость",
  "chat.serviceTier.default": "Обычная скорость",
  "chat.personalityTooltip": "Тон",
  "chat.personality.default": "Тон по умолчанию",
  "chat.personality.none": "Нейтральный",
  "chat.personality.friendly": "Дружелюбный",
  "chat.personality.pragmatic": "Прагматичный",
  // ── Длинный разговор: серия вызовов инструментов сворачивается в строку, плюс возврат в конец ──
  "chat.toolRun.count": (n: number) => `Вызовов инструментов: ${n}`,
  "chat.toolRun.tooltip": "Показать каждый вызов",
  "chat.backToEnd": "К последнему сообщению",
  "chat.turnFold.hide": "Скрыть шаги",
  "chat.turnFold.show": (n: number) => `Показать шаги (${n})`,
  "chat.turnFold.hideAll": "Скрыть все шаги",
  "chat.turnFold.showAll": "Показать все шаги",
  "chat.elicitation.heading": (server: string) => `${server} запрашивает данные`,
  "chat.elicitation.cancel": "Отмена",
  "chat.elicitation.decline": "Отклонить",
  "chat.elicitation.submit": "Отправить",
  "chat.elicitation.done": "Готово",
  "chat.elicitation.choose": "Выберите…",
  "chat.effort.off": "Выключено",
  "chat.effort.offHint": "Без расширенного размышления",
  "chat.fastMode.label": "Быстро",
  "chat.fastMode.on": "Быстрый режим включён",
  "chat.fastMode.off": "Быстрый режим выключен",
  "chat.auth.login": "Войти",
  "chat.auth.logout": "Выйти",
  "chat.auth.confirmLogout": "Подтвердить выход",
  "chat.auth.logoutConfirm": (provider: string) => `Выйти из ${provider} на этом хосте? Общие учётные данные будут удалены. Это повлияет на другие сеансы, которые их используют. История разговоров сохранится.`,
  "chat.auth.signingOut": "Выход из аккаунта…",
  "chat.auth.signedOut": (provider: string) => `Вы вышли из ${provider}. Войдите, чтобы продолжить этот разговор.`,
  "chat.auth.logoutFailed": "Не удалось подтвердить выход. Попробуйте ещё раз.",
  "chat.auth.wait": "Дождитесь завершения текущей задачи, прежде чем менять учётную запись.",
  "chat.auth.title": (provider: string) => `Учётная запись ${provider}`,
  "chat.auth.start": "Войти снова",
  "chat.auth.required": (provider: string) => `Авторизация в ${provider} больше не действительна. Войдите снова, чтобы продолжить.`,
  "chat.auth.starting": "Подготовка к входу…",
  "chat.auth.pending": "Откройте страницу авторизации и введите этот код. После завершения входа это представление обновится автоматически.",
  "chat.auth.success": "Вход выполнен. Отправьте сообщение, чтобы продолжить этот разговор.",
  "chat.auth.failed": "Не удалось завершить вход. Попробуйте ещё раз. Убедитесь, что в ChatGPT включена авторизация по коду устройства и ваша версия Codex CLI поддерживает эту функцию.",
  "chat.auth.canceled": "Вход отменён. Вы можете повторить попытку в любое время.",
  "chat.auth.scope": (provider: string) => `При входе обновится учётная запись ${provider}, используемая на этом хосте. Другие сеансы с теми же учётными данными также будут использовать эту запись.`,
  "chat.auth.canceling": "Отмена входа…",
  "chat.auth.submitting": "Проверка кода авторизации…",
  "chat.auth.claude.pending": "Откройте страницу авторизации, войдите в аккаунт и вставьте показанный код целиком.",
  "chat.auth.claude.failed": "Не удалось завершить вход. Повторите попытку и убедитесь, что ваша версия Claude CLI поддерживает авторизацию аккаунта.",
  "chat.auth.claude.code": "Код авторизации",
  "chat.auth.claude.submit": "Отправить код",
  "chat.auth.claude.invalidCode": "Вставьте полный код текущей попытки авторизации, включая часть после #.",
  "chat.auth.claude.externalAuth": "Ключи API и другие настроенные способы аутентификации не изменятся.",
  "chat.auth.open": "Открыть страницу авторизации",
  "chat.resetCredits.label": (n: string) => `Сбросов лимита: ${n}`,
  "chat.resetCredits.title": "Доступные сбросы лимитов Codex",
  "chat.resetCredits.unknown": "Не удалось получить количество доступных сбросов.",
  "chat.resetCredits.confirm": "Использовать один сброс для подходящих лимитов Codex. Это действие нельзя отменить.",
  "chat.resetCredits.reset": "Лимиты использования сброшены.",
  "chat.resetCredits.alreadyRedeemed": "Этот запрос уже выполнен успешно.",
  "chat.resetCredits.nothingToReset": "Нет лимитов, которые можно сбросить сейчас.",
  "chat.resetCredits.noCredit": "Доступных сбросов лимита нет.",
  "chat.resetCredits.error": "Запрос не выполнен или текущий остаток недоступен. Обновите остаток или повторите запрос на сброс, результат которого ещё не подтверждён.",
  "chat.resetCredits.busy": "Обработка…",
  "chat.resetCredits.retry": "Повторить сброс",
  "chat.resetCredits.use": "Использовать один сброс",
  "chat.resetCredits.refresh": "Обновить",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `Контекст: ${used} из ${max} токенов (${pct} %)`,
  "chat.usage.cost": (usd: string) => `Стоимость сеанса: $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `Лимит использования исчерпан; сброс ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) =>
    `Лимит использования: израсходовано ${pct} %; сброс ${resets}`,
  "chat.autoContinue.fiveHour": (time: string) => `Исчерпан 5-часовой лимит использования. Автоматическое продолжение задачи: ${time}.`, // 5-hour usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.weekly": (time: string) => `Исчерпан недельный лимит использования. Автоматическое продолжение задачи: ${time}.`, // Weekly usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.generic": (time: string) => `Лимит использования исчерпан. Автоматическое продолжение задачи: ${time}.`, // Usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.unknownReset": "Лимит использования исчерпан. Время сброса неизвестно, поэтому задача не продолжится автоматически.", // Usage limit reached. The reset time is unknown, so the task will not continue automatically.
  "chat.autoContinue.repeated": "Лимит использования снова исчерпан. Задача больше не будет продолжаться автоматически.", // The usage limit was reached again. The task will no longer continue automatically.
  "chat.autoContinue.failed": "Не удалось автоматически продолжить задачу. Отправьте сообщение, чтобы продолжить.", // The task could not continue automatically. Send a message to continue.
  "chat.mcp.codexScope": "Это изменит пользовательскую конфигурацию Codex и затронет другие беседы, использующие её. Продолжить?",
  "chat.mcp.tooltip": "Серверы MCP",
  "chat.mcp.loading": "Чтение списка серверов…",
  "chat.mcp.backendUnsupported": "Сервер VelaTerm, к которому установлено подключение, не поддерживает управление MCP. Обновите и перезапустите этот сервер, затем повторите попытку.",
  "chat.mcp.none": "Серверы MCP не настроены",
  "chat.mcp.tools": (n: number) => `Инструментов: ${n}`,
  "chat.mcp.reconnect": "Переподключить",
  "chat.mcp.disable": "Отключить",
  "chat.mcp.enable": "Включить",
  "chat.mcp.status.connected": "Подключён",
  "chat.mcp.status.disabled": "Отключён",
  "chat.mcp.status.failed": "Ошибка",
  "chat.mcp.status.pending": "Подключение",
  "chat.mcp.status.disconnected": "Соединение разорвано",
  "chat.mcp.status.other": "Неизвестно",
  "chat.tasks.label": "Задачи",
  "chat.tasks.tooltip": "Фоновые задачи",
  "chat.tasks.backgroundAll": "Перевести текущую работу в фон",
  "chat.tasks.none": "Фоновых задач нет",
  "chat.tasks.stop": "Остановить",
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `Повторная попытка (${attempt}/${max}) через ${seconds} с: ${message}`,
  "chat.notify.dismiss": "Закрыть",
  "settings.completionMode": "Подсказки команд",
  "settings.completionAuto": "Автоматически",
  "settings.completionTab": "По Tab",
  "settings.completionOff": "Выключены",
  "settings.completionUnavailable": "Не удалось загрузить или сохранить настройки.",
  "settings.completionHint": "Применяется к новым терминалам Zsh, Bash 4+, Fish и PowerShell. В CMD сохраняется стандартное действие Tab. Tab вставляет выбранную подсказку; Enter выполняет текущую команду без применения подсказки.",


};

export default ru;
