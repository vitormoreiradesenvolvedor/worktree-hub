//! Interface translations. English is the default; the user picks the rest.
//!
//! The table is sorted by key so lookups are a binary search, and the current
//! language lives in an atomic: every widget can translate without carrying a
//! handle around.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Pt,
    Ru,
    Zh,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Pt, Lang::Ru, Lang::Zh];

    /// Each language names itself, whatever the interface is set to.
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Pt => "Português",
            Lang::Ru => "Русский",
            Lang::Zh => "中文",
        }
    }

    fn index(self) -> usize {
        match self {
            Lang::En => 0,
            Lang::Pt => 1,
            Lang::Ru => 2,
            Lang::Zh => 3,
        }
    }

    fn from_index(index: u8) -> Self {
        match index {
            1 => Lang::Pt,
            2 => Lang::Ru,
            3 => Lang::Zh,
            _ => Lang::En,
        }
    }

    /// True when this language needs glyphs no embedded font carries.
    pub fn needs_cjk(self) -> bool {
        self == Lang::Zh
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(lang: Lang) {
    CURRENT.store(lang.index() as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    Lang::from_index(CURRENT.load(Ordering::Relaxed))
}

/// Translation for the current language, falling back to English and then to
/// the key itself, so a missing entry is visible instead of silently empty.
pub fn tr(key: &str) -> &'static str {
    match TABLE.binary_search_by(|(k, _)| (*k).cmp(key)) {
        Ok(index) => {
            let row = TABLE[index].1;
            let text = row[lang().index()];
            if text.is_empty() { row[0] } else { text }
        }
        Err(_) => {
            debug_assert!(false, "missing translation key: {key}");
            Box::leak(key.to_owned().into_boxed_str())
        }
    }
}

/// Translation with one `{}` filled in.
pub fn tr1(key: &str, value: impl std::fmt::Display) -> String {
    tr(key).replacen("{}", &value.to_string(), 1)
}

/// Translation with every `{}` filled in, in order.
pub fn trn(key: &str, values: &[&str]) -> String {
    let mut out = tr(key).to_owned();
    for value in values {
        out = out.replacen("{}", value, 1);
    }
    out
}

type Row = (&'static str, [&'static str; 4]);

