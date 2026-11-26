//! macOS 的菜单栏：结构照 Tauri 2 的默认菜单（tauri 2.11 的 `Menu::default`），每一项的文字跟着界面语言（docs/I18N.md）。
//! Tauri 默认的那份用的是 muda 写死的英文（Hide Captae、Quit Captae、Edit、Copy……），中文界面里也是英文，所以自己建一份。
//!
//! - 启动时建一次（main.rs 的 `Builder::menu`，这时界面语言已经按设置定好）；设置里改了语言以后（`set_settings` 收到 `language`）重建、换上。
//! - 窗口菜单、帮助菜单和默认菜单一样用 Tauri 认的 id（`WINDOW_SUBMENU_ID`、`HELP_SUBMENU_ID`）：装上菜单时 Tauri 把它们设成系统的
//!   windowsMenu / helpMenu（窗口列表、帮助里的搜索框由系统加）。
//! - 比 Tauri 的默认菜单多一项「全部显示」，和 macOS 标准的应用菜单一样（隐藏 Captae、隐藏其他、全部显示）。
//! - 系统自己管的几处不归这里：编辑菜单里系统自动加的「开始听写」「表情与符号」、全屏那一项切换时系统换上的「进入 / 退出全屏幕」、
//!   「服务」里面的项目。它们按 macOS 给 Captae 选的语言显示（Info.plist 的 CFBundleLocalizations，启动时定），
//!   Captae 设置里的语言是「跟随系统」时和界面一致；在 Captae 里另选一种语言时它们不跟着变。
//!
//! 只在 macOS 上用（其他平台本来就没有这个菜单，保持现状）。

use captae_core::tr;
use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID};
use tauri::{AppHandle, Wry};

/// 菜单栏每一项的文字（按现在的界面语言）。原文是 macOS 简体中文的叫法，英文照 Apple 的写法
struct Labels {
    about: String,
    services: String,
    hide: String,
    hide_others: String,
    show_all: String,
    quit: String,
    file: String,
    close_window: String,
    edit: String,
    undo: String,
    redo: String,
    cut: String,
    copy: String,
    paste: String,
    select_all: String,
    view: String,
    fullscreen: String,
    window: String,
    minimize: String,
    zoom: String,
    help: String,
}

impl Labels {
    fn now() -> Labels {
        Labels {
            about: tr!("关于 Captae§macOS 菜单栏"),
            services: tr!("服务§macOS 菜单栏"),
            hide: tr!("隐藏 Captae§macOS 菜单栏"),
            hide_others: tr!("隐藏其他§macOS 菜单栏"),
            show_all: tr!("全部显示§macOS 菜单栏"),
            quit: tr!("退出 Captae§macOS 菜单栏"),
            file: tr!("文件§macOS 菜单栏"),
            close_window: tr!("关闭窗口§macOS 菜单栏"),
            edit: tr!("编辑§macOS 菜单栏"),
            undo: tr!("撤销§macOS 菜单栏"),
            redo: tr!("重做§macOS 菜单栏"),
            cut: tr!("剪切§macOS 菜单栏"),
            copy: tr!("拷贝§macOS 菜单栏"),
            paste: tr!("粘贴§macOS 菜单栏"),
            select_all: tr!("全选§macOS 菜单栏"),
            view: tr!("显示§macOS 菜单栏"),
            fullscreen: tr!("进入全屏幕§macOS 菜单栏"),
            window: tr!("窗口§macOS 菜单栏"),
            minimize: tr!("最小化§macOS 菜单栏"),
            zoom: tr!("缩放§macOS 菜单栏"),
            help: tr!("帮助§macOS 菜单栏"),
        }
    }
}

