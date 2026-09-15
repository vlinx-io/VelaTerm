//! Japanese dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const ja: typeof en = {
  "tree.newPlanExecuteSession": "計画・実行セッションを新規作成…",
  "launch.splitTasks": "複数のタスクに自動分割",
  "launch.splitTasksHint": "計画セッションが独立したタスクを提案します。実行前に、各タスクの指示、エージェント、モデル、推論の強度を確認できます。",
  "launch.splitReview": "実行タスクの確認",
  "launch.splitReviewHint": "1つの計画セッションがすべての報告を受け取り、タスクごとに検証します。確認後に実行セッションが起動します。",
  "launch.splitConfirmed": "これらのタスクは確認済みです。",
  "launch.splitClosed": "この提案は確認待ちではありません。",
  "launch.splitRetry": "未配信のメッセージを再送",
  "launch.splitSharedDirectory": "すべての実行セッションは計画セッションの作業ディレクトリを使用します。ワークツリーを有効にした場合は、そのワークツリーを共有します。",
  "launch.createIn": "作成先",
  "launch.workingDirectory": "作業ディレクトリのパス",
  "launch.createAndStart": "作成して起動",
  "launch.planExecuteTaskHint": "計画に必要なタスクの目的、要件、受け入れ基準を入力してください。",
  "launch.planExecuteResult": "まず計画セッションを開始し、計画が整ったら実行セッションを作成します。",
  "launch.planExecuteWorktreeHint": "新しいワークツリーは現在のコミットから作成され、未コミットの変更は含まれません。作成に失敗した場合、該当セッションは起動しません。",
  "launch.workflowDirectorySharedHint": "計画セッションとすべての実行セッションが、1つの新しいディレクトリとブランチを共有します。",
  "launch.workflowDirectoryEachHint": "計画セッションと各実行セッションに、それぞれ専用のワークツリーとブランチを作成します。",
  "launch.planTitle": "計画と検証",
  "launch.execTitle": "実行",
  "launch.planExecuteIntro": "専用の計画セッションが作業を指示し、結果を検証して、実行セッションに修正を依頼します。",
  "chat.origin.plan": "計画",
  "chat.origin.exec": "実行",

  // Project code intelligence and knowledge entry associations.
  "knowledge.callers": "呼び出し元",
  "knowledge.callees": "呼び出し先",
  "knowledge.explore": "コード探索",
  "knowledge.exploreHint": "機能や呼び出しの流れを説明するか、ファイル名・シンボル名を入力…",
  "knowledge.impact": "影響分析",
  "knowledge.path": "呼び出し経路",
  "knowledge.target": "到達先のシンボルを検索…",
  "knowledge.depth": "探索の深さ",
  "knowledge.noPath": "インデックスに有向の呼び出し経路が見つかりませんでした。",
  "knowledge.watching": "自動同期が有効",
  "knowledge.onDemand": "クエリ前に同期",
  "knowledge.overview": "概要",
  "knowledge.uncertain": "推定された関係",
  "knowledge.kind": "シンボルの種類",
  "knowledge.language": "言語",
  "knowledge.results": "結果",
  "knowledge.resultLarge": "結果が大きすぎるため表示できません。検索範囲を絞るか、探索の深さを下げてください。",
  "knowledge.queryFailed": "コードのクエリに失敗しました。再試行するか、インデックスを同期してください。",
  "knowledge.liveHelp": "クエリプロセスの実行中はファイルの変更を同期します。アイドル状態で終了した後は、次のクエリの前に変更を反映します。",
  "knowledge.startHelp": "インデックスを有効にすると、コードの検索、呼び出しの追跡、変更の影響分析ができます。分析はバックエンドで行われ、AI モデルは使用しません。",
  "knowledge.title": "コードグラフ",
  "knowledge.intro": "コードの関係を調べ、保存された設計判断と関連付けます。",
  "knowledge.setup": "このバックエンドに CodeGraph をインストールすると、プロジェクトのインデックスを有効にできます。",
  "knowledge.downloadNotice": "検証済みの CodeGraph ランタイムを GitHub からダウンロードします。コードの解析はこのマシン内で行われ、テレメトリーと更新確認は無効です。",
  "knowledge.install": "CodeGraph をダウンロード",
  "knowledge.installing": "ダウンロードしてインストール中…",
  "knowledge.directory": "作業ディレクトリ",
  "knowledge.enable": "インデックスを有効化",
  "knowledge.disable": "インデックスを無効化",
  "knowledge.sync": "同期",
  "knowledge.ready": "利用可能",
  "knowledge.disabled": "無効",
  "knowledge.indexing": "インデックス作成中…",
  "knowledge.syncing": "同期中…",
  "knowledge.failed": "失敗",
  "knowledge.symbols": "シンボル",
  "knowledge.files": "ファイル",
  "knowledge.edges": "関係",
  "knowledge.search": "シンボルやファイルパスを検索…",
  "knowledge.searchButton": "検索",
  "knowledge.noResults": "一致するシンボルはありません。",
  "knowledge.selectSymbol": "シンボルを選択すると、ソース、関係、関連するナレッジ記事を表示します。",
  "knowledge.source": "ソース",
  "knowledge.incoming": "入力側の関係",
  "knowledge.outgoing": "出力側の関係",
  "knowledge.noEdges": "インデックスに関係がありません。",
  "knowledge.analysisNote": "関係は静的解析によるものであり、不完全または不確かな場合があります。",
  "knowledge.changed": "検索中にファイルが変更されました。行番号の利用や確認の記録を行う前に、再度同期してください。",
  "knowledge.truncated": "表示件数を制限しています。一部の関係やソース行は表示されていません。",
  "knowledge.linkMemory": "記事を関連付ける",
  "knowledge.chooseMemory": "ナレッジ記事を選択",
  "knowledge.noLinks": "コードとの関連付けはありません。シンボルの詳細から記事を関連付けられます。",
  "knowledge.inspect": "コードと記事を確認",
  "knowledge.unlink": "関連付けを削除",
  "knowledge.codeReferences": "コードへの参照",
  "knowledge.refresh": "再読み込み",
  "knowledge.current": "変更なし",
  "knowledge.review": "要確認",
  "knowledge.unavailable": "利用不可",
  "knowledge.reviewHelp": "この記事と表示中のコードを照合してください。確認すると現在のファイルバージョンを記録します。記事の本文は変更しません。",
  "knowledge.confirmReview": "確認済みにする",
  "knowledge.agentHint": "エージェントはこの作業ディレクトリで vkb search \"トピック\" を実行できます。有効なインデックスを同期し、コードとナレッジ記事を分けて返します。",
  "knowledge.busy": "インデックス処理を実行中です。このページを閉じることも、インデックスを無効にして処理を停止することもできます。",
  "knowledge.disabledHelp": "コードを検索するには、このディレクトリのインデックスを有効にしてください。無効にしてもインデックスと記事の関連付けは保持されます。",
  "knowledge.conflict": "コードまたは記事が変更されました。両方を再読み込みしてから関連付けを保存してください。",
  "knowledge.symbolMissing": "シンボルまたはソースを取得できません。同期してから再検索してください。",
  "knowledge.directoryMissing": "作業ディレクトリが存在しないか、変更されています。プロジェクトとセッションのパスを確認してください。",
  "knowledge.partial": "インデックスが不完全です。再度同期し、ソースファイルを読み取れるか確認してください。",
  "knowledge.interrupted": "前回の処理は中断されました。同期して再試行してください。",
  "knowledge.checksum": "ダウンロードのチェックサムが一致しません。ランタイムはインストールされていません。",
  "knowledge.downloadFailed": "CodeGraph をダウンロードできません。バックエンドから GitHub への接続を確認して再試行してください。",
  "knowledge.timeout": "インデックス処理がタイムアウトしました。リポジトリの規模を確認して再試行してください。",
  "knowledge.error": "操作に失敗しました。バックエンドのディレクトリへのアクセス権とランタイムを確認して再試行してください。",

  // Knowledge base: saved knowledge organized by project and session.
  "memory.hierarchy": "プロジェクトとセッション",
  "memory.up": "1つ上の階層へ",
  "memory.manualGroup": "手動作成",
  "memory.legacyGroup": "過去に統合された項目",
  "memory.unknownProject": "元のプロジェクトが不明",
  "nb.addLink": "リンクを挿入",
  "nb.attach": "添付ファイルを追加",
  "nb.browse": "参照",
  "nb.chooseNote": "最初のノートを作成",
  "nb.closeHint": "この保管庫を一覧から外します。ディスク上のファイルは残ります。",
  "nb.closeVault": "保管庫を閉じる",
  "nb.conflict": "ファイルが外部で変更されました。編集中の内容は保持されています。ファイルを再読み込みするか、新しいノートとして保存してください。",
  "nb.copyTo": "ローカル保管庫にコピー",
  "nb.createVault": "ナレッジベースを作成",
  "nb.destination": "移動先のパス",
  "nb.download": "ダウンロード",
  "nb.downloadHint": "添付ファイルをダウンロードして、別のアプリで開けます。",
  "nb.empty": "フォルダーを開くか、保管庫を新規作成して記録を始めましょう。",
  "nb.emptyImport": "読み込み可能なファイルが選択されていません。",
  "nb.emptyNotes": "ノートは Markdown ファイルとして保存されます。",
  "nb.emptyOutline": "文書の見出しがここに表示されます。",
  "nb.emptyTrash": "ごみ箱は空です。",
  "nb.error": "保管庫にアクセスできません。接続とフォルダーを確認して、もう一度お試しください。",
  "nb.exists": "移動先または同名のファイルが既に存在します。別の名前かフォルダーを選んでください。",
  "nb.favorites": "お気に入り",
  "nb.files": "ファイル",
  "nb.folder": "フォルダー",
  "nb.generatedHint": "セッションから整理されたナレッジで、出典と変更履歴も確認できます。",
  "nb.homeHint": "セッションナレッジベースとローカルナレッジベースを閲覧できます。",
  "nb.homeSearch": "セッションのナレッジとローカルノートを検索…",
  "nb.loadMore": "さらに読み込む",
  "nb.import": "読み込む",
  "nb.importFiles": "ファイルを選択",
  "nb.importFolder": "フォルダーを選択",
  "nb.importHint": "選択したフォルダーにファイルをコピーします。既存のファイルは上書きせず、非表示の設定フォルダーはスキップします。",
  "nb.imported": "読み込み済み",
  "nb.imports": "インポート履歴",
  "nb.importsEmpty": "インポート履歴はまだありません。",
  "nb.importRoot": "ナレッジベースのルート",
  "nb.importBusy": "このナレッジベースでは既にインポートを実行中です。",
  "nb.importDelete": "記録を削除",
  "nb.importDeleteConfirm": "このインポート記録を削除しますか？ インポート済みのファイルは削除されません。",
  "nb.importDone": "インポートが完了しました",
  "nb.importDuration": (seconds: string) => `${seconds} 秒`,
  "nb.importFailed": "インポートに失敗しました",
  "nb.importFilePending": "未インポート",
  "nb.importHideFiles": "ファイルを隠す",
  "nb.importInterruptedHint": "インポートは完了する前に停止しました。",
  "nb.importProgress": (done: string, total: string) => `${done} / ${total} ファイル`,
  "nb.importShowFiles": (count: string) => `ファイル（${count}）`,
  "nb.importSkipHidden": "隠しファイルまたはフォルダー",
  "nb.importStatusCancelled": "キャンセル済み",
  "nb.importStatusCompleted": "完了",
  "nb.importStatusFailed": "失敗",
  "nb.importStatusInterrupted": "中断",
  "nb.importStatusRunning": "インポート中",
  "nb.incomplete": "操作を完了できませんでした。ファイルを確認して、もう一度お試しください。",
  "nb.info": "ノートの詳細",
  "nb.invalid": "名前またはパスが無効です。",
  "nb.links": "リンク先",
  "nb.local": "ローカルファイル",
  "nb.localVaults": "ローカルナレッジベース",
  "nb.move": "名前の変更・移動",
  "nb.moveHint": "保管庫のルートからの相対パスを入力してください。ファイルやフォルダーの移動時に、既存のノートのリンクも更新します。",
  "nb.name": "名前",
  "nb.newFolder": "新規フォルダー",
  "nb.newNote": "新規ノート",
  "nb.noLinks": "リンクされたノートはありません。",
  "nb.tags": "タグ",
  "nb.notes": "ノート",
  "nb.openVault": "ナレッジベースを開く",
  "nb.outline": "アウトライン",
  "nb.quickOpen": "クイックオープン",
  "nb.readOnly": "このファイルは UTF-8 の Markdown ノートとして編集できません。",
  "nb.recent": "最近のノート",
  "nb.restore": "復元",
  "nb.reload": "ファイルを再読み込み",
  "nb.root": "フォルダーのパス",
  "nb.rootHint": "接続先のコンピューターにあるフォルダーを選んでください。既存の Markdown ファイルと添付ファイルは移動しません。",
  "nb.saveCopy": "新しいノートとして保存",
  "nb.saved": "ファイルに保存済み",
  "nb.saving": "保存中…",
  "nb.search": "ノートを検索…",
  "nb.searchAllVaults": "すべてのナレッジベース",
  "nb.searchCount": (count: string) => `${count} 件の結果`,
  "nb.searchEmpty": "一致するノートはありません。",
  "nb.searchEmptyAll": "一致するものはありません。",
  "nb.searchFuzzy": "完全一致はありません。近似の結果を表示しています。",
  "nb.searchLine": (line: string) => `${line} 行目`,
  "nb.searchMatches": (count: string) => `${count} 件一致`,
  "nb.searchMore": "先頭の結果のみ表示しています。キーワードを絞り込むと残りを確認できます。",
  "nb.searchRelated": "関連ノート",
  "nb.searchResults": "検索結果",
  "nb.searchScope": "検索範囲",
  "nb.searchThisVault": "このナレッジベース",
  "nb.skipped": "スキップ済み",
  "nb.split": "分割表示",
  "nb.tooLarge": "ファイルまたは選択した内容が保管庫の上限を超えています。",
  "nb.trash": "ごみ箱",
  "nb.trashHint": "この項目を保管庫のごみ箱に移動します。後で復元できます。",
  "nb.unsaved": "未保存の変更",
  "nb.vaults": "ナレッジベース",
  "nb.view": "表示モード",
  "nb.welcome": "自分のノート保管庫",
  "nb.welcomeText": "自由に書き、考えをつなぎ、通常のローカルファイルでノートを管理できます。既存の Markdown フォルダーを開くか、新しい保管庫に資料を読み込みましょう。",
  "memory.globalMemory": "セッションナレッジベース",
  "memory.collections": "アーカイブ済みセッション",
  "memory.collectionConversation": "会話",
  "memory.collectionEmptyEntries": "この会話にはまだナレッジ記事がありません。",
  "memory.title": "ナレッジベース",
  "memory.add": "セッションナレッジベースに整理",
  "memory.intro": "プロジェクトとセッションごとに知識を整理します。保存された記事は元の情報の変更に連動せず、手動で編集できます。",
  "memory.entries": "ナレッジ記事",
  "memory.emptyJobs": "整理履歴はまだありません。",
  "memory.jobs": "整理履歴",
  "memory.search": "記事のタイトルと本文を検索…",
  "memory.empty": "一致する記事はありません。セッションから記事を生成するか、手動で作成してください。",
  "memory.emptyDetail": "記事を選択すると、内容、関連項目、出典を確認できます。",
  "memory.new": "記事を新規作成",
  "memory.titleField": "タイトル",
  "memory.summary": "概要",
  "memory.content": "本文（Markdown）",
  "memory.tags": "タグ（カンマ区切り）",
  "memory.related": "関連記事",
  "memory.backlinks": "この記事へのリンク",
  "memory.sources": "出典",
  "memory.history": "変更履歴",
  "memory.restore": "この版を復元",
  "memory.restoreConfirm": "この版を新しいバージョンとして復元しますか？現在の版も履歴に残ります。",
  "memory.deleteConfirm": "この記事と変更履歴を削除しますか？元のセッションは削除されません。",
  "memory.groupDeleteConfirm": (count: string) => `このグループのナレッジ記事 ${count} 件をすべて削除しますか？プロジェクトまたはセッション自体は残ります。`,
  "memory.export": "Markdown をエクスポート",
  "memory.selectAgent": "エージェント",
  "memory.model": "モデル（任意）",
  "memory.modelHint": "空欄の場合、エージェントに設定されたモデルを使用します。",
  "memory.compile": "整理して保存",
  "memory.compileHelp": "選択したエージェントがこのセッションを整理します。再生成すると、このセッションから生成済みの項目が手動での編集内容も含めて上書きされます。セッションのテキストは、設定済みのエージェントを通じてモデルに送信されます。",
  "memory.unavailable": "未インストールまたは未設定",
  "memory.allTags": "すべてのタグ",
  "memory.updated": "更新日時順",
  "memory.titleSort": "タイトル順",
  "memory.sourceNote": "整理に使用した会話のテキストを保存したスナップショットです。元のセッションを削除しても閲覧できます。",
  "memory.noKnowledge": "再利用できる知識は抽出されませんでした。このセッションから生成済みの項目は削除されました。",
  "memory.queued": "開始待ち",
  "memory.cancelling": "キャンセル中",
  "memory.schedulingHint": "異なるセッションは並行して整理できます。再度送信すると、このセッションの未完了のタスクをキャンセルし、新しいタスクに置き換えます。",
  "memory.waitingHint": "このセッションの前のタスクが停止すると、自動的に開始します。",
  "memory.running": "処理中",
  "memory.completed": "完了",
  "memory.failed": "失敗",
  "memory.cancelled": "キャンセル済み",
  "memory.extract": "テーマを抽出中",
  "memory.merge": "知識を統合中",
  "memory.commit": "記事を保存中",
  "memory.done": "保存済み",
  "memory.closeHint": "整理中にこのウィンドウを閉じても処理は続きます。整理履歴で進捗を確認できます。",
  "memory.conflict": "操作中にこの記事が変更されました。再読み込みしてからやり直してください。今回の変更は保存されていません。",
  "memory.duplicate": "同じタイトルの記事が既にあります。その記事を開いて内容を統合してください。",
  "memory.notFound": "この記事、出典、または処理は存在しません。",
  "memory.noTranscript": "このセッションには読み取り可能な会話がありません。",
  "memory.agentUnavailable": "選択したエージェントを利用できません。設定で実行ファイルのパスを確認してください。",
  "memory.invalid": "無効な項目またはリンクがあります。タイトル、本文、関連記事を確認してください。",
  "memory.processFailed": "エージェントが整理を完了できませんでした。ログイン状態、モデル、CLI 設定を確認して再試行してください。",
  "memory.timeout": "エージェントがタイムアウトしました。利用可能なモデルに変更するか、会話を短くして再試行してください。",
  "memory.interrupted": "整理処理が中断されました。保存済みの出典スナップショットから再試行できます。",
  "memory.tooLarge": "出典、コンテキスト、または出力が対応サイズを超えています。内容の切り詰めや記事の保存は行われていません。",
  "memory.invalidOutput": "エージェントが無効な構造化データを返しました。保存は行われていません。再試行するか、別のエージェントを選択してください。",
  "memory.loadError": "ナレッジベースを読み込めません。接続を確認して再試行してください。",
  "memory.unsaved": "未保存の変更を破棄しますか？",
  "memory.source": "出典スナップショット",

  // ── Common ──
  "common.cancel": "キャンセル", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "削除", // Delete
  "common.save": "保存", // Save
  "common.create": "作成", // Create
  "common.close": "閉じる", // Close
  "chat.imageViewOriginal": "元の画像を表示",
  "chat.imageCopy": "画像をコピー",
  "chat.imageSave": "画像を保存",
  "chat.imageActionFailed": "画像の操作に失敗しました。もう一度お試しください。",
  "common.copy": "コピー", // Copy
  "common.cut": "切り取り", // Cut
  "common.paste": "貼り付け", // Paste
  "common.selectAll": "すべて選択", // Select All
  "common.copied": "コピーしました", // Copied
  "common.copyFailed": "コピーできませんでした。もう一度お試しください。",
  "chat.sync.loading": "会話を同期中…",
  "chat.sync.failed": "同期できませんでした。読み込み済みのメッセージは引き続き表示できます。",
  "chat.sync.history": "以前のメッセージを読み込む",
  "chat.submission.updateRequired": "このクライアントからメッセージを送信するには、サーバーを更新してください。",
  "chat.submission.sending": "送信中…",
  "chat.submission.sent": "送信済み",
  "chat.submission.queued": "送信待ち",
  "chat.submission.failed": "送信に失敗しました",
  "chat.submission.unknown": "送信結果を確認できません",
  "chat.submission.check": "状態を確認",
  "common.retry": "再試行", // Retry
  "common.experimental": "実験的機能",
  "common.refresh": "更新", // Refresh
  "common.loading": "読み込み中…", // Loading…
  "common.prev": "前へ", // Previous
  "common.next": "次へ", // Next
  "common.on": "オン", // On
  "common.off": "オフ", // Off
  "common.gotIt": "OK", // Got it
  "common.rename": "名前を変更", // Rename
  "common.edit": "編集", // Edit
  "common.open": "開く", // Open
  "common.session": "セッション", // Session

  // ── Session types and status ──
  "kind.terminal": "ターミナル", // Terminal
  "kind.browser": "ブラウザ", // Browser
  "status.idle": "待機", // Idle
  "status.running": "実行中", // Running
  "status.exited": "終了", // Exited
  "status.error": "エラー", // Error
  "status.working": "処理中", // Working
  "status.asking": "要確認", // Needs confirmation
  "status.waiting": "確認済み", // Viewed
  "status.unavailable": "ステータス取得不可",
  "indicator.unread": "未読 · 確認待ち", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `ビルド: ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `バージョン不一致：フロント v${frontend} ≠ バック v${backend}。再ビルドまたは同期デプロイしてください。`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `ホットリロード: ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `システムに従う（現在: ${resolved}）`, // Follow system (currently {resolved})
  "titlebar.themeDark": "ダーク", // Dark
  "titlebar.themeLight": "ライト", // Light
  "titlebar.gameCenter": "ゲームセンター",
  "titlebar.browser": "内蔵ブラウザ", // Built-in Browser
  "titlebar.remoteAccess": "リモートアクセス（ブラウザ）", // Remote Access (Browser)
  "titlebar.connectRemote": "リモートサーバーに接続", // Connect to Remote Server
  "titlebar.mirrored": "ミラー中", // Mirrored
  "titlebar.mirroredHint":
    "ミラーが有効です。タブ・分割・アクティブなセッションはホストに追従します。切り替えはホスト側にあります。", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `${n} 台がミラー中`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `リモート ${n} 台が接続中です。タブ・分割・アクティブなセッションは共有され、どちら側からでも変更できます。`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "接続中のクライアント", // Attached clients
  "titlebar.clientUnnamed": "名前のないクライアント", // Unnamed client
  "titlebar.clientSince": (time: string) => `${time} から`, // since {time}
  "titlebar.feedback": "フィードバック", // Feedback
  "titlebar.share": "共有", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "ファイル", // File
  "menubar.terminal": "ターミナル", // Terminal
  "menubar.help": "ヘルプ", // Help
  "menubar.newTerminal": "新しいターミナル", // New Terminal
  "menubar.visitWebsite": "ウェブサイトを開く", // Visit Website
  "menubar.sendFeedback": "フィードバックを送信", // Send Feedback
  "menubar.clearBadges": "通知バッジを消去", // Clear Notification Badges
  "share.title": "VelaTerm を共有", // Share VelaTerm
  "share.subtitle":
    "VelaTerm は小さなチームで開発しています。気に入っていただけたら、ぜひ周りの方にシェアしてください。より多くの方に私たちを知っていただくことは、チームにとって大きな支えになります。ありがとうございます！❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "リンクをコピー", // Copy link
  "share.copied": "コピーしました", // Copied!
  "share.wechatMoments": "WeChat モーメンツ",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "小紅書",
  "share.xiaohongshuAction":
    "投稿文とリンクをコピーして、小紅書クリエイターセンターを開く",
  "share.wechatQrTitle": "WeChat モーメンツにシェア",
  "share.wechatQrHint":
    "WeChat で QR コードを読み取り、リンクを開いてから「モーメンツにシェア」を選択してください。",
  "share.backToPlatforms": "共有先に戻る",
  "titlebar.appearance": "外観設定", // Appearance
  "titlebar.showLeft": "サイドバーを表示", // Show sidebar
  "titlebar.hideLeft": "サイドバーを隠す", // Hide sidebar
  "titlebar.showRight": "情報パネルを表示", // Show info panel
  "titlebar.hideRight": "情報パネルを隠す", // Hide info panel

  // ── Settings ──
  "settings.title": "設定", // Settings
  "settings.catTerminal": "ターミナル", // Terminal
  "settings.catBehavior": "動作", // Behavior
  "settings.catAgents": "エージェント", // Agents
  "settings.agentDefaultsTitle": "新規セッションの既定値",
  "settings.referSummaryTitle": "セッション参照のコンテキスト",
  "settings.referSummaryMode": "コンテキスト方式",
  "settings.referSummaryFull": "全文を使用",
  "settings.referSummaryFirst": "先に要約",
  "settings.referSummaryAgent": "要約エージェント",
  "settings.referSummaryHint":
    "既定では、vrefer --ask は回答エージェントに会話記録の全文を渡します。「先に要約」を有効にすると、ここで選択した単一のエージェント、モデル、思考レベルで圧縮し、最終回答には関連する原文の検索抜粋も渡します。",
  "settings.permDefault": "デフォルト", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `起動時に ${flag} を付与し、すべての権限確認をスキップします。慎重に使用してください。`, // YOLO flag hint
  "settings.permViaEnvHint":
    "設定ファイル経由ですべての権限確認をスキップします（CLI フラグなし）。このセッションの起動時に適用されます。",
  "settings.catGeneral": "一般", // General
  "settings.cliLabel": "シェルコマンド",
  "settings.cliInstall": "‘vela’ コマンドをインストール",
  "settings.cliUninstall": "‘vela’ コマンドをアンインストール",
  "settings.cliInstalledAt": (path: string) => `${path} にインストール済み`,
  "settings.cliConflict": (path: string) =>
    `${path} に別の ‘vela’ コマンドがあります。VelaTerm は上書きしません。`,
  "settings.cliHint":
    "VS Code の `code` と同様に `vela <project-path>` を PATH に追加します。",
  "settings.agentArgsHint":
    "各エージェントタイプの新規セッションに適用される既定の起動引数。セッションの作成・編集時に設定した個別の引数が優先されます。空欄で引数なし。", // Agent default launch args hint
  "settings.agentPathLabel": "実行ファイルパス（任意）", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "例: ~/.local/bin/claude — 空欄なら PATH から検索", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "設定すると、このタイプのセッションは PATH でコマンドを探す代わりに、この完全パスで起動します。インストール済みなのにシェルの PATH に無い場合に便利です。ワンクリックインストール成功後、場所を検出できた場合は自動で入力されます。", // Agent executable path hint
  "settings.agentDefaultView": "既定のビュー", // Default view
  "settings.agentDefaultViewHint":
    "このエージェントの新しいセッションを開くときのビューです。既存のセッションは作成時のビューのままです。", // Agent default view hint
  "settings.appearance": "外観", // Appearance
  "settings.accent": "アクセント", // Accent
  "settings.accentAuto": "テーマに従う", // Follow theme
  "settings.density": "密度", // Density
  "settings.densityCompact": "コンパクト", // Compact
  "settings.densityRegular": "標準", // Regular
  "settings.densityComfy": "ゆったり", // Comfy
  "settings.pane": "ペイン", // Panes
  "settings.paneFlush": "フラット", // Flush
  "settings.paneCard": "カード", // Card
  "settings.divider": "区切り線", // Divider
  "settings.dividerSubtle": "極細", // Subtle
  "settings.dividerVisible": "表示", // Visible
  "settings.nav": "サイドバー", // Sidebar
  "settings.navTree": "標準", // Tree
  "settings.navCompact": "コンパクト", // Compact
  "settings.tabs": "タブ", // Tabs
  "settings.dynamicStatusFilter": "状態フィルターへの動的追加",
  "settings.tabSingle": "シングル", // Single
  "settings.tabMulti": "マルチ", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "既定のシェル", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.autoContinue": "上限リセット後に自動再開", // Continue after limit resets
  "settings.autoContinueHint": "Claude または Codex が 5 時間または週間の利用上限で停止した場合、上限のリセット後にタスクを自動的に再開します。", // When a 5-hour or weekly usage limit stops Claude or Codex, the task continues automatically after the limit resets.
  "settings.cleanImages": "貼り付け画像の自動クリーンアップ",
  "settings.cleanImagesHint":
    "ターミナルに貼り付け／ドロップした画像は、まず一時ファイルとして保存されます（パスがエージェントに渡されます）。オンにすると、このセッションの一時ファイルは終了時に削除され、24 時間以上前の残りは起動時に整理されます。ドキュメント内の画像には影響しません。",
  "settings.cleanImagesNow": "今すぐクリーンアップ",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `一時画像を ${n} 件削除しました（${size} 解放）。`,
  "settings.cleanImagesEmpty": "クリーンアップする一時画像はありません。",
  "settings.imagePasteMode": "画像の貼り付け",
  "settings.imagePasteUpload": "ファイルパスを貼り付け",
  "settings.imagePasteAgent": "ネイティブ画像貼り付け",
  "settings.imagePasteHint":
    "画像を貼り付けたときに入力する内容を選びます（ローカルデスクトップのみ）。ファイルパスを貼り付け：画像を一時保存し、そのパスを Claude または Codex に入力します。ネイティブ画像貼り付け：Claude または Codex がシステムのクリップボードを読み、独自の画像プレースホルダーを表示します。",
  "settings.imagePasteRemoteHint":
    "リモートセッションでは、エージェント側で画像を読めるよう常にファイルパスを貼り付けます。ネイティブ画像貼り付けはローカルデスクトップでのみ使用できます。",
  "spawn.title": "子セッションを起動",
  "spawn.fromSession": "起動元のセッション",
  "spawn.promptLabel": "タスクの指示",
  "spawn.agentLabel": "セッションの種類",
  "spawn.worktreeLabel": "独立したワークツリー",
  "spawn.modelLabel": "モデル",
  "spawn.effortLabel": "推論強度",
  "spawn.modelDefault": "エージェントの既定値",
  "spawn.modelLoading": "モデルを取得中…",
  "spawn.modelListUnavailable": "モデル一覧がありません。識別子を直接入力できます。",
  "spawn.launch": "子セッションを起動",
  "spawn.remaining": (n: number) => `ほか ${n} 件が確認待ち`,
  "spawn.notifyTitle": "子セッションの起動確認待ち",
  "orch.title": "子セッションを一括起動",
  "orch.notifyTitle": "一括起動の確認待ち",
  "orch.coordinatorName": "セッションの状態",
  "orch.sharedSettings": "共通設定",
  "orch.agentLabel": "エージェント",
  "orch.modelLabel": "モデル",
  "orch.effortLabel": "推論強度",
  "orch.nameLabel": "セッション名",
  "orch.promptLabel": "タスクの指示",
  "orch.worktreeLabel": "Git ワークツリー",
  "orch.worktreeNone": "現在のディレクトリ",
  "orch.worktreeShared": "ワークツリーを共有",
  "orch.worktreeEach": "セッションごとに作成",
  "orch.follow": "共通設定を使用",
  "orch.overridden": "個別設定",
  "orch.remove": "タスクを除外",
  "orch.launch": (n: number) => `${n} 件のセッションを起動`,
  "orch.modelPlaceholder": "エージェントの既定値",
  "orch.effortPlaceholder": "エージェントの既定値",
  "launch.terminalHint": "通常のターミナルで作業ディレクトリを開きます。タスクの指示は自動実行されません。",
  "launch.optionsError": "起動オプションを読み込めません。再試行してから起動してください。",
  "launch.singleIntro": "起動前に、子セッションのタスクと設定を確認してください。",
  "launch.taskHint": "この内容が子セッションへの最初のメッセージになります。",
  "launch.runtime": "実行設定",
  "launch.directory": "作業ディレクトリ",
  "launch.directoryCurrentHint": "元のディレクトリでファイルを編集します。",
  "launch.directorySharedHint": "全セッションが同じ新規ディレクトリとブランチを使います。",
  "launch.directoryEachHint": "各セッションに専用のディレクトリとブランチを作成します。",
  "launch.worktreeHint": "ワークツリーは現在のコミットから作成され、未コミットの変更は含まれません。作成に失敗した場合は元のディレクトリを使います。",
  "launch.singleResult": "子セッションはサイドバーの起動元セッションの下に表示されます。",
  "launch.startError": "起動に失敗しました。設定を確認して再試行してください。",
  "launch.starting": "起動中…",
  "launch.batchIntro": "共通設定を確認し、各タスクを選択して指示を編集してください。",
  "launch.sessionCount": (n: number) => `セッション数: ${n}`,
  "launch.batchName": "タスクグループ名",
  "launch.sharedHint": "個別に変更していないセッションに適用されます。",
  "launch.tasks": "タスク一覧",
  "launch.incomplete": "入力が必要",
  "launch.undoRemove": "除外を取り消す",
  "launch.taskNumber": (n: number) => `タスク ${n}`,
  "launch.taskSettings": "このセッションの設定",
  "launch.taskAgent": "このセッションのエージェント",
  "launch.sharedDirectoryLocked": "このグループの全セッションで1つのワークツリーを共有します。",
  "launch.resetSettings": "共通設定に戻す",
  "launch.monitorHint": "起動後に「セッションの状態」ターミナルが開き、処理中か入力待ちかを表示します。タスクの完了率は示しません。",
  "launch.taskIncomplete": (n: number) => `タスク ${n} の名前と指示を入力してください。`,
  "launch.batchResult": "タスクごとに、個別に操作できる子セッションを起動します。",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "変更を表示…",
  "changes.title": "変更",
  "changes.loading": "読み込み中…",
  "changes.loadingDiff": "差分を読み込み中…",
  "changes.noChanges": "変更なし",
  "changes.refresh": "更新",
  "changes.notRepo": "Git リポジトリではありません",
  "changes.selectFile": "ファイルを選択してください",
  "changes.binary": "バイナリファイル — 行差分は表示できません",
  "changes.commitTitle": (hash: string) => `コミット ${hash}`,

  "git.staged": "ステージ済み",
  "git.changes": "変更",
  "git.untracked": "未追跡ファイル",
  "git.committed": "コミット済みの変更",
  "git.stage": "ステージ",
  "git.unstage": "ステージ解除",
  "git.stageAll": "すべてステージ",
  "git.unstageAll": "すべてステージ解除",
  "git.discard": "変更を破棄",
  "git.deleteFile": "削除",
  "git.viewAll": "すべて表示",
  "git.detached": "(detached)",
  "git.aheadBehind": "上流ブランチに対する先行・遅延コミット数",
  "git.commitPlaceholder": "コミットメッセージ",
  "git.amend": "直前のコミットを修正",
  "git.amendCommit": "コミットを修正",
  "git.commitCount": (n: number) => `${n} ファイルをコミット`,
  "git.commitNoFiles": "このコミットにファイルの変更はありません",
  "git.noCommits": "コミットがありません",
  "git.loadMore": "さらに読み込む",
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
  "tree.moveGroupToWorktree": "Worktree に移動…",
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
  "settings.renderer": "ターミナルレンダラー", // Terminal renderer
  "settings.redrawOnReveal": "タブ復帰時に再描画", // Redraw on tab switch
  "settings.catAdvanced": "詳細設定", // Advanced
  "settings.outputScheduler": "フォアグラウンド優先出力", // Foreground-priority output
  "settings.recordSessions": "セッションログを記録", // Record session logs
  "settings.recordSessionsHint":
    "既定はオフ。オンにするとターミナル出力をログファイルに保存し、アーカイブ再生と検索に使います。通常のターミナルセッションは記録しません。エージェントセッションは独自の会話記録を読み込みます。", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.termLineHeight": "ターミナルの行の高さ",
  "settings.chatTypography": "会話ビュー",
  "settings.chatTypographyHint": "フォント設定はターミナルとは別に保存され、変更はすぐに反映されます。",
  "settings.chatFont": "会話のフォント",
  "settings.chatFontSize": "会話の文字サイズ",
  "settings.chatLineHeight": "会話の行の高さ",
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontListUnavailable": "システムフォントの一覧を取得できません。フォント名を手動で入力できます。",
  "settings.fontUnconfirmed": "このフォントが利用可能か確認できません。",
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "通知音", // Notification sound
  "settings.language": "言語", // Language
  "settings.langAuto": "自動（システム）", // Auto (system)
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
  "settings.catShortcuts": "ショートカット", // Shortcuts
  "settings.scOpenProject": "プロジェクトを開く", // Open project
  "settings.scNewTab": "新規ターミナル", // New terminal
  "settings.scNewBrowserTab": "新規ブラウザタブ", // New browser tab
  "settings.scClosePane": "ペイン／タブを閉じる", // Close pane / tab
  "settings.scSplitRight": "右に分割", // Split right
  "settings.scSplitDown": "下に分割", // Split down
  "settings.scSearch": "ターミナル内を検索", // Find in terminal
  "settings.scGlobalSearch": "全セッションを検索", // Search all sessions
  "settings.scSaveDoc": "ドキュメントを保存", // Save document
  "settings.scRecording": "キーを押してください…", // Press keys…
  "settings.scHint":
    "ショートカットをクリックし、新しい組み合わせを押します（Cmd/Ctrl が必要）。", // hint
  "settings.scReset": "デフォルトに戻す", // Restore defaults
  "settings.scConflict": (label: string) =>
    `「${label}」で既に使用されています`, // conflict

  // ── Remote access panel ──
  "remote.title": "リモートアクセス（ブラウザ）", // Remote Access (Browser)
  "remote.desc":
    "有効にすると、同じ LAN 上のデバイスがブラウザで下記アドレスを開き、パスワードを入力すれば、デスクトップと同じ画面を利用できます。", // Once enabled, devices on the same LAN…
  "remote.needPassword": "先にアクセスパスワードを設定してください", // Please set an access password first
  "remote.running": (port) => `稼働中 · ポート ${port}`, // Running · port {port}
  "remote.urlsHint":
    "お使いのデバイスと同じ WiFi / サブネットのアドレスをブラウザで開いてください（NIC が複数ある場合は適切なものを選択。VPN/トンネルのアドレスは末尾に表示され、外部デバイスからは通常接続できません）：", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "クリックしてアドレスをコピー", // Click to copy address
  "remote.moreUrls": (n: number) => `他 ${n} 件のリンク`, // N more urls
  "remote.lessUrls": "折りたたむ", // Show less
  "remote.stop": "サーバーを停止", // Stop Server
  "remote.passwordPlaceholder": "アクセスパスワードを設定", // Set access password
  "remote.starting": "起動中…", // Starting…
  "remote.start": "サーバーを開始", // Start Server
  "remote.portLabel": "ポート", // Port
  "remote.portInvalid": "ポートは 1〜65535 の範囲で指定してください", // Port must be between 1 and 65535
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "自動（最初の LAN アドレス）", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "スマートフォンでスキャンすると、選択したアドレスでペアリングリンクを開けます。", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "証明書フィンガープリント (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "初回接続時、ブラウザは証明書が信頼されていないと警告します（自己署名証明書では正常です）。このフィンガープリントを照合し、この端末への接続であることを確認してください。", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "ペアリングリンクを作成", // Create pairing link
  "remote.pairingRegenerate": "リンクを再生成（全端末を切断）", // Regenerate link (disconnects all)
  "remote.pairingCreating": "生成中…", // Generating…
  "remote.pairingHint":
    "ブラウザで開いてパスワードを入力します。このリンクには認証情報が含まれます。自分のデバイスにのみ共有してください。", // Open in a browser, then enter the password…

  "remote.devicesLabel": "ペア済みデバイス", // Paired devices
  "remote.lastSeen": "最終接続", // Last seen
  "remote.revoke": "失効", // Revoke
  "remote.deviceBlock": "ブロック", // Block
  "remote.deviceBlockConfirm": "ブロックする", // Confirm block
  "remote.deviceBlockHint":
    "ブロックした端末は切断され、再接続できません（再度ペアリングリンクが必要）。他の端末には影響しません。", // Block hint
  "remote.devicesEmpty": "ペアリング済みの端末はありません", // No paired devices yet
  "remote.autoRestartHint":
    "リモートアクセスはアプリの再起動時に自動的に再開されます。「サーバーを停止」で無効になります。", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "自動起動に失敗しました:", // Automatic start failed:
  "remote.mirror": "デバイス間でレイアウトを同期", // Mirror layout across devices
  "remote.mirrorHint":
    "タブ・分割・アクティブなセッションが接続中のすべての端末で一致します。キーボードフォーカスは各端末でそのまま保たれます。", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "リモートサーバーに接続", // Connect to Remote Server
  "connect.pairingPlaceholder": "ペアリングリンクを貼り付け", // Paste pairing link
  "connect.confirmConnect": "指紋を確認して接続", // Fingerprint matches, connect
  "connect.desc":
    "リモート VelaTerm のアドレスとパスワードを入力し、新しいウィンドウで接続・操作します。", // Enter the address and password…
  "connect.addressPlaceholder": "IPアドレス（例: 192.168.1.100）", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "ポート", // Port
  "connect.connecting": "接続中…", // Connecting…
  "connect.connect": "接続", // Connect
  "connect.stagePreparing": "サーバーを準備中…",
  "connect.stageTransferring": "サーバーを転送中…",
  "connect.stageStarting": "サーバーを起動中…",
  "connect.sshFingerprintLabel": (kt: string) =>
    `SSH ホスト鍵のフィンガープリント（${kt}）`,
  "connect.sshHostNew":
    "このホストへの初回接続です。続行する前にフィンガープリントを確認してください。",
  "connect.sshHostChanged":
    "⚠ このホストの鍵が変更されました。サーバーの再インストール、または中間者攻撃の可能性があります。確信がある場合のみ続行してください。",
  "connect.urlCertChanged":
    "⚠ このサーバーの証明書フィンガープリントが前回の確認から変更されました。サーバーの再インストール、または中間者攻撃の可能性があります。確信がある場合のみ続行してください。",
  "connect.sshPasswordLabel": "SSH パスワード",
  "connect.sshPasswordPlaceholder": "アカウントのパスワード",
  "connect.savedHosts": "最近の接続先",
  "connect.savedHostsAll": "最近の接続先すべて",
  "connect.showAllHosts": (n: number) => `すべて表示 (${n})`,
  "connect.forgetHost": "このホストを削除",
  "connect.savedHasPassword": "パスワード保存済み",
  "connect.rememberPassword": "パスワードを保存",
  "connect.showPassword": "パスワードを表示",
  "connect.hidePassword": "パスワードを非表示",
  "connect.urlPasswordPlaceholder": "ログインパスワード",
  "connect.mirror": "リモートのデスクトップ版をミラー", // Mirror the remote desktop app
  "connect.mirrorHint":
    "タブ・分割・アクティブなセッションがリモート機のデスクトップ版と一致し、どちらで変更しても両方に反映されます。デスクトップ版が起動していない場合はそのデータベースを直接開き、データベースがなければ独立したデータベースを使います。", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb": "リモートのデスクトップ版のデータベースを共用",
  "connect.shareDesktopDbHint":
    "リモートマシンのデスクトップ版と同じデータベースを共有します（両方を同じバージョンに揃えることを推奨）。オフにすると独立したデータベースを使用します。",

  // ── Sidebar ──
  "tree.newSession": "新規セッション", // New Session
  "tree.newTerminalSession": "新規ターミナル", // New Terminal
  "tree.newBrowserPage": "新規ブラウザページ", // New Browser Page
  "tree.newAgentSession": (agent) => `新規 ${agent} セッション`, // New {agent} Session
  "tree.newAgentSessionGroup": "その他のエージェントセッション", // More Agent Session
  "tree.newAgentSessionCustom": "起動引数を指定して新規…", // New with launch args…
  "tree.resumeSession": "セッションを再開…", // Resume Session…
  "tree.newGroup": "新規グループ", // New Group
  "tree.newSubgroup": "新規サブグループ", // New Subgroup
  "tree.newChildSession": "新規子セッション", // New Child Session
  "tree.openSelected": "選択したセッションを開く", // Open Selected Sessions
  "tree.archiveSelected": "選択したセッションをアーカイブ", // Archive Selected Sessions
  "tree.moveSelected": "選択項目を移動…", // Move Selected to…
  "tree.deleteSelected": (n) => `選択した ${n} 件を削除`, // Delete {n} Selected Items
  "tree.removeProject": "プロジェクトを削除", // Remove Project
  "tree.deleteGroup": "グループを削除", // Delete Group
  "tree.deleteSession": "セッションを削除", // Delete Session
  "tree.projectRoot": "プロジェクトルート（グループなし）", // Project root (no group)
  "tree.moveToSession": "セッションの下へ移動（子にする）", // Move under a session (as child)
  "tree.moveTo": "移動先…", // Move to…
  "tree.openNewTab": "新しいタブで開く", // Open in New Tab
  "tree.forkSession": "セッションをフォーク", // Fork Session
  "tree.exportSession": "セッションをエクスポート…", // Export Session…
  "tree.sessionInfo": "セッション情報", // Session Info
  "tree.groupInfo": "グループ情報", // Group Info
  "tree.collectionInfo": "コレクション情報", // Collection Info
  "tree.projectInfo": "プロジェクト情報", // Project Info
  "info.branch": "ブランチ", // Branch
  "info.path": "パス", // Path
  "info.recentCommits": "最近のコミット", // Recent Commits
  "info.noCommits": "コミットなし", // No commits
  "tree.killProcess": "プロセスを終了", // Kill Process
  "tree.killProcessConfirm": (name: string) => `「${name}」のプロセスを終了しますか？実行中のタスクは中断されます。保存済みの会話履歴とファイルは保持されます。`,
  "tree.archiveSession": "セッションをアーカイブ", // Archive Session
  "tree.archiveGroup": "グループをアーカイブ", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "一時", // scratch
  "tree.persistSession": "永続セッションに変換…", // Make Permanent Session…
  "tree.persistDoc": "ディスクに保存…", // Save to Disk…
  "tree.closeScratch": "下書きを閉じる", // Close Scratch
  "tree.importProject": "プロジェクトをインポート", // Import Project
  "tree.createProject": "プロジェクトを作成",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "新規コレクション",
  "tree.deleteCollection": "コレクションを削除",
  "collection.title": "新規コレクション",
  "collection.name": "コレクション名",
  "collection.namePlaceholder": "research",
  "collection.submit": "コレクションを作成",
  "collection.tag": "フォルダーなし",
  "collection.deleteTitle": "コレクションを削除",
  "collection.deleteBody": (name) =>
    `コレクション「${name}」を削除しますか？中のグループとセッションもすべて削除され、元に戻せません。`,
  "tree.cloneProject": "Git からクローン", // Clone from Git
  "createProject.title": "プロジェクトを作成",
  "createProject.name": "プロジェクト名",
  "createProject.namePlaceholder": "my-project",
  "createProject.into": "作成先",
  "createProject.choose": "選択…",
  "createProject.noParent": "親フォルダーを選択してください",
  "createProject.invalidName":
    "/ または \\ を含まない単一のフォルダー名を入力してください。",
  "createProject.creating": "作成中…",
  "createProject.submit": "プロジェクトを作成",
  "clone.title": "Git リポジトリをクローン", // Clone Git Repository
  "clone.url": "リポジトリ URL", // Repository URL
  "clone.urlPlaceholder": "https://… または git@…",
  "clone.branch": "ブランチ（任意）", // Branch (optional)
  "clone.branchPlaceholder": "空欄なら既定のブランチ", // Default branch if empty
  "clone.folder": "フォルダ名", // Folder name
  "clone.folderPlaceholder": "URL から自動取得", // Auto from URL
  "clone.into": "クローン先", // Clone into
  "clone.choose": "選択…", // Choose…
  "clone.noParent": "親フォルダを選択してください", // Choose a parent folder
  "clone.cloning": "クローン中…", // Cloning…
  "clone.cancelling": "キャンセル中…",
  "clone.stageStarting": "Git を起動しています…",
  "clone.stageConnecting": "リポジトリに接続しています…",
  "clone.stagePreparing": "オブジェクトを準備しています…",
  "clone.stageReceiving": "オブジェクトを受信しています…",
  "clone.stageResolving": "差分を解決しています…",
  "clone.stageCheckout": "ファイルをチェックアウトしています…",
  "clone.stageFinalizing": "完了処理中…",
  "clone.stageImporting": "プロジェクトをインポートしています…",
  "clone.elapsed": (seconds: number) => `経過 ${seconds} 秒`,
  "clone.slowHint":
    "30 秒間進捗がありません。リモートマシンのネットワークまたはプロキシを確認するか、キャンセルして再試行してください。",
  "clone.submit": "クローン", // Clone
  "tree.globalSearch": "すべてのセッションを検索", // Search All Sessions
  "tree.archivedSessions": "アーカイブ済みセッション", // Archived Sessions
  "tree.searchPlaceholder": "セッション / グループを検索…", // Search sessions / groups…
  "tree.clearSearch": "検索をクリア", // Clear search
  "tree.filterWorking": "作業中", // Working
  "tree.filterAsking": "対応待ち", // Pending
  "tree.filterWaiting": "確認済み", // Viewed
  "tree.filterStatus": "ステータスで絞り込み", // Filter by status
  "tree.refreshStatusFilter": "ステータスフィルターを更新",
  "tree.refreshStatusMatch": "ステータスを更新",
  "tree.filterStatusSection": "ステータス", // Status
  "tree.filterMarkSection": "マーク", // Mark
  "tree.viewMainName": "メイン",
  "tree.viewUntitled": "名称未設定ビュー",
  "tree.viewDefaultName": (n) => `ビュー ${n}`,
  "tree.viewPrimary": "メインビュー",
  "tree.viewManage": "ビューを管理",
  "tree.viewSetPrimary": "メインに設定",
  "tree.viewRename": "ビュー名を変更",
  "tree.viewName": "ビュー名",
  "tree.viewDelete": "ビューを削除",
  "tree.viewDeletePrimary": "メインビューは削除できません",
  "tree.viewDeleteTitle": "ツリービューを削除",
  "tree.viewDeleteConfirm": (name) =>
    `「${name}」を削除しますか？保存された検索とフィルターは削除されますが、プロジェクトとセッションには影響しません。`,
  "tree.viewSplitRight": "ツリービューを右に分割",
  "tree.viewSplitDown": "ツリービューを下に分割",
  "tree.viewAdd": "現在のツリービューを新しいタブにコピー",
  "tree.viewCount": (n) => `${n} 個のツリービュー`,
  "mark.menu": "マーク", // Mark
  "mark.urgent": "緊急", // Urgent
  "mark.important": "重要", // Important
  "mark.bug": "バグ", // Bug
  "mark.done": "完了", // Done
  "mark.wip": "進行中", // In progress
  "mark.pinned": "ピン留め", // Pinned
  "mark.idea": "アイデア", // Idea
  "mark.caution": "注意", // Caution
  "tree.clearAllNotifications":
    "通知バッジをすべてクリア（セッションのドットと Dock バッジ）", // Clear all notification badges…
  "tree.noProjectsPre":
    "プロジェクトがまだありません。フォルダーアイコンをクリック、または ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " でディレクトリをインポートしてください。", // to import a directory.
  "tree.openProject": "プロジェクトを開く", // Open Project
  "tree.noAttention": "ステータス絞り込みに一致するセッションはありません", // No sessions match the status filter
  "tree.noMatch": "一致なし", // No matches

  // Dialog fields
  "tree.groupName": "グループ名", // Group name
  "tree.sessionNameAuto": "セッション名（空欄で自動命名）", // Session name (leave empty to auto-name)
  "tree.editSession": "セッションを編集", // Edit Session
  "tree.sessionName": "セッション名", // Session name
  "tree.shellLabel": "シェル（空欄でシステム既定）", // Shell (leave empty for system default)
  "tree.shellMenu": "シェル", // Shell
  "tree.downloadFullGitbash": "完全版 Git Bash をダウンロード",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "完全版 Git Bash をダウンロード中…",
  "gitbash.extracting": "完全版 Git Bash を展開中…",
  "gitbash.done": "完全版 Git Bash の準備ができました。",
  "gitbash.failed": "Git Bash のダウンロードに失敗しました",
  "tree.shellSystemDefault": "システム既定", // System default
  "form.customOption": "カスタム…", // Custom…
  "tree.cwdLabel": "作業ディレクトリ（空欄でプロジェクトルート）", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "起動コマンド（任意）", // Startup command (optional)
  "tree.engineLabel": "表示形式",
  "tree.engineTui": "ターミナルビュー",
  "tree.engineChat": "会話ビュー",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "エージェント本体のターミナル画面で実行します。",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "メッセージとツールカードで表示し、権限の要求は画面上で応答します。",
  "tree.agentArgsLabel": "起動引数（任意）", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "作業ディレクトリ",
  "tree.workingDirPlaceholder": "空欄なら既定のディレクトリ",
  "preset.execPathLabel": "実行ファイル（任意）",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "空欄ならエージェントに設定済みのコマンドを使います。指定するとこのセッションだけが互換の別プログラムで動きます。",
  "preset.saveLabel": "プリセットとして保存",
  "preset.namePlaceholder": "プリセット名",
  "preset.iconChoose": "アイコンを選択",
  "preset.iconClear": "削除",
  "preset.iconHint":
    "正方形の画像が最適です。それ以外は切り取って 64x64 に縮小します。",
  "tree.permissionSkipLabel": "すべての権限確認をスキップ", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "起動時にこのエージェントのバイパス用フラグを付与します（例: Claude の --dangerously-skip-permissions。Codex はサンドボックスも無効化）。毎回の起動で有効になるため、慎重に使用してください。",
  "tree.permissionUnsupported":
    "OpenCode は設定ファイルで権限を管理し、対応する起動フラグがないため、この項目は適用されません。",
  "tree.permissionUnsupportedPi":
    "Pi は設計上、権限確認のプロンプトなしでツールを実行するため、この項目は適用されません。",

  // New agent-session dialog
  "newAgent.desc":
    "セッション名とカスタム起動引数（agent コマンドに渡されます。例: --model opus）は任意です。両方空のまま Enter を押すと通常どおり起動します。", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "一括削除", // Batch Delete
  "tree.deleteProjectTitle": "プロジェクトを削除", // Delete Project
  "tree.deleteGroupTitle": "グループを削除", // Delete Group
  "tree.deleteSessionTitle": "セッションを削除", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `選択した ${n} 件を削除します（プロジェクト/グループは配下のサブグループとセッションも連鎖削除されます）。この操作は取り消せません。`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `プロジェクト「${name}」を削除しますか？配下のサブグループとセッションもすべて削除されます。この操作は取り消せません。`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `グループ「${name}」を削除しますか？配下のサブグループとセッションもすべて削除されます。この操作は取り消せません。`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `セッション「${name}」（およびすべての子セッション）を削除しますか？この操作は取り消せません。`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `関連する git worktree も削除する（計 ${n} 件。作業ツリーに変更があると削除に失敗することがあります）`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "名前", // Name
  "info.type": "種類", // Type
  "info.status": "状態", // Status
  "info.notYetCaptured": "未生成（初回実行後に取得）", // Not yet generated (captured after first run)
  "info.sessionId": "セッション ID", // Session ID
  "info.projectId": "プロジェクト ID", // Project ID
  "info.cwd": "作業ディレクトリ", // Working dir
  "info.initCmd": "起動コマンド", // Startup cmd
  "info.agentArgs": "起動引数", // Launch args
  "info.launchCmd": "完全な起動コマンド", // Full launch command
  "info.permission": "権限", // Permission
  "info.permissionSkip": "すべての確認をスキップ", // Skip all confirmations
  "info.parentSessionId": "親セッション ID", // Parent ID
  "info.termTitle": "ターミナルタイトル", // Terminal title
  "info.createdAt": "作成日時", // Created at

  // Resume-session dialog
  "importSessions.results": ({ count }: { count: number }) => `${count} 件の結果`,
  "importSessions.selected": ({ count }: { count: number }) => `${count} 件選択中`,
  "importSessions.clearSelection": "選択を解除",
  "importSessions.clearSearch": "検索をクリア",
  "importSessions.noHistory": "このプロジェクトのディレクトリにセッション履歴はありません。",
  "importSessions.title": "セッションをインポート",
  "importSessions.description": "作業ディレクトリがこのプロジェクトと一致する Codex、Claude、OpenCode の既存セッションを検索します。セッションを選択してプロジェクトに追加し、開くと会話を再開できます。",
  "importSessions.search": "タイトル、エージェント、セッション ID で検索",
  "importSessions.empty": "一致するセッションが見つかりません。",
  "importSessions.imported": "インポート済み",
  "importSessions.confirm": ({ count }: { count: number }) => `インポート（${count}）`,
  "importSessions.success": ({ count }: { count: number }) => `${count} 件のセッションをプロジェクトに追加しました。`,
  "resume.title": "セッションを再開", // Resume Session
  "resume.desc":
    "エージェントの種類を選び、そのエージェント自身の session id を入力します。開くと元の会話を引き継ぎます。", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "エージェント種別", // Agent type
  "resume.sessionIdPlaceholder": "会話の session id", // Conversation session id
  "resume.confirm": "再開して開く", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "新規 worktree セッション…", // New Worktree Session…
  "worktree.worktreeNameLabel": "worktree 名", // Worktree name
  "worktree.worktreeNameHint":
    "worktree のディレクトリ名とブランチ名に使われます。", // Used as the worktree directory and branch name.
  "worktree.createFailed": "worktree の作成に失敗しました", // Couldn't create the worktree
  "worktree.noRepoRoot":
    "このプロジェクトには利用可能な git リポジトリのパスがありません。", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "なし", // None
  "worktreeSel.modeNew": "新規", // New
  "worktreeSel.modeExisting": "既存", // Existing
  "worktreeSel.loading": "worktree を読み込み中…", // Loading worktrees…
  "worktreeSel.empty": "このリポジトリには既存の worktree がありません。", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "worktree を一覧できませんでした（git リポジトリではない？）。", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "このグループで作成したセッションは既定でこの worktree を使用します。", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "グループを Worktree に移動",
  "worktree.moveGroupHint":
    "以後このグループで作成するセッションはこの worktree を使います。既存のセッションは今のディレクトリのままです。",

  // ── Archive panel ──
  "archive.title": "アーカイブ済みセッション", // Archived Sessions
  "archive.empty1": "アーカイブ済みセッションはありません。", // No archived sessions.
  "archive.empty2":
    "サイドバーのセッションを右クリックして「セッションをアーカイブ」を選ぶとここに収納されます。", // Right-click a session in the sidebar…
  "archive.restore": "通常のセッションに戻す", // Restore to normal session
  "archive.export": "完全なコンテキストを Markdown でエクスポート", // Export full context as Markdown
  "archive.deleteForever": "完全に削除（録画も含む）", // Delete permanently (with recording)
  "archive.pickOne": "左側のアーカイブ済みセッションを選んで会話履歴を表示", // Select an archived session on the left…
  "archive.recordingEnd": "--- 録画終了 ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `録画の読み込みに失敗: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "録画内を検索…", // Search in recording…
  "archive.searchTranscript": "会話内容を検索…", // Search transcript…
  "archive.searchPlaceholder": "アーカイブ内容を検索…", // Search archived content…
  "archive.msgCountAll": (n) => `${n} 件`, // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} 件`, // {shown} / {total} messages
  "archive.you": "あなた", // You
  "archive.toolsUsed": (tools) => `ツール: ${tools}`, // Tools: {tools}
  "archive.noMatch": "一致するメッセージはありません", // No matching messages
  "archive.emptyTranscript": "会話履歴は空です", // Transcript is empty
  "archive.loadingTranscript": "会話履歴を読み込み中…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "すべてのセッション内容を検索…", // Search across all session content…
  "search.hint":
    "セッション内容を検索します。アーカイブ済みは既定で除外され、「アーカイブも検索」で追加できます。", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "アーカイブも検索", // Include archived
  "search.includeArchivedHint":
    "アーカイブ済みセッションも検索対象に含める（既定ではオフ）", // Also search archived sessions (off by default)
  "search.searching": "検索中…", // Searching…
  "search.noResults": "一致する項目が見つかりません", // No matches found
  "search.sessionCount": (n) => `${n} 件のセッション`, // n sessions
  "search.matchCount": (n) => `${n} 件の一致`, // n matches
  "search.pickSession": "左側のセッションを選択すると一致箇所が表示されます", // Select a session on the left to see its matches
  "search.openSession": "セッションを開く", // Open session
  "search.backToResults": "結果に戻る", // Back to results
  "search.archivedBadge": "アーカイブ済み", // Archived
  "search.summary": (m, s) => `${m} 件の一致 · ${s} 件のセッション`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} / ${total}`, // N of M
  "search.roleTerminal": "ターミナル", // Terminal
  "search.collapseGroup": "折りたたむ", // Collapse
  "search.expandGroup": "展開", // Expand
  "search.cappedNote": (l, total) => `${total} 件中 ${l} 件に移動可能`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "セッションなし", // No session
  "center.noSessionHintPre": "サイドバーからセッションを選ぶか、", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " でターミナルを作成", // to create a terminal
  "center.createTerminal": "ターミナルを作成", // Create Terminal
  "tab.unsavedDot": "未保存の変更あり", // Unsaved changes
  "tab.newTerminal": "新規ターミナル", // New terminal
  "tab.newDocument": "新規ドキュメント", // New document
  "tab.bgTitle": (n) => `バックグラウンド常駐タブ: ${n} 件（プロセスは実行中）`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `バックグラウンド ${n}`, // Background {n}
  "tab.scratchFallback": "（一時ターミナル）", // (scratch terminal)
  "tab.killBgTab": "このバックグラウンドタブを終了（プロセスも終了します）", // Kill this background tab…
  "tab.newBrowserTab": "新しいタブ", // New Tab
  "tab.refreshFile": "ファイルを再読み込み", // Refresh File
  "tab.closeOthers": "他のタブを閉じる", // Close Other Tabs
  "tab.closeRight": "右側のタブを閉じる", // Close Tabs to the Right
  "tab.closeAll": "すべてのタブを閉じる", // Close All Tabs
  "tab.sendToBackground": "バックグラウンドに移動", // Send to Background

  // ── Built-in browser ──
  "browser.back": "戻る", // Back
  "browser.forward": "進む", // Forward
  "browser.reload": "再読み込み", // Reload
  "browser.desktopOnly": "ブラウザタブはデスクトップアプリでのみ開けます。", // Browser tabs open in the desktop app only.
  "browser.stop": "読み込みを中止", // Stop loading
  "browser.openExternal": "システムのブラウザで開く", // Open in system browser
  "browser.addressPlaceholder": "URL または検索語を入力", // Enter URL or search terms
  "browser.quickAccess": "クイックアクセス", // Quick access
  "browser.loading": "読み込み中…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "VelaTerm を終了しますか？", // Quit VelaTerm?
  "quit.body":
    "実行中のターミナルとエージェントのセッションはすべて停止します。", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "ワークスペースを保存", // Save workspace
  "quit.saveWorkspaceHint":
    "次回、同じタブと分割を復元します。ターミナルは復元されますが再起動はされません。", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "終了", // Quit
  "dormant.body":
    "保存したワークスペースから復元しました。プロセスはまだ起動していません。", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "起動", // Start
  "overlimit.title": (max) =>
    `バックグラウンド常駐が上限を超えています（最大 ${max} 件）`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "貼り付け", // Paste
  "term.pasteUseShortcut": "貼り付け（⌘V を押してください）", // Paste (press ⌘V)
  "term.selectAll": "すべて選択", // Select All
  "term.autoCopied": (n: number) => `${n} 文字を自動コピー · ⌘V で貼り付け`,
  "term.clear": "画面をクリア", // Clear
  "term.searchMenu": "検索…", // Search…  ⌘F
  "term.splitRight": "右に分割", // Split right (⌘D)
  "term.splitDown": "下に分割", // Split down (⌘⇧D)
  "term.closePane": "分割を閉じる", // Close split
  "term.redraw": "再描画", // Redraw
  "term.mirrorTooltip":
    "ミラー表示中（サイズは他のクライアントが制御）。クリックすると PTY をこのウィンドウのサイズに合わせます", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) =>
    `⤢ ミラー${dims} · クリックでこのウィンドウに合わせる`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) =>
    `⤢ ミラー${dims} · このウィンドウに合わせる`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `画像のアップロードに失敗: ${n} 件${lastError ? `（${lastError}）` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "クリップボードから画像を読み取れませんでした。画像をコピーし直して再試行してください。",
  "term.starting": (agent) => `${agent} を起動中…`, // Starting {agent}…
  "term.startFailed": (err) => `起動に失敗: ${err}`, // Failed to start: {err}

  // ── Agent installation guidance ──
  "agentInstall.title": (label) => `${label} がインストールされていません`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm は PATH 上で ${label} を見つけられませんでした。インストールするとこのセッションを起動できます。`, // couldn't find {label} on PATH
  "agentInstall.install": "今すぐインストール", // Install now
  "agentInstall.retry": "起動を再試行", // Retry launch
  "agentInstall.dismiss": "自分でインストールする", // I'll do it myself
  "agentInstall.docs": "インストール手順", // Install docs
  "agentInstall.needsNode": "Node.js / npm が必要です", // Requires Node.js / npm
  "agentInstall.afterInstall": "インストール後：", // After install:
  "agentInstall.pathSaved": (label: string) =>
    `${label} の実行ファイルパスを設定に保存しました:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) =>
    `${label} はインストール済みです`, // {label} is installed
  "agentInstall.doneDesc": "このセッションを再起動すると使い始められます。", // Relaunch this session to start using it.
  "agentInstall.restartNow": "今すぐ再起動", // Relaunch now
  "agentInstall.later": "後で", // Later
  "agentInstall.pathLabel": "実行ファイルのパス", // Executable path
  "agentInstall.pathPlaceholder": (bin: string) => `~/.local/bin/${bin}`,
  "agentInstall.pathHint": "PATH 以外にインストール済みの場合は、実行ファイルのフルパスを入力します。", // Already installed outside PATH?
  "agentInstall.pathSave": "このパスを使う", // Use this path
  "agentInstall.pathBrowse": "参照…", // Browse…
  "search.placeholder": "ターミナル内を検索", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.visual": "ビジュアル",
  "doc.source": "ソース",
  "doc.compare": "比較",
  "doc.editorLoadFailed": "Markdown エディターを読み込めませんでした。",
  "doc.imageOnly": "ここには画像ファイルのみ挿入できます。",
  "doc.searchPlaceholder": "検索", // Find
  "doc.searchReplacePlaceholder": "置換", // Replace
  "doc.searchReplace": "置換", // Replace
  "doc.searchReplaceAll": "すべて", // All
  "doc.searchNoMatch": "一致なし", // No results
  "doc.searchCaseSensitive": "大文字小文字を区別", // Match case
  "doc.searchToggleReplace": "置換を切り替え", // Toggle replace
  "doc.fileTree": "ファイルツリー", // File tree
  "doc.treeUp": "親フォルダ", // Parent folder
  "doc.sidebar": "サイドバー", // Sidebar
  "doc.unsaved": "未保存", // Unsaved
  "doc.saveAsTitle": "名前を付けて保存", // Save As
  "doc.saveAsName": "ファイル名", // File name
  "doc.outline": "アウトライン", // Outline
  "doc.outlineEmpty": "見出しなし", // No headings
  "doc.saving": "保存中…", // Saving…
  "doc.overwriteConfirm":
    "同名のファイルが既に存在します。「上書き」で置き換えます。", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "保存", // Save
  "doc.externalChanged":
    "ファイルがディスク上で変更されました（未保存のローカル変更があります）。", // The file was modified on disk…
  "doc.reloadDiscard": "再読み込み（自分の変更を破棄）", // Reload (discard my changes)
  "doc.externalChangedClean": "ファイルがディスク上で変更されました。", // The file was modified on disk.
  "doc.reload": "再読み込み", // Reload
  "doc.ignore": "無視", // Ignore
  "doc.loadingFile": (title) => `${title} を読み込み中…`, // Loading {title}…
  "doc.closeTitle": "ドキュメントを閉じる", // Close Document
  "doc.unsavedBody": (title) => `「${title}」には未保存の変更があります。`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "保存して閉じる", // Save & Close
  "doc.closeNoSave": "保存せずに閉じる", // Close Without Saving
  "doc.conflictTitle": "保存の競合", // Save Conflict
  "doc.conflictBody":
    "ディスク上のファイルが外部で変更されています。それでも現在の内容で上書きしますか？", // The file on disk was modified externally…
  "doc.overwrite": "上書き", // Overwrite
  "doc.saveFailed": (err) => `保存に失敗: ${err}`, // Save failed: {err}
  "doc.closeTab": "タブを閉じる", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `読み取り専用：先頭 10 MB のみ表示（全 ${size}）。ファイルの残りを上書きしないよう保存は無効です。`,
  "doc.imgLoading": (title, size) => `${title}（${size}）を読み込み中…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "ファイルは書き込み中です。安定したら自動で再読み込みします。", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed":
    "この画像を表示できません（未対応の形式か破損しています）。", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "ウィンドウに合わせる", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "PDFとして書き出す", // Export PDF
  "doc.diagramError": "図の構文エラー", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "セッション未選択", // No session selected
  "panel.collapseSection": "セクションを折りたたむ", // Collapse section
  "panel.expandSection": "セクションを展開する", // Collapse section
  "panel.openInEditor": "エディタで開く", // Open in Editor
  "panel.openInEditorTooltip":
    "中央のドキュメントエディタで開く（view コマンドと同じ）", // Open in the document editor…
  "panel.preview": "プレビュー", // Preview
  "panel.cantRead": "（このファイルを読み取れません）", // (cannot read this file)
  "panel.binary": "（バイナリファイルのためプレビューなし）", // (binary file, no preview)
  "panel.truncated": "\n…（長すぎるため省略）", // …(content truncated)
  "panel.showHidden": "隠しファイルを表示", // Show hidden files
  "panel.hideHidden": "隠しファイルを非表示", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "新規ファイル", // New File
  "files.newFolder": "新規フォルダ", // New Folder
  "files.nameLabel": "名前", // Name
  "files.newTooltip": "新規ファイル / フォルダ", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.dblClickOpen": "ダブルクリックで開く",
  "files.deleteConfirm": (name) =>
    `「${name}」を削除しますか？この操作は元に戻せません。`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "アップロード", // Uploads
  "transfer.download": "ダウンロード", // Download
  "transfer.upload": "ファイルをアップロード…", // Upload Files…
  "transfer.uploadTooltip": "このフォルダーにファイルをアップロード", // Upload files to this folder
  "transfer.clear": "クリア", // Clear
  "transfer.cancelled": "キャンセル済み", // Cancelled
  "transfer.failed": "失敗", // Failed
  "transfer.stalled": "再接続中…", // Reconnecting…
  "transfer.foldersUnsupported": "フォルダーはアップロードできません。", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => `${n} セッション`, // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `クリックでサイドバーを「${label}」のみに絞り込み（再クリックで解除）`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `バックグラウンド ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `バックグラウンド常駐タブ数（上限 ${max}。超過時は最も古い非アクティブなタブを自動終了）`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `バックグラウンドタブを終了しました: ${name}（上限超過）`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `ブラウザリモートアクセス有効: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "権限：確認", // Perms: Ask
  "statusbar.permSkip": "権限：スキップ", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "このセッションの権限モード · クリックで切り替え（このセッションのみ）", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "このセッションの権限", // This session's permissions
  "statusbar.permOptAsk": "毎回確認（既定）", // Ask each time (default)
  "statusbar.permScopeHint":
    "現在のセッションにのみ適用されます。全体の既定値は「設定 ▸ エージェント」で調整できます。", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "権限を変更しました。反映にはこのセッションの再起動が必要です。再起動すると現在の会話は継続されますが、進行中のタスクは中断されます。今すぐ再起動しますか？", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "今すぐ再起動", // Restart now
  "statusbar.permRestartLater": "後で", // Later
  "statusbar.permScopeTitle": "適用先は？", // Apply to?
  "statusbar.permScopeSession": "このセッションのみ", // This session only
  "statusbar.permScopeGlobal": "グローバル既定", // Global default
  "statusbar.permScopeGlobalHint":
    "このセッションに即時適用し、今後新規作成する同種セッションの既定値になります（設定と同期）。", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ 処理中…", // ⏳ Working…
  "notify.asking": "❓ 確認が必要です", // ❓ Needs your confirmation
  "notify.waiting": "✅ 返信済み", // ✅ Replied
  "store.subtask": "サブタスク", // Subtask
  "store.splitPane": "分割", // Split
  "export.failedTitle": "セッションのエクスポートに失敗", // Failed to export session
  "export.contextSuffix": "コンテキスト", // context

  // ── Error panel ──
  "err.renderTitle": "レンダリングエラー", // Rendering Error
  "err.renderDesc":
    "予期しないエラーが発生しました。以下の情報が問題の特定に役立ちます。", // An unexpected error occurred…
  "err.reload": "再読み込み", // Reload
  "err.uncaughtTitle": "未捕捉のエラーが発生", // Uncaught Error
  "err.uncaughtDesc": "以下の情報が問題の特定に役立ちます。", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "ブラウザでは録画再生はまだサポートされていません", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `画像のアップロードに失敗 (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.showPassword": "表示",
  "login.hidePassword": "非表示",
  "login.passwordSaveFailed": "接続しましたが、パスワードをこの端末に保存できませんでした。もう一度お試しください。",
  "login.connecting": "接続中…", // Connecting…
  "login.remoteAccess": "リモートアクセス", // Remote Access
  "login.desc":
    "このターミナルに接続するにはアクセスパスワードを入力してください。", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "アクセスパスワード", // Access password
  "login.connect": "接続", // Connect
  "login.wrongPassword": "パスワードが違います", // Wrong password
  "login.rateLimited":
    "試行回数が多すぎます。1分ほど待ってから再度お試しください。", // Too many attempts. Please wait a minute and try again.
  "login.failed": "ログインに失敗しました。もう一度お試しください", // Login failed, please try again
  "login.pairingRequired":
    "このサーバーはペアリングリンクが必要です。デスクトップアプリの「リモートアクセス」で生成したリンクを開いてください。", // This server requires a pairing link
  "login.authFailed":
    "認証に失敗しました。アクセスパスワードを確認してください。リンクを再生成した場合は新しいペアリングリンクを使用してください。", // Authentication failed, check password or use a new pairing link
  "dir.title": "プロジェクトディレクトリを選択", // Choose Project Directory
  "dir.pathPlaceholder":
    "検索、またはパスを入力して Enter で移動（~ 始まりに対応）", // Search, or type a path and press Enter (supports ~)
  "dir.up": "一つ上へ", // Up one level
  "dir.newFolder": "新しいフォルダ", // New Folder
  "dir.newFolderPlaceholder": "フォルダ名", // Folder name
  "dir.goInput": "入力したパスへ移動", // Go to typed path
  "dir.noSubdirs": "（サブディレクトリなし）", // (no subdirectories)
  "dir.empty": "（空のフォルダ）", // (empty folder)
  "dir.noMatch": "一致する項目がありません", // No matching items
  "dir.target": "対象フォルダ", // Target
  "dir.showHidden": "隠しファイルを表示", // Show hidden items
  "dir.importing": "インポート中…", // Importing…
  "dir.choose": "このディレクトリを選択", // Choose This Directory
  "conn.reconnecting": "接続が切断されました。再接続しています…", // Connection lost, reconnecting…
  "conn.reconnectNow": "今すぐ再接続", // Reconnect now
  "conn.retrying": "再接続しています…", // Reconnecting…
  "conn.sshReconnecting":
    "SSH接続が切断されました。トンネルを再構築しています…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown":
    "SSH接続が切断されました。「今すぐ再接続」を押して再試行してください", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "リクエストに失敗しました", // Request failed
  "reqerr.dismiss": "閉じる", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "エラーログ", // Error Log
  "errlog.empty": "記録されたエラーはありません。", // No errors recorded.
  "errlog.copyAll": "すべてコピー", // Copy all
  "errlog.clear": "クリア", // Clear
  "errlog.close": "閉じる", // Close

  // ── Mobile ──
  "mobile.backConnections": "接続一覧に戻る",
  "mobile.loadSlow": "読み込みに時間がかかっています。再試行するか、接続一覧に戻れます。",
  "mobile.connectionUnavailable": "接続できません",
  "mobile.pushTitle": "タスクの通知",
  "mobile.pushHint": "アプリの使用中、バックグラウンド、画面ロック中に、セッション名と返信の短いプレビューを通知します。このテキストは velaterm.com とプッシュ通知サービスに送信されます。接続用パスワードや SSH 秘密鍵は送信されません。",
  "mobile.pushEnable": "通知を有効にする",
  "mobile.pushDisable": "通知を無効にする",
  "mobile.pushTest": "テスト通知を送信",
  "mobile.pushTestSent": "テスト通知を送信待ちに追加しました。システムの通知センターをご確認ください。",
  "mobile.pushDisabled": "バックグラウンド通知は無効です。",
  "mobile.pushEnabled": "バックグラウンド通知は有効です。",
  "mobile.pushNotConfigured": "このビルドにはプッシュ通知サービスが設定されていません。",
  "mobile.pushDenied": "システム設定で通知を許可してください。",
  "mobile.pushRegistrationFailed": "デバイスの登録に失敗しました。もう一度お試しください。",
  "mobile.pushRelayUnavailable": "通知の中継サービスを利用できません。もう一度お試しください。",
  "mobile.pushHostUnavailable": "接続先でバックグラウンド通知が有効になっていません。接続先を更新して再接続してください。",
  "mobile.pushDisclosure": "バックグラウンド通知には、Getui と端末メーカーのプッシュ通知サービスを使用します。通知の配信に必要な端末識別子、ネットワーク情報、セッション名、返信の短いプレビューが処理されます。接続用パスワードや SSH 秘密鍵は送信されません。",
  "mobile.pushConnectHint": "通知を有効にした後、通知を受け取りたい接続をそれぞれ一度開いてください。",
  "mobile.pushTarget": "テストする接続",
  "mobile.copyConnection": "コピーして編集",
  "mobile.copyConnectionHint": "この接続をもとに設定を編集します。保存済みの認証情報は安全に引き継がれます。元の接続は変更されず、設定が同じ場合は既存の接続を使用します。",
  "mobile.copyConnectionReused": "この設定は保存済みです。既存の接続をそのまま使用します。",
  "mobile.inputOptions": "入力オプション",
  "mobile.connections": "接続管理",
  "mobile.more": "その他の操作",
  "mobile.toDesktop": "デスクトップ版に切り替え", // Switch to desktop
  "mobile.empty1": "セッションがありません。", // No sessions.
  "mobile.noMatch": "一致するセッションがありません", // No matching sessions
  "mobile.empty2":
    "デスクトップ版か PC ブラウザで作成すると、ここに自動で表示されます。", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ 戻る", // ‹ Back
  "mobile.selCopy": "コピー", // Copy
  "mobile.selCancel": "キャンセル", // Cancel

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
  "splitter.dragToResize": "ドラッグでサイズを調整", // Drag to resize
  "transport.wsDisconnected": "WebSocket が切断されました", // WebSocket disconnected
  "transport.wsConnectFailed": "WebSocket 接続に失敗しました", // WebSocket connection failed
  "transport.cmdFailed": "コマンドが失敗しました", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `このコマンドはリモートクライアントでは利用できません: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `この設定キーはリモートクライアントからは書き込めません: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `リモートクライアントはアプリのデータディレクトリ内のファイルにアクセスできません: ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe (built-in WYSIWYG editor UI) ──
  "crepe.placeholder": "本文を入力するか、/ で挿入メニューを開きます", // Type text, or press / for the insert menu
  "crepe.textGroup": "テキスト", // Text
  "crepe.paragraph": "本文", // Text
  "crepe.h1": "見出し 1", // Heading 1
  "crepe.h2": "見出し 2", // Heading 2
  "crepe.h3": "見出し 3", // Heading 3
  "crepe.h4": "見出し 4", // Heading 4
  "crepe.h5": "見出し 5", // Heading 5
  "crepe.h6": "見出し 6", // Heading 6
  "crepe.quote": "引用", // Quote
  "crepe.divider": "区切り線", // Divider
  "crepe.listGroup": "リスト", // List
  "crepe.bulletList": "箇条書き", // Bullet List
  "crepe.orderedList": "番号付きリスト", // Ordered List
  "crepe.taskList": "タスクリスト", // Task List
  "crepe.advancedGroup": "挿入", // Insert
  "crepe.image": "画像", // Image
  "crepe.codeBlock": "コードブロック", // Code Block
  "crepe.table": "表", // Table
  "crepe.math": "数式", // Math
  "crepe.linkPlaceholder": "リンクを貼り付けまたは入力…", // Paste or type a link…
  "crepe.upload": "アップロード", // Upload
  "crepe.uploadImage": "画像をアップロード", // Upload Image
  "crepe.orPasteImageLink": "または画像リンクを貼り付け", // or paste an image link
  "crepe.imageCaption": "画像の説明", // Image caption
  "crepe.confirm": "確認", // Confirm
  "crepe.searchLanguage": "言語を検索", // Search language
  "crepe.noResult": "一致なし", // No results
  "crepe.edit": "編集", // Edit
  "crepe.collapse": "折りたたむ", // Collapse
  // ── Additional right and bottom bar entries ──
  "info.project": "プロジェクト", // Project
  "info.collection": "コレクション", // Collection
  "panel.sessionInfo": "セッション情報", // Session info
  "panel.gitTitle": "Git ステータス", // Git status
  "panel.gitProbing": "確認中…", // Checking…
  "panel.gitNotRepo": "Git リポジトリではありません", // Not a Git repository
  "panel.gitBranch": "ブランチ", // Branch
  "panel.gitStaged": "ステージ済み", // Staged
  "panel.gitUnstaged": "変更", // Changed
  "panel.gitUntracked": "未追跡", // Untracked
  "bottombar.running": "実行中", // Running
  "bottombar.collapseTasks": "タスク欄を閉じる", // Collapse tasks
  "bottombar.expandTasks": "タスク欄を開く", // Expand tasks
  "bottombar.sound": "🔔 サウンド", // 🔔 Sound
  "bottombar.muted": "🔕 ミュート", // 🔕 Muted
  "bottombar.overview": "セッション概要", // Sessions overview
  "bottombar.noSessions": "セッションがありません", // No sessions
  "doc.pdfFilter": "PDF ファイル", // PDF file
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

  // ── セッションビュー（エージェントのセッションを会話として読む） ──
  "session.showConversation": "会話ビュー",
  "session.showTerminal": "ターミナルビュー",
  "session.switchTitle": "ビューを切り替えるとエージェントが再起動します",
  "session.switchBody": "進行中のターンは中断されます。会話の内容は残ります。",
  "session.switchConfirm": "切り替える",
  "session.terminalViewHint": "ここをクリックすると、ターミナルビューに戻れます。",
  "session.loading": "会話を読み込んでいます…",
  "session.unavailable": "この会話はまだ読み取れません",
  "session.working": "実行中…",
  "session.thinking": "思考",
  "session.toolRunning": "実行中",
  "session.toolUnknown": "ツール",
  "session.toolFailed": "失敗しました",
  "session.toolNoDetail": "これ以上の記録はありません",
  "session.showMore": (n: number) => `残り ${n} 文字を表示`,
  "session.showLess": "折りたたむ",
  "session.composerHint": "エージェントに送信 · Enter で送信、Shift+Enter で改行",
  "session.send": "送信",

  // ── チャットエンジン（プロトコルで駆動するセッション） ──
  "chat.empty": "下の入力欄から会話を開始できます。",
  "chat.interrupt": "停止",
  "chat.interruptTooltip": "停止 · Esc",
  "chat.allow": "許可",
  "chat.deny": "拒否",
  "chat.permissionAsk": (tool: string) => `${tool} の実行許可を求めています`,
  "chat.exited": (code: number) => `エージェントが終了しました（コード ${code}）`,
  "chat.modeNextTurn": "次のターンから適用",
  "chat.modePendingHint": (current: string, next: string) =>
    `現在の権限：${current}。${next} は次のターンから適用されます。現在のターンは元の権限で続行します。`,
  "chat.modeTooltip": "許可モード",
  "chat.collaborationModeTooltip": "協働モード",
  "chat.collaborationMode.default": "デフォルト",
  "chat.collaborationMode.defaultHint": "作業を進め、判断が必要な場合のみ質問します",
  "chat.collaborationMode.plan": "プラン",
  "chat.collaborationMode.planHint": "調査して計画を作成し、質問には選択式カードを使用できます",
  "chat.moreOptions": "その他",
  "chat.modelTooltip": "モデル",
  "chat.keepChoice": "既定にする",
  "chat.keepChoiceFor": (model) => `${model} の既定にする`,
  "chat.followModelDefault": (agent: string) => `${agent} の既定設定を使用`,
  "chat.followModelDefaultHint": "エージェントの設定に基づくモデルを使用します。",
  "chat.savedModelDefault": "アプリの既定",
  "chat.catalogWebsite": "ウェブサイトのモデル一覧",
  "chat.catalogCache": "キャッシュ済みのモデル一覧",
  "chat.catalogBundled": "内蔵のモデル一覧",
  "chat.catalogChecked": (time: string) => `最終確認：${time}`,
  "chat.catalogFailed": "更新に失敗しました。既存の一覧は引き続き使用できます。",
  "chat.catalogRefresh": "更新",
  "chat.modelDefault": "既定のモデル",
  "chat.mode.default": "毎回確認",
  "chat.mode.agentDefault": "エージェント既定",
  "chat.mode.acceptEdits": "編集を自動承認",
  "chat.mode.plan": "計画モード",
  "chat.permissionRestart.unconfirmed": "接続が切れたため、権限の切り替え結果を確認できません。再接続して、会話の現在の権限を確認してください。",
  "permission.stateUnavailable": "権限の状態を取得できません",
  "permission.currentUnknown": "現在の権限は未確認",
  "permission.notRunning": "停止中",
  "permission.applied": "適用済み",
  "permission.nextTurn": "次のメッセージから適用",
  "permission.restart": "このセッションの再起動後に適用",
  "permission.nextStart": "次回起動時に適用",
  "permission.defaultHint": "新しく作成するセッションの既定の権限です。既存のセッションの権限設定は変わりません。",
  "chat.permissionRestart.title": "再起動して確認をスキップしますか？",
  "chat.permissionRestart.body": "確認をスキップするには Claude の再起動が必要です。現在の応答は中断されますが、会話履歴は保持されます。切り替えが成功すると、権限の確認が省略されます。",
  "chat.permissionRestart.confirm": "再起動して適用",
  "chat.permissionRestart.busy": "再起動中…",
  "chat.permissionRestart.failed": (detail: string) => "権限の切り替えに失敗しました。元の権限モードを保持しています。" + detail,
  "chat.permissionRestart.tasks": "キュー内のメッセージを処理または削除し、バックグラウンドタスクを停止してから再起動してください。",
  "chat.permissionRestart.stale": "会話のプロセスが変更されました。「確認なし」をもう一度選択してください。",
  "chat.permissionRestart.noHistory": "この会話はまだ再開できません。初期化が完了してから再試行してください。",
  "chat.mode.bypassPermissions": "確認なし",
  "chat.mode.readOnly": "読み取り専用",
  "chat.mode.fullAccess": "フルアクセス",
  "chat.placeholder": "エージェントに送信。/コマンド・/スキル・@ファイル も使えます",
  "chat.command.clearDescription": "このセッションをアーカイブして新しい会話を始める",
  "chat.command.rewindDescription": "直近のユーザーメッセージから巻き戻す範囲を選ぶ",
  "chat.command.rewindUnavailable":
    "完了済みのユーザーメッセージがあり、処理中のターン、待機中のメッセージ、権限確認がない場合に巻き戻せます。",
  "chat.effortTooltip": "思考の深さ",
  "chat.effortDefault": "思考",
  "chat.effort.auto": "自動",
  "chat.effort.low": "低",
  "chat.effort.medium": "中",
  "chat.effort.high": "高",
  "chat.effort.xhigh": "特高",
  "chat.effort.max": "最大",
  "chat.effort.ultra": "極限",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "エージェント",
  "chat.effort.minimal": "最小",
  "chat.filterPlaceholder": "絞り込み",
  "chat.placeholderOpencode": "エージェントに送信。/コマンド・@ファイル が使えます。! で始めるとシェルコマンドを実行します",
  "chat.command.compactDescription": "会話を要約してコンテキストを空ける",
  "chat.command.undoDescription": "最後のメッセージと、それによるファイル変更を取り消す",
  "chat.command.redoDescription": "直前の取り消しを元に戻す",
  "chat.command.shareDescription": "この会話の共有リンクを作成する",
  "chat.command.unshareDescription": "この会話の共有を停止する",
  "chat.mode.auto": "自動判定",

  // ── エージェントからの質問にフォームで答える ──
  "chat.question.heading": "エージェントからの質問",
  "chat.question.submit": "送信",
  "chat.question.next": "次へ",
  "chat.question.dismiss": "閉じる",
  "chat.question.answerPlaceholder": "回答を入力",
  "chat.question.otherPlaceholder": "その他の回答",
  "chat.question.answeredHeading": (n: number) => `${n} 件の質問に回答済み`,
  "chat.question.blankAnswer": "未記入",

  // ── 承認待ちのプラン ──
  "chat.plan.heading": "プランの承認待ちです",
  "chat.plan.implement": "承認して実行",
  "chat.plan.reject": "却下",

  // ── エージェントの作業中に入力したメッセージ ──
  "chat.placeholderBusy": "メッセージを入力してください。このターンの終了後に送信されます",
  "chat.queueTooltip": (combo: string) => `このターンの終了後に送信 · ${combo} で今すぐ送信`,
  "chat.queue.pending": "送信待ち",
  "chat.queue.view": "メッセージ全体を表示",
  "chat.queue.edit": "編集",
  "chat.queue.remove": "削除",

  // ── 入力欄に貼り付け・ドロップした画像 ──
  "chat.attach.remove": "この画像を削除",
  "chat.attach.tooMany": (max: number) => `1 通のメッセージに添付できる画像は ${max} 枚までです`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} は ${mb} MB を超えるため添付しませんでした`,
  "chat.attach.unreadable": (name: string) => `${name} を読み込めませんでした`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "コンテキストを圧縮しています…",
  "chat.compaction.manual": "コンテキストを圧縮しました",
  "chat.compaction.auto": "コンテキストを自動で圧縮しました",
  "chat.compaction.from": (tokens: string) => `圧縮前 ${tokens} トークン`,
  // N steps
  "chat.subagent.steps": (n: number) => `${n} ステップ`,
  "chat.subagent.tokens": (tokens: string) => `${tokens}トークン`,
  "chat.rewind.edit": "編集",
  "chat.rewind.editSend": "確認して再送信",
  "chat.rewind.editConfirm": "削除して再送信",
  "chat.rewind.editWarning": "元のメッセージとそれ以降のすべてのメッセージが完全に削除され、編集した内容がこの位置から送信されます。ファイルの変更は元に戻りません。",
  "chat.rewind.inactive": "会話のプロセスが起動していません。起動後にこれらの操作を利用できます。",
  "chat.rewind.unsupported": "接続中のエージェントは現在この操作に対応していません。",
  "chat.rewind.title": "ここから巻き戻す",
  "chat.rewind.warning": "この操作は元に戻せません。",
  "chat.rewind.conversation": "会話を巻き戻す",
  "chat.rewind.files": "ファイルを復元する",
  "chat.rewind.both": "会話を巻き戻してファイルを復元する",
  "chat.rewind.confirm.conversation": "このメッセージ以降をすべて削除しますか？",
  "chat.rewind.confirm.files": "ファイルをこのメッセージ以前の状態に復元しますか？",
  "chat.rewind.confirm.both": "このターンを削除し、変更されたファイルも復元しますか？",
  "chat.rewind.unavailable": "このメッセージに対応するファイルのチェックポイントはありません。",
  "chat.rewind.previewing": "ファイルのチェックポイントを確認しています…",
  "chat.rewind.cancel": "変更しない",
  "chat.rewind.apply": "巻き戻す",
  "chat.rewind.applying": "巻き戻しています…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `${files} 件のファイルが変更されます：+${insertions} −${deletions}。この操作は元に戻せません。`,
  // ── 許可カードが提示する常設ルール。1 回押すだけで採用される ──
  "chat.suggest.modeSession": (mode: string) => `このセッションは${mode}にする`,
  "chat.suggest.mode": (mode: string) => `${mode}に切り替える`,
  "chat.suggest.allowSession": (rule: string) => `このセッションでは ${rule} を許可`,
  "chat.suggest.allowAlways": (rule: string) => `${rule} を常に許可`,
  "chat.suggest.dirSession": (dirs: string) => `このセッションでは ${dirs} へのアクセスを許可`,
  "chat.suggest.dirAlways": (dirs: string) => `${dirs} へのアクセスを常に許可`,
  // ── Codex：ネットワーク許可ルール、割り込み、独自コマンド、速度と口調のチップ ──
  "chat.suggest.networkAlways": (host: string) => `${host} へのネットワークアクセスを常に許可`,
  "chat.steer": "指示を追加",
  "chat.stopping": "現在のターンを停止中…",
  "chat.stopped": "現在のターンを停止しました",
  "chat.steerAccepted": "追加の指示を送信しました",
  "chat.steerTooltip": (combo: string) => `${combo} で実行中のターンに追加`,
  "chat.command.reviewDescription": "コードをレビューして対応が必要な箇所を報告",
  "chat.command.reviewHint": "[branch <ブランチ名> | commit <コミット ID> | 指示]",
  "chat.command.startTimeout": "エージェントが時間内にセッションを開始できませんでした",
  "chat.serviceTierTooltip": "速度",
  "chat.serviceTier.default": "標準速度",
  "chat.personalityTooltip": "口調",
  "chat.personality.default": "既定の口調",
  "chat.personality.none": "中立",
  "chat.personality.friendly": "フレンドリー",
  "chat.personality.pragmatic": "実務的",
  // ── 長い会話：連続したツール呼び出しを 1 行にまとめ、末尾へ戻る導線を置く ──
  "chat.toolRun.count": (n: number) => `ツール呼び出し ${n} 件`,
  "chat.toolRun.tooltip": "1 件ずつ表示",
  "chat.backToEnd": "最新のメッセージに戻る",
  "chat.turnFold.hide": "途中経過を隠す",
  "chat.turnFold.show": (n: number) => `途中経過を表示（${n} ステップ）`,
  "chat.turnFold.hideAll": "すべての途中経過を隠す",
  "chat.turnFold.showAll": "すべての途中経過を表示",
  "chat.elicitation.heading": (server: string) => `${server} が入力を求めています`,
  "chat.elicitation.cancel": "キャンセル",
  "chat.elicitation.decline": "拒否",
  "chat.elicitation.submit": "送信",
  "chat.elicitation.done": "完了",
  "chat.elicitation.choose": "選択…",
  "chat.effort.off": "オフ",
  "chat.effort.offHint": "拡張思考を行わない",
  "chat.fastMode.label": "高速",
  "chat.fastMode.on": "高速モードはオンです",
  "chat.fastMode.off": "高速モードはオフです",
  "chat.auth.login": "ログイン",
  "chat.auth.logout": "ログアウト",
  "chat.auth.confirmLogout": "ログアウトを確定",
  "chat.auth.logoutConfirm": (provider: string) => `このホストの ${provider} アカウントからログアウトしますか？共有の認証情報が削除され、それを使用する他のセッションにも影響します。会話履歴は保持されます。`,
  "chat.auth.signingOut": "ログアウトしています…",
  "chat.auth.signedOut": (provider: string) => `${provider} からログアウトしました。ログインすると、この会話を続けられます。`,
  "chat.auth.logoutFailed": "ログアウトの結果を確認できませんでした。もう一度お試しください。",
  "chat.auth.wait": "現在の処理が完了してからアカウントを切り替えてください。",
  "chat.auth.title": (provider: string) => `${provider} アカウント`,
  "chat.auth.start": "再ログイン",
  "chat.auth.required": (provider: string) => `${provider} の認証が無効になりました。再ログインして続行してください。`,
  "chat.auth.starting": "ログインを準備しています…",
  "chat.auth.pending": "認証ページを開き、このコードを入力してください。ログインが完了すると、この画面が自動的に更新されます。",
  "chat.auth.success": "ログインしました。メッセージを送信して、この会話を続けられます。",
  "chat.auth.failed": "ログインを完了できませんでした。もう一度お試しください。ChatGPT でデバイスコード認証を有効にし、対応する Codex CLI を使用していることを確認してください。",
  "chat.auth.canceled": "ログインをキャンセルしました。いつでも再試行できます。",
  "chat.auth.scope": (provider: string) => `このホストで使用する ${provider} アカウントが更新されます。同じ認証情報を共有する他のセッションでも、そのアカウントが使用されます。`,
  "chat.auth.canceling": "ログインをキャンセルしています…",
  "chat.auth.submitting": "認証コードを確認しています…",
  "chat.auth.claude.pending": "認証ページを開いてログインし、表示されたコードを省略せずに貼り付けてください。",
  "chat.auth.claude.failed": "ログインを完了できませんでした。Claude CLI がアカウント認証に対応していることを確認し、もう一度お試しください。",
  "chat.auth.claude.code": "認証コード",
  "chat.auth.claude.submit": "コードを送信",
  "chat.auth.claude.invalidCode": "今回の認証ページに表示されたコードを、# 以降も含めて貼り付けてください。",
  "chat.auth.claude.externalAuth": "API キーやその他の設定済みの認証方法は変更されません。",
  "chat.auth.open": "認証ページを開く",
  "chat.resetCredits.label": (n: string) => `リセット券：${n} 枚`,
  "chat.resetCredits.title": "Codex 利用上限のリセット券",
  "chat.resetCredits.unknown": "リセット券の枚数を取得できません。",
  "chat.resetCredits.confirm": "リセット券を 1 枚使って、対象となる Codex の利用上限をリセットします。この操作は取り消せません。",
  "chat.resetCredits.reset": "利用上限をリセットしました。",
  "chat.resetCredits.alreadyRedeemed": "このリクエストはすでに成功しています。",
  "chat.resetCredits.nothingToReset": "リセット対象の利用上限はありません。",
  "chat.resetCredits.noCredit": "利用できるリセット券はありません。",
  "chat.resetCredits.error": "リクエストに失敗したか、最新の残高を取得できません。残高を更新するか、結果が未確認のリセットを再試行してください。",
  "chat.resetCredits.busy": "処理中…",
  "chat.resetCredits.retry": "リセットを再試行",
  "chat.resetCredits.use": "1 枚使う",
  "chat.resetCredits.refresh": "更新",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `コンテキスト: ${max} トークン中 ${used} を使用（${pct}%）`,
  "chat.usage.cost": (usd: string) => `セッションの費用: $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `利用上限に達しました。リセット: ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) => `利用上限: ${pct}% 使用済み。リセット: ${resets}`,
  "chat.autoContinue.fiveHour": (time: string) => `5 時間の利用上限に達しました。${time} にタスクを自動的に再開します。`, // 5-hour usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.weekly": (time: string) => `週間の利用上限に達しました。${time} にタスクを自動的に再開します。`, // Weekly usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.generic": (time: string) => `利用上限に達しました。${time} にタスクを自動的に再開します。`, // Usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.unknownReset": "利用上限に達しました。リセット時刻を取得できないため、タスクは自動的に再開されません。", // Usage limit reached. The reset time is unknown, so the task will not continue automatically.
  "chat.autoContinue.repeated": "再び利用上限に達したため、タスクの自動再開を停止しました。", // The usage limit was reached again. The task will no longer continue automatically.
  "chat.autoContinue.failed": "タスクを自動的に再開できませんでした。メッセージを送信すると再開できます。", // The task could not continue automatically. Send a message to continue.
  "chat.mcp.codexScope": "Codex のユーザー設定を変更します。この設定を使用する他のセッションにも影響します。続行しますか？",
  "chat.mcp.tooltip": "MCP サーバー",
  "chat.mcp.loading": "サーバー一覧を読み込んでいます…",
  "chat.mcp.backendUnsupported": "接続先の VelaTerm バックエンドは MCP の管理に対応していません。バックエンドを更新して再起動し、もう一度お試しください。",
  "chat.mcp.none": "MCP サーバーは設定されていません",
  "chat.mcp.tools": (n: number) => `${n} 個のツール`,
  "chat.mcp.reconnect": "再接続",
  "chat.mcp.disable": "無効にする",
  "chat.mcp.enable": "有効にする",
  "chat.mcp.status.connected": "接続済み",
  "chat.mcp.status.disabled": "無効",
  "chat.mcp.status.failed": "失敗",
  "chat.mcp.status.pending": "接続中",
  "chat.mcp.status.disconnected": "切断",
  "chat.mcp.status.other": "不明",
  "chat.tasks.label": "タスク",
  "chat.tasks.tooltip": "バックグラウンドタスク",
  "chat.tasks.backgroundAll": "実行中の作業をバックグラウンドに移す",
  "chat.tasks.none": "バックグラウンドタスクはありません",
  "chat.tasks.stop": "停止",
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `${seconds} 秒後に再試行します（${attempt}/${max}）: ${message}`,
  "chat.notify.dismiss": "閉じる",
  "settings.completionMode": "コマンドの補完候補",
  "settings.completionAuto": "自動表示",
  "settings.completionTab": "Tab で表示",
  "settings.completionOff": "オフ",
  "settings.completionUnavailable": "設定を読み込めないか、保存できませんでした。",
  "settings.completionHint": "新しい Zsh、Bash 4+、Fish、PowerShell のターミナルに適用されます。CMD では標準の Tab 動作を維持します。Tab で選択した候補を入力し、Enter で候補を適用せずに現在のコマンドを実行します。",


};

export default ja;