#[rustfmt::skip]
static TABLE: &[Row] = &[
    ("about.built_with", ["Built with", "Feito com", "Сделано на", "技术栈"]),
    ("about.contact", ["Contact", "Contato", "Контакт", "联系方式"]),
    ("about.developer", ["Developer", "Desenvolvedor", "Разработчик", "开发者"]),
    ("about.fonts", ["Embedded fonts under the SIL Open Font License 1.1", "Fontes embutidas sob a SIL Open Font License 1.1", "Встроенные шрифты под SIL Open Font License 1.1", "内置字体采用 SIL 开放字体许可证 1.1"]),
    ("about.license", ["License", "Licença", "Лицензия", "许可证"]),
    ("about.subtitle", ["The tool, and who makes it", "A ferramenta e quem faz ela", "Инструмент и кто его делает", "关于这个工具和作者"]),
    ("about.tagline", ["Graphical orchestration of git worktrees, with a real multiterminal built in.", "Orquestração gráfica de git worktrees, com multiterminal de verdade embutido.", "Графическое управление git worktrees со встроенным мультитерминалом.", "图形化管理 git worktrees，内置真正的多终端。"]),
    ("about.what", ["What it does", "O que ele faz", "Что он умеет", "功能"]),
    ("about.what_body", ["Lists, creates and removes worktrees of each repository, and opens one terminal per worktree in panes you arrange yourself — no Electron, no webview: a single binary.", "Lista, cria e remove worktrees de cada repositório e abre um terminal por worktree em painéis que você mesmo organiza — sem Electron, sem webview: um binário só.", "Показывает, создаёт и удаляет рабочие деревья и открывает по терминалу на каждое — без Electron и webview, один бинарник.", "列出、创建和删除每个仓库的工作树，并为每个工作树打开一个终端面板——没有 Electron，没有 webview，只有一个可执行文件。"]),
    ("accent.blue", ["Dark blue", "Azul escuro", "Тёмно-синий", "深蓝"]),
    ("accent.gray", ["Gray", "Cinza", "Серый", "灰色"]),
    ("accent.green", ["Stone green", "Verde pedra", "Зелёный камень", "石绿"]),
    ("accent.purple", ["Purple", "Roxo", "Фиолетовый", "紫色"]),
    ("accent.yellow", ["Yellow", "Amarelo", "Жёлтый", "黄色"]),
    ("dlg.add_project_sub", ["Choose the folder of the main git repository", "Escolha a pasta do repositório git principal", "Выберите папку основного git-репозитория", "选择主 git 仓库的文件夹"]),
    ("dlg.add_project_title", ["Add project", "Adicionar projeto", "Добавить проект", "添加项目"]),
    ("dlg.advanced", ["Advanced options", "Opções avançadas", "Дополнительно", "高级选项"]),
    ("dlg.base", ["From", "A partir de", "От", "基于"]),
    ("dlg.branch", ["Branch", "Branch", "Ветка", "分支"]),
    ("dlg.cancel", ["Cancel", "Cancelar", "Отмена", "取消"]),
    ("dlg.choose", ["choose", "escolher", "выбрать", "选择"]),
    ("dlg.create_branch", ["Create new branch", "Criar branch nova", "Создать новую ветку", "创建新分支"]),
    ("dlg.create_branch_hint", ["Off: checks out a branch that already exists", "Desligado: faz checkout de uma branch que já existe", "Выключено: переключается на существующую ветку", "关闭时：检出已存在的分支"]),
    ("dlg.create_sub", ["Creates a working folder linked to this repository", "Cria uma pasta de trabalho ligada a este repositório", "Создаёт рабочую папку, связанную с этим репозиторием", "创建一个与该仓库关联的工作文件夹"]),
    ("dlg.create_title", ["Create worktree", "Criar worktree", "Создать рабочее дерево", "创建工作树"]),
    ("dlg.delete_branch", ["Delete the branch too", "Apagar a branch também", "Удалить и ветку", "同时删除分支"]),
    ("dlg.delete_branch_hint", ["Runs git branch -D after removing the folder", "Roda git branch -D depois de remover a pasta", "Выполняет git branch -D после удаления папки", "删除文件夹后执行 git branch -D"]),
    ("dlg.delete_force", ["Force removal", "Forçar remoção", "Принудительное удаление", "强制删除"]),
    ("dlg.delete_force_hint", ["Needed when the worktree has changes", "Necessário quando o worktree tem alterações", "Нужно, если в рабочем дереве есть изменения", "工作树有改动时需要"]),
    ("dlg.delete_sub", ["The folder leaves the disk; the repository history stays intact", "A pasta sai do disco; o histórico do repositório continua intacto", "Папка исчезнет с диска; история репозитория не пострадает", "文件夹会从磁盘移除；仓库历史保持不变"]),
    ("dlg.delete_title", ["Remove \"{}\"", "Remover \"{}\"", "Удалить «{}»", "删除“{}”"]),
    ("dlg.detach", ["Detached HEAD", "Detached HEAD", "Detached HEAD", "分离 HEAD"]),
    ("dlg.detach_hint", ["Checks the commit out without a branch", "Faz checkout do commit sem branch", "Переключается на коммит без ветки", "不使用分支直接检出提交"]),
    ("dlg.editor_none", ["No code editor found — set one in Settings", "Nenhum editor de código encontrado — defina um em Ajustes", "Редактор кода не найден — укажите его в настройках", "没有找到代码编辑器 — 请在设置中指定"]),
    ("dlg.editor_remember", ["Always use this one", "Usar sempre este", "Всегда использовать этот", "总是使用这个"]),
    ("dlg.editor_remember_hint", ["Saves it as the editor command; clear the field in Settings to be asked again", "Salva como o comando do editor; limpe o campo em Ajustes para escolher de novo", "Сохранит его как команду редактора; очистите поле в настройках, чтобы снова выбирать", "保存为编辑器命令；在设置中清空该字段可恢复每次询问"]),
    ("dlg.editor_sub", ["Pick which application opens this worktree", "Escolha qual aplicativo abre esta worktree", "Выберите, какое приложение откроет это рабочее дерево", "选择用哪个应用打开这个工作树"]),
    ("dlg.editor_title", ["Open in editor", "Abrir no editor", "Открыть в редакторе", "在编辑器中打开"]),
    ("dlg.folder_missing", ["folder not found", "pasta não encontrada", "папка не найдена", "找不到文件夹"]),
    ("dlg.force", ["Force", "Forçar", "Принудительно", "强制"]),
    ("dlg.force_branch", ["Overwrite existing branch", "Sobrescrever branch existente", "Перезаписать существующую ветку", "覆盖已存在的分支"]),
    ("dlg.force_branch_hint", ["Uses -B instead of -b", "Usa -B em vez de -b", "Использует -B вместо -b", "使用 -B 代替 -b"]),
    ("dlg.force_hint", ["Reuses the folder and ignores git warnings", "Reaproveita a pasta e ignora avisos do git", "Повторно использует папку и игнорирует предупреждения git", "复用文件夹并忽略 git 警告"]),
    ("dlg.git_repo", ["git repository", "repositório git", "git-репозиторий", "git 仓库"]),
    ("dlg.home", ["Home folder", "Pasta pessoal", "Домашняя папка", "主文件夹"]),
    ("dlg.is_repo", ["this folder is a git repository", "esta pasta é um repositório git", "это git-репозиторий", "该文件夹是 git 仓库"]),
    ("dlg.no_checkout", ["No checkout", "Sem checkout", "Без checkout", "不检出"]),
    ("dlg.no_checkout_hint", ["Creates the folder empty (--no-checkout)", "Cria a pasta vazia (--no-checkout)", "Создаёт пустую папку (--no-checkout)", "创建空文件夹 (--no-checkout)"]),
    ("dlg.no_subfolders", ["no subfolders here", "nenhuma subpasta aqui", "здесь нет подпапок", "这里没有子文件夹"]),
    ("dlg.not_repo", ["this folder is not a git repository", "esta pasta não é um repositório git", "это не git-репозиторий", "该文件夹不是 git 仓库"]),
    ("dlg.open_terminal", ["Open terminal after creating", "Abrir terminal ao criar", "Открыть терминал после создания", "创建后打开终端"]),
    ("dlg.path", ["Worktree folder", "Pasta do worktree", "Папка рабочего дерева", "工作树文件夹"]),
    ("dlg.quick_command", ["Command", "Comando", "Команда", "命令"]),
    ("dlg.quick_hint", ["Sent to every open pane, as if you had typed it and pressed Enter.", "Enviado para todos os painéis abertos, como se você tivesse digitado e dado Enter.", "Отправляется во все открытые панели, как будто вы набрали это и нажали Enter.", "发送到所有打开的面板，就像你输入后按下回车。"]),
    ("dlg.quick_label", ["Button name", "Nome do botão", "Название кнопки", "按钮名称"]),
    ("dlg.quick_sub", ["A button in the mirroring bar that sends this command to every pane", "Um botão na barra de espelhamento que manda este comando para todos os painéis", "Кнопка в панели дублирования, отправляющая команду во все панели", "镜像栏中的按钮，把这条命令发给所有面板"]),
    ("dlg.quick_title", ["Saved command", "Comando salvo", "Сохранённая команда", "保存的命令"]),
    ("dlg.remove", ["Remove", "Remover", "Удалить", "删除"]),
    ("dlg.remove_project_sub", ["Only the app forgets it: nothing is deleted from disk", "Só o app esquece: nada é apagado do disco", "Приложение просто забудет его: с диска ничего не удаляется", "只是应用不再记住它：磁盘上不会删除任何内容"]),
    ("dlg.remove_project_title", ["Remove project from the list", "Remover projeto da lista", "Убрать проект из списка", "从列表中移除项目"]),
    ("dlg.save", ["Save", "Salvar", "Сохранить", "保存"]),
    ("dlg.track", ["Track the origin", "Rastrear a origem", "Отслеживать origin", "跟踪远程分支"]),
    ("dlg.track_hint", ["Adds --track when creating the branch", "Adiciona --track à criação da branch", "Добавляет --track при создании ветки", "创建分支时加上 --track"]),
    ("dlg.up", ["Folder above", "Pasta acima", "На уровень выше", "上一级文件夹"]),
    ("dlg.use_folder", ["Use this folder", "Usar esta pasta", "Использовать эту папку", "使用此文件夹"]),
    ("layout.columns", ["Columns", "Colunas", "Колонки", "列"]),
    ("layout.free", ["Free", "Livre", "Свободно", "自由"]),
    ("layout.grid", ["Grid", "Grade", "Сетка", "网格"]),
    ("layout.rows", ["Rows", "Linhas", "Строки", "行"]),
    ("layout.tabs", ["Tabs", "Abas", "Вкладки", "标签页"]),
    ("msg.added", ["{} added", "{} adicionado", "{} добавлен", "已添加 {}"]),
    ("msg.bare", ["{} is a bare repository — worktrees live outside it", "{} é um repositório bare — worktrees ficam fora dele", "{} — bare-репозиторий, рабочие деревья живут вне его", "{} 是 bare 仓库 — 工作树位于其外部"]),
    ("msg.branch_deleted", [" (branch deleted)", " (branch apagada)", " (ветка удалена)", "（分支已删除）"]),
    ("msg.branch_kept", [" — branch kept: {}", " — branch mantida: {}", " — ветка сохранена: {}", " — 分支保留：{}"]),
    ("msg.branch_required", ["enter the branch name", "informe o nome da branch", "укажите имя ветки", "请输入分支名"]),
    ("msg.git_missing", ["git not found in PATH — install git to use the app", "git não encontrado no PATH — instale o git para usar o app", "git не найден в PATH — установите git, чтобы пользоваться приложением", "PATH 中找不到 git — 请先安装 git"]),
    ("msg.not_exist", ["{} does not exist", "{} não existe", "{} не существует", "{} 不存在"]),
    ("msg.nothing_to_clean", ["nothing to clean", "nada para limpar", "нечего очищать", "没有需要清理的"]),
    ("msg.path_copied", ["path copied", "caminho copiado", "путь скопирован", "已复制路径"]),
    ("msg.path_exists", ["{} already exists — tick \"force\" to reuse it", "{} já existe — marque \"forçar\" para reutilizar", "{} уже существует — отметьте «Принудительно», чтобы использовать повторно", "{} 已存在 — 勾选“强制”以复用"]),
    ("msg.pruned", ["orphan worktrees removed", "worktrees órfãos removidos", "потерянные рабочие деревья удалены", "已清除孤立的工作树"]),
    ("msg.removed", ["{} removed", "{} removido", "{} удалено", "已删除 {}"]),
    ("msg.wt_created", ["worktree created", "worktree criado", "рабочее дерево создано", "工作树已创建"]),
    ("msg.wt_created_at", ["worktree created at {}", "worktree criado em {}", "рабочее дерево создано в {}", "工作树已创建于 {}"]),
    ("msg.wt_locked", ["worktree locked", "worktree travado", "рабочее дерево заблокировано", "工作树已锁定"]),
    ("msg.wt_removed", ["worktree removed", "worktree removido", "рабочее дерево удалено", "工作树已删除"]),
    ("msg.wt_unlocked", ["worktree unlocked", "worktree destravado", "рабочее дерево разблокировано", "工作树已解锁"]),
    ("msg.wt_updated", ["worktree updated", "worktree atualizado", "рабочее дерево обновлено", "工作树已更新"]),
    ("nav.about", ["About", "Sobre", "О программе", "关于"]),
    ("nav.add_project", ["Add project", "Adicionar projeto", "Добавить проект", "添加项目"]),
    ("nav.collapse", ["Collapse the sidebar (F9)", "Recolher a barra lateral (F9)", "Свернуть боковую панель (F9)", "折叠侧边栏 (F9)"]),
    ("nav.expand", ["Open the sidebar (F9)", "Abrir a barra lateral (F9)", "Открыть боковую панель (F9)", "展开侧边栏 (F9)"]),
    ("nav.no_projects", ["no repositories yet", "nenhum repositório ainda", "репозиториев пока нет", "还没有仓库"]),
    ("nav.open_folder", ["Open folder", "Abrir pasta", "Открыть папку", "打开文件夹"]),
    ("nav.projects", ["Projects", "Projetos", "Проекты", "项目"]),
    ("nav.remove_project", ["Remove from the app", "Remover do app", "Убрать из приложения", "从应用中移除"]),
    ("nav.settings", ["Settings", "Ajustes", "Настройки", "设置"]),
    ("nav.terminals", ["Terminals", "Terminais", "Терминалы", "终端"]),
    ("nav.worktrees", ["Worktrees", "Worktrees", "Рабочие деревья", "工作树"]),
    ("sc.close_pane", ["close the focused pane", "fechar painel em foco", "закрыть активную панель", "关闭当前面板"]),
    ("sc.copy_paste", ["copy / paste in the terminal", "copiar / colar no terminal", "копировать / вставить в терминале", "在终端中复制 / 粘贴"]),
    ("sc.font", ["change the font size of the pane", "mudar o tamanho da fonte do painel", "изменить размер шрифта панели", "改变面板字号"]),
    ("sc.fullscreen", ["fullscreen", "tela cheia", "полный экран", "全屏"]),
    ("sc.mirror", ["mirror typing", "espelhar digitação", "дублировать ввод", "同步输入"]),
    ("sc.mouse", ["select text while a program uses the mouse", "selecionar texto quando o programa usa o mouse", "выделение текста, когда мышь занята программой", "程序占用鼠标时选择文本"]),
    ("sc.multi", ["open the multiterminal for the selection", "abrir multiterminal da seleção", "открыть мультитерминал для выбранных", "打开所选的多终端"]),
    ("sc.new", ["new worktree", "novo worktree", "новое рабочее дерево", "新建工作树"]),
    ("sc.refresh", ["refresh the list", "atualizar lista", "обновить список", "刷新列表"]),
    ("sc.screens", ["worktrees / terminals / settings / about", "worktrees / terminais / ajustes / sobre", "рабочие деревья / терминалы / настройки / о программе", "工作树 / 终端 / 设置 / 关于"]),
    ("sc.scroll", ["scroll the terminal history", "rolar histórico do terminal", "прокрутка истории терминала", "滚动终端历史"]),
    ("sc.sidebar", ["fold / open the sidebar", "recolher / abrir a barra lateral", "свернуть / открыть боковую панель", "折叠 / 展开侧边栏"]),
    ("sc.sigint", ["interrupt the program in the terminal (SIGINT)", "interrompe o programa do terminal (SIGINT)", "прервать программу в терминале (SIGINT)", "中断终端中的程序 (SIGINT)"]),
    ("set.about", ["About", "Sobre", "О программе", "关于"]),
    ("set.accent", ["Accent color", "Cor de destaque", "Акцентный цвет", "强调色"]),
    ("set.app_opacity", ["Window transparency", "Transparência da janela", "Прозрачность окна", "窗口透明度"]),
    ("set.appearance", ["Appearance", "Aparência", "Внешний вид", "外观"]),
    ("set.cjk_missing", ["no CJK font found on this system — Chinese may show as boxes", "nenhuma fonte CJK encontrada no sistema — o chinês pode aparecer como quadrados", "шрифт CJK в системе не найден — китайский может отображаться квадратами", "系统中未找到 CJK 字体 — 中文可能显示为方块"]),
    ("set.config", ["Configuration: ", "Configuração: ", "Конфигурация: ", "配置文件："]),
    ("set.config_file", ["Configuration file", "Arquivo de configuração", "Файл конфигурации", "配置文件"]),
    ("set.ctrl_c_hint", ["Ctrl+C, Ctrl+X and friends go to the running program, like in any terminal — copy and paste live on Ctrl+Shift+C and Ctrl+Shift+V.", "Ctrl+C, Ctrl+X e companhia vão para o programa que está rodando, como em qualquer terminal — copiar e colar ficam em Ctrl+Shift+C e Ctrl+Shift+V.", "Ctrl+C, Ctrl+X и подобные уходят в запущенную программу, как в любом терминале — копирование и вставка на Ctrl+Shift+C и Ctrl+Shift+V.", "Ctrl+C、Ctrl+X 等会发送给正在运行的程序，就像任何终端一样 — 复制和粘贴请用 Ctrl+Shift+C 和 Ctrl+Shift+V。"]),
    ("set.editor", ["Editor", "Editor", "Редактор", "编辑器"]),
    ("set.external", ["External applications", "Aplicativos externos", "Внешние приложения", "外部应用"]),
    ("set.external_hint", ["use {path} in place of the worktree path", "use {path} no lugar do caminho da worktree", "используйте {path} вместо пути рабочего дерева", "用 {path} 代表工作树路径"]),
    ("set.font_size", ["Font size", "Tamanho da fonte", "Размер шрифта", "字号"]),
    ("set.install", ["This copy", "Esta instalação", "Эта установка", "此安装"]),
    ("set.language", ["Language", "Idioma", "Язык", "语言"]),
    ("set.lines", [" lines", " linhas", " строк", " 行"]),
    ("set.mono_font", ["Terminal font", "Fonte do terminal", "Шрифт терминала", "终端字体"]),
    ("set.opacity_hint", ["needs a compositor; 100% is fully opaque", "precisa de compositor; 100% é opaco", "нужен композитор; 100% — непрозрачно", "需要合成器；100% 为完全不透明"]),
    ("set.open_config", ["Open the configuration folder", "Abrir pasta de configuração", "Открыть папку конфигурации", "打开配置文件夹"]),
    ("set.open_terminal", ["Open a terminal when creating a worktree", "Abrir terminal ao criar worktree", "Открывать терминал при создании рабочего дерева", "创建工作树时打开终端"]),
    ("set.open_terminal_hint", ["Leaves a pane ready in the new folder", "Já deixa um painel pronto na pasta nova", "Сразу готовит панель в новой папке", "在新文件夹中直接准备好一个面板"]),
    ("set.panes_count", ["Open panes: ", "Painéis abertos: ", "Открытых панелей: ", "已打开面板："]),
    ("set.preview", ["Preview", "Prévia", "Предпросмотр", "预览"]),
    ("set.preview_hint", ["Accent, fonts and transparency as they will look", "Destaque, fontes e transparência como vão ficar", "Акцент, шрифты и прозрачность так, как они будут выглядеть", "强调色、字体与透明度的实际效果"]),
    ("set.preview_open", ["Open", "Abrir", "Открыть", "打开"]),
    ("set.projects_count", ["Projects: ", "Projetos: ", "Проектов: ", "项目数："]),
    ("set.scrollback", ["Scrollback", "Scrollback", "История прокрутки", "回滚缓冲"]),
    ("set.scrollback_hint", ["applies to terminals opened from now on", "vale para os terminais abertos a partir de agora", "действует для терминалов, открытых с этого момента", "对此后打开的终端生效"]),
    ("set.shell", ["Shell", "Shell", "Оболочка", "Shell"]),
    ("set.shell_hint", ["empty = detect ({})", "vazio = detectar ({})", "пусто = определить ({})", "留空 = 自动检测 ({})"]),
    ("set.shortcuts", ["Shortcuts", "Atalhos", "Горячие клавиши", "快捷键"]),
    ("set.show_clean", ["Show the \"clean\" badge", "Mostrar selo \"limpo\"", "Показывать значок «чисто»", "显示“干净”标记"]),
    ("set.show_clean_hint", ["Also marks the worktrees without changes", "Marca também as worktrees sem alterações", "Отмечает и рабочие деревья без изменений", "也标记没有改动的工作树"]),
    ("set.subtitle", ["Everything is saved automatically", "Tudo é salvo automaticamente", "Всё сохраняется автоматически", "所有更改自动保存"]),
    ("set.system_terminal", ["System terminal", "Terminal do sistema", "Системный терминал", "系统终端"]),
    ("set.term_opacity", ["Terminal transparency", "Transparência do terminal", "Прозрачность терминала", "终端透明度"]),
    ("set.terminal_section", ["Embedded terminal", "Terminal embutido", "Встроенный терминал", "内置终端"]),
    ("set.theme", ["Theme", "Tema", "Тема", "主题"]),
    ("set.title", ["Settings", "Ajustes", "Настройки", "设置"]),
    ("set.ui_font", ["Interface font", "Fonte da interface", "Шрифт интерфейса", "界面字体"]),
    ("set.ui_scale", ["Interface size", "Tamanho da interface", "Размер интерфейса", "界面大小"]),
    ("set.version", ["Version: ", "Versão: ", "Версия: ", "版本："]),
    ("term.back", ["Back to worktrees", "Voltar para worktrees", "Назад к рабочим деревьям", "返回工作树"]),
    ("term.broadcast_placeholder", ["Command for every pane (Enter sends)", "Comando para todos os painéis (Enter envia)", "Команда для всех панелей (Enter — отправить)", "发送给所有面板的命令（回车发送）"]),
    ("term.chip_bell", ["bell", "bell", "сигнал", "响铃"]),
    ("term.chip_exited", ["exited", "encerrado", "завершён", "已结束"]),
    ("term.close_all", ["Close all", "Fechar todos", "Закрыть все", "全部关闭"]),
    ("term.close_pane", ["Close pane (Ctrl+Shift+W)", "Fechar painel (Ctrl+Shift+W)", "Закрыть панель (Ctrl+Shift+W)", "关闭面板 (Ctrl+Shift+W)"]),
    ("term.drag_tooltip", ["Drag onto another pane to dock · double click maximizes", "Arraste sobre outro painel para encaixar · 2 cliques maximiza", "Перетащите на другую панель, чтобы пристыковать · двойной клик разворачивает", "拖到另一个面板上停靠 · 双击最大化"]),
    ("term.empty_body", ["Tick worktrees in the list and click \"Open multiterminal\".", "Marque worktrees na lista e clique em \"Abrir multiterminal\".", "Отметьте рабочие деревья в списке и нажмите «Открыть мультитерминал».", "在列表中勾选工作树，然后点击“打开多终端”。"]),
    ("term.empty_title", ["No terminal open", "Nenhum terminal aberto", "Нет открытых терминалов", "没有打开的终端"]),
    ("term.fullscreen", ["Fullscreen", "Tela cheia", "Полный экран", "全屏"]),
    ("term.fullscreen_off", ["Leave fullscreen", "Sair da tela cheia", "Выйти из полноэкранного режима", "退出全屏"]),
    ("term.hint", ["{} sessions · Ctrl+C interrupts · Ctrl+wheel changes the font", "{} sessões · Ctrl+C interrompe · Ctrl+roda muda a fonte", "{} сессий · Ctrl+C прерывает · Ctrl+колесо меняет шрифт", "{} 个会话 · Ctrl+C 中断 · Ctrl+滚轮改变字号"]),
    ("term.hint_free", ["drag a header to dock · gutters resize · Ctrl+wheel = font size", "arraste o cabeçalho para encaixar · divisões redimensionam · Ctrl+roda = fonte", "перетащите заголовок · разделители меняют размер · Ctrl+колесо = шрифт", "拖动标题停靠 · 分隔条调整大小 · Ctrl+滚轮 = 字号"]),
    ("term.maximize", ["Maximize this pane", "Maximizar este painel", "Развернуть панель", "最大化此面板"]),
    ("term.mirror", ["Mirror typing", "Espelhar digitação", "Дублировать ввод", "同步输入"]),
    ("term.mirror_hint", ["With this on, what you type in the focused pane goes to all the others (Ctrl+Shift+B)", "Com isso ligado, o que você digita no painel em foco vai para todos os outros (Ctrl+Shift+B)", "Когда включено, набранное в активной панели уходит во все остальные (Ctrl+Shift+B)", "开启后，在当前面板输入的内容会发送到其他所有面板 (Ctrl+Shift+B)"]),
    ("term.quick_add", ["Save a command", "Salvar um comando", "Сохранить команду", "保存命令"]),
    ("term.quick_edit", ["Edit this command", "Editar este comando", "Изменить команду", "编辑此命令"]),
    ("term.quick_run", ["Send to every pane", "Enviar para todos os painéis", "Отправить во все панели", "发送到所有面板"]),
    ("term.repo_tooltip", ["Repository folder, not a worktree", "Pasta do repositório, não um worktree", "Папка репозитория, а не рабочее дерево", "仓库文件夹，不是工作树"]),
    ("term.restart", ["Restart shell", "Reiniciar shell", "Перезапустить оболочку", "重启 shell"]),
    ("term.restore", ["Put the pane back", "Devolver o painel ao lugar", "Вернуть панель на место", "还原面板"]),
    ("term.send", ["Send", "Enviar", "Отправить", "发送"]),
    ("term.tidy", ["Tidy: put the panes back in a grid", "Organizar: volta os painéis para a grade", "Разложить панели обратно по сетке", "整理：把面板放回网格"]),
    ("theme.dark", ["Dark", "Escuro", "Тёмная", "深色"]),
    ("theme.light", ["Light", "Claro", "Светлая", "浅色"]),
    ("wt.cant_remove_main", ["The main worktree cannot be removed", "O worktree principal não pode ser removido", "Главное рабочее дерево удалить нельзя", "主工作树无法删除"]),
    ("wt.chip_bare", ["bare", "bare", "bare", "bare"]),
    ("wt.chip_changes_many", ["{} changes", "{} alterações", "{} изменений", "{} 处改动"]),
    ("wt.chip_changes_one", ["{} change", "{} alteração", "{} изменение", "{} 处改动"]),
    ("wt.chip_clean", ["clean", "limpo", "чисто", "干净"]),
    ("wt.chip_detached", ["detached", "detached", "detached", "detached"]),
    ("wt.chip_locked", ["locked", "travado", "заблокировано", "已锁定"]),
    ("wt.chip_missing", ["folder missing", "pasta ausente", "папка отсутствует", "文件夹不存在"]),
    ("wt.chip_official", ["official repo", "repo oficial", "официальный репозиторий", "官方仓库"]),
    ("wt.chip_official_hint", ["Repository folder: where every worktree comes from", "Pasta do repositório: origem de todos os worktrees", "Папка репозитория: из неё растут все рабочие деревья", "仓库文件夹：所有工作树的来源"]),
    ("wt.chip_orphan", ["orphan", "órfão", "потерянное", "孤立"]),
    ("wt.chip_terminal", ["terminal open", "terminal aberto", "терминал открыт", "终端已打开"]),
    ("wt.chip_worktree", ["worktree", "worktree", "рабочее дерево", "工作树"]),
    ("wt.clear", ["Clear", "Limpar", "Очистить", "清除"]),
    ("wt.clear_selection", ["Clear selection", "Limpar seleção", "Снять выделение", "清除选择"]),
    ("wt.copy_path", ["Copy path", "Copiar caminho", "Скопировать путь", "复制路径"]),
    ("wt.count_many", ["{} worktrees", "{} worktrees", "{} рабочих деревьев", "{} 个工作树"]),
    ("wt.count_one", ["{} worktree", "{} worktree", "{} рабочее дерево", "{} 个工作树"]),
    ("wt.dirty_count", ["{} with changes", "{} com alterações", "{} с изменениями", "{} 个有改动"]),
    ("wt.empty_body", ["Add a git repository to start orchestrating worktrees.", "Adicione um repositório git para começar a orquestrar worktrees.", "Добавьте git-репозиторий, чтобы управлять рабочими деревьями.", "添加一个 git 仓库，开始管理工作树。"]),
    ("wt.empty_title", ["No project yet", "Nenhum projeto ainda", "Пока нет проектов", "还没有项目"]),
    ("wt.empty_worktrees", ["no worktrees yet — use \"New worktree\" to create the first one", "nenhum worktree ainda — use \"Novo worktree\" para criar o primeiro", "рабочих деревьев пока нет — нажмите «Новый worktree», чтобы создать первое", "还没有工作树 — 用“新建工作树”创建第一个"]),
    ("wt.lock_hint", ["Lock so git will not remove or prune this worktree", "Travar para o git não remover nem limpar este worktree", "Заблокировать, чтобы git не удалял это рабочее дерево", "锁定，让 git 不会删除或清理此工作树"]),
    ("wt.new", ["New worktree", "Novo worktree", "Новый worktree", "新建工作树"]),
    ("wt.no_upstream", ["no upstream", "sem upstream", "без upstream", "没有上游分支"]),
    ("wt.none_listed", ["No worktrees listed", "Nenhum worktree listado", "Рабочие деревья не найдены", "没有列出工作树"]),
    ("wt.none_listed_body", ["Check that the path is a valid git repository.", "Confirme se o caminho é um repositório git válido.", "Проверьте, что путь — корректный git-репозиторий.", "请确认该路径是有效的 git 仓库。"]),
    ("wt.nothing_found", ["Nothing found", "Nada encontrado", "Ничего не найдено", "没有找到"]),
    ("wt.nothing_found_body", ["No worktree matches the filter.", "Nenhum worktree corresponde ao filtro.", "Ни одно рабочее дерево не подходит под фильтр.", "没有工作树符合该筛选。"]),
    ("wt.open_editor", ["Open in the editor", "Abrir no editor", "Открыть в редакторе", "在编辑器中打开"]),
    ("wt.open_external", ["Open in the system terminal", "Abrir no terminal do sistema", "Открыть в системном терминале", "在系统终端中打开"]),
    ("wt.open_multi", ["Open multiterminal ({})", "Abrir multiterminal ({})", "Открыть мультитерминал ({})", "打开多终端 ({})"]),
    ("wt.prune", ["Prune", "Prune", "Prune", "Prune"]),
    ("wt.prune_hint", ["Removes records of worktrees that no longer exist on disk", "Remove registros de worktrees que não existem mais no disco", "Удаляет записи о рабочих деревьях, которых больше нет на диске", "清除磁盘上已不存在的工作树记录"]),
    ("wt.refresh", ["Refresh", "Atualizar", "Обновить", "刷新"]),
    ("wt.remove", ["Remove worktree", "Remover worktree", "Удалить рабочее дерево", "删除工作树"]),
    ("wt.search", ["Search by name, branch or path", "Buscar por nome, branch ou caminho", "Поиск по имени, ветке или пути", "按名称、分支或路径搜索"]),
    ("wt.section_official", ["Official repository", "Repositório oficial", "Официальный репозиторий", "官方仓库"]),
    ("wt.section_worktrees", ["Worktrees ({})", "Worktrees ({})", "Рабочие деревья ({})", "工作树 ({})"]),
    ("wt.select_all", ["Select all", "Selecionar todos", "Выбрать все", "全选"]),
    ("wt.selected_many", ["{} selected", "{} selecionadas", "{} выбрано", "已选 {} 个"]),
    ("wt.selected_one", ["{} selected", "{} selecionada", "{} выбрано", "已选 {} 个"]),
    ("wt.status_tooltip", ["staged {} · unstaged {} · new {} · conflicts {}", "staged {} · não staged {} · novos {} · conflitos {}", "в индексе {} · не в индексе {} · новых {} · конфликтов {}", "已暂存 {} · 未暂存 {} · 新增 {} · 冲突 {}"]),
    ("wt.terminal", ["Terminal", "Terminal", "Терминал", "终端"]),
    ("wt.terminal_hint", ["Opens an embedded terminal in this worktree", "Abre um terminal embutido nesta worktree", "Открывает встроенный терминал в этом рабочем дереве", "在该工作树中打开内置终端"]),
    ("wt.unlock_hint", ["Unlock (allows removal and prune again)", "Destravar (volta a permitir remoção e prune)", "Разблокировать (снова разрешить удаление и prune)", "解锁（重新允许删除和 prune）"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_complete() {
        for pair in TABLE.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "out of order: {} {}",
                pair[0].0,
                pair[1].0
            );
        }
        for (key, row) in TABLE {
            for (index, text) in row.iter().enumerate() {
                assert!(!text.trim().is_empty(), "{key} is missing language {index}");
            }
            let holes = row[0].matches("{}").count();
            for text in row {
                assert_eq!(
                    text.matches("{}").count(),
                    holes,
                    "{key} has mismatched placeholders"
                );
            }
        }
    }

    /// The current language is global, so the tests that switch it take turns.
    static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn translation_follows_the_language() {
        let _turn = LANGUAGE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_lang(Lang::En);
        assert_eq!(tr("nav.settings"), "Settings");
        set_lang(Lang::Pt);
        assert_eq!(tr("nav.settings"), "Ajustes");
        set_lang(Lang::Ru);
        assert_eq!(tr("nav.settings"), "Настройки");
        set_lang(Lang::Zh);
        assert_eq!(tr("nav.settings"), "设置");
        set_lang(Lang::En);
    }

    #[test]
    fn placeholders_are_filled() {
        let _turn = LANGUAGE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_lang(Lang::En);
        assert_eq!(tr1("msg.added", "repo"), "repo added");
        assert_eq!(
            trn("wt.status_tooltip", &["1", "2", "3", "4"]),
            "staged 1 · unstaged 2 · new 3 · conflicts 4"
        );
    }
}