/// 按现在的界面语言建菜单栏
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let l = Labels::now();
    let pkg = app.package_info();
    let config = app.config();
    // 「关于」窗口里的名字、版本、版权：和 Tauri 的默认菜单一样，从 tauri.conf.json 取
    let about = AboutMetadata {
        name: Some(pkg.name.clone()),
        version: Some(pkg.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };
    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        &l.window,
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some(l.minimize.as_str()))?,
            &PredefinedMenuItem::maximize(app, Some(l.zoom.as_str()))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, Some(l.close_window.as_str()))?,
        ],
    )?;
    let help_menu = Submenu::with_id_and_items(app, HELP_SUBMENU_ID, &l.help, true, &[])?;
    Menu::with_items(
        app,
        &[
            // 应用菜单：菜单栏上显示的标题由系统换成程序名
            &Submenu::with_items(
                app,
                &pkg.name,
                true,
                &[
                    &PredefinedMenuItem::about(app, Some(l.about.as_str()), Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, Some(l.services.as_str()))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, Some(l.hide.as_str()))?,
                    &PredefinedMenuItem::hide_others(app, Some(l.hide_others.as_str()))?,
                    &PredefinedMenuItem::show_all(app, Some(l.show_all.as_str()))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, Some(l.quit.as_str()))?,
                ],
            )?,
            &Submenu::with_items(app, &l.file, true, &[&PredefinedMenuItem::close_window(app, Some(l.close_window.as_str()))?])?,
            &Submenu::with_items(
                app,
                &l.edit,
                true,
                &[
                    &PredefinedMenuItem::undo(app, Some(l.undo.as_str()))?,
                    &PredefinedMenuItem::redo(app, Some(l.redo.as_str()))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, Some(l.cut.as_str()))?,
                    &PredefinedMenuItem::copy(app, Some(l.copy.as_str()))?,
                    &PredefinedMenuItem::paste(app, Some(l.paste.as_str()))?,
                    &PredefinedMenuItem::select_all(app, Some(l.select_all.as_str()))?,
                ],
            )?,
            &Submenu::with_items(app, &l.view, true, &[&PredefinedMenuItem::fullscreen(app, Some(l.fullscreen.as_str()))?])?,
            &window_menu,
            &help_menu,
        ],
    )
}

/// 启动时（main.rs 的 `Builder::menu`）：建不成就先用 Tauri 默认的英文菜单，不因为菜单栏让程序打不开
pub fn initial(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    build(app).or_else(|err| {
        tracing::warn!("菜单栏没建成，先用 Tauri 默认的：{err}");
        Menu::default(app)
    })
}

/// 设置里改了界面语言以后：按新的语言重建菜单栏、换上（换下来的旧菜单 Tauri 从系统里摘掉）
pub fn apply(app: &AppHandle) {
    if let Err(err) = build(app).and_then(|m| app.set_menu(m)) {
        tracing::warn!("菜单栏没能按新的语言重建：{err}");
    }
}

#[cfg(test)]
mod tests {
    use super::Labels;
    use captae_core::i18n::{with_lang, Lang};

    /// 按菜单里从左到右、从上到下的顺序
    fn all(l: &Labels) -> [&str; 21] {
        [
            &l.about, &l.services, &l.hide, &l.hide_others, &l.show_all, &l.quit, &l.file, &l.close_window, &l.edit, &l.undo, &l.redo, &l.cut, &l.copy, &l.paste,
            &l.select_all, &l.view, &l.fullscreen, &l.window, &l.minimize, &l.zoom, &l.help,
        ]
        .map(String::as_str)
    }

    /// 中文是 macOS 简体中文的叫法；英文照 Apple 的写法（每一项英文词表里都要有，没有时会退回中文原文）
    #[test]
    fn labels_follow_language() {
        let zh = with_lang(Lang::ZhCn, Labels::now);
        assert_eq!(
            all(&zh),
            ["关于 Captae", "服务", "隐藏 Captae", "隐藏其他", "全部显示", "退出 Captae", "文件", "关闭窗口", "编辑", "撤销", "重做", "剪切", "拷贝", "粘贴", "全选", "显示", "进入全屏幕", "窗口", "最小化", "缩放", "帮助"]
        );
        let en = with_lang(Lang::En, Labels::now);
        assert_eq!(
            all(&en),
            [
                "About Captae", "Services", "Hide Captae", "Hide Others", "Show All", "Quit Captae", "File", "Close Window", "Edit", "Undo", "Redo", "Cut", "Copy", "Paste", "Select All",
                "View", "Enter Full Screen", "Window", "Minimize", "Zoom", "Help"
            ]
        );
    }
}
