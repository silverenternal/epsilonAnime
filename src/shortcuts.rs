use crate::config::Hotkeys;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct HelpPage {
    pub title: &'static str,
    pub entries: Vec<(String, &'static str)>,
}

pub fn pages(hotkeys: &Hotkeys) -> Vec<HelpPage> {
    let entry = |key: &str, action| (key.to_string(), action);
    vec![
        HelpPage {
            title: "播放与歌词",
            entries: vec![
                entry("Space", "暂停 / 继续"),
                entry("← / →", "后退 / 前进 5 秒"),
                entry("- / =", "音量减 / 加"),
                entry("m", "静音 / 恢复"),
                entry("p", "切换播放模式"),
                entry("n / b", "下一首 / 上一首"),
                entry("t", "显示 / 隐藏翻译"),
                entry("o", "显示 / 隐藏罗马音"),
                entry(", / .", "歌词偏移 -0.1 / +0.1 秒"),
                entry("Shift+↑ / ↓", "滚动歌词时间轴"),
                entry("Alt+F", "搜索已加载歌词"),
                entry("鼠标点击歌词栏", "切换播放、上一首、下一首或 seek"),
            ],
        },
        HelpPage {
            title: "动画、封面与窗口",
            entries: vec![
                entry("[ / ]", "上一场 / 下一场动画"),
                entry("f", "收藏当前动画"),
                entry("Alt+D", "禁用 / 恢复当前动画"),
                entry("Alt+Shift+D", "重置动画收藏、禁用和权重"),
                entry("j", "锁定自动换场"),
                entry("w / W", "提高 / 降低当前场景权重"),
                entry("c", "切换盲文 / 混合 / 半块封面"),
                entry("Tab", "切换 balanced / gallery / instrument"),
                entry("r", "恢复默认布局"),
                entry("Alt+H", "显示 / 关闭本快捷键列表"),
                entry("Ctrl+Tab", "切换窗口焦点"),
                entry("Alt+M / Alt+Shift+M", "最小化焦点 / 恢复最小化窗口"),
                entry("Ctrl+PgUp / PgDn", "调整窗口遮挡层级"),
                entry("拖动 / 右下角", "移动窗口 / 调整窗口大小"),
            ],
        },
        HelpPage {
            title: "界面、系统与配置",
            entries: vec![
                entry("F1 / ?", "显示 / 关闭快捷键列表"),
                entry("F3 / Alt+L", "显示 / 隐藏日志"),
                entry("F4 / Alt+S", "打开 / 收起 Shell"),
                entry("Alt+N", "切换通知列表"),
                entry("Alt+R", "重试可恢复错误"),
                entry("Alt+X", "退出网易云登录"),
                entry(&hotkeys.reload, "重新加载 TOML 配置"),
                entry(&hotkeys.cycle_locale, "切换界面语言"),
                entry("Enter / Esc", "输入框提交 / 取消"),
                entry("q", "退出（输入框和 Shell 中作为普通输入）"),
                entry("Esc", "退出；弹窗或帮助中最先关闭当前层"),
                entry("Ctrl+C", "退出"),
            ],
        },
        HelpPage {
            title: "网易云音乐库",
            entries: vec![
                entry("l", "打开歌单和音乐库首页"),
                entry("g", "打开网易云扫码登录"),
                entry("/", "搜索网易云歌曲"),
                entry("i", "输入歌曲链接或数字 ID"),
                entry("↑ / ↓", "移动列表选择"),
                entry("Enter", "打开分类 / 播放歌曲"),
                entry("Backspace", "返回上一层"),
                entry("PageDown", "加载当前歌单下一页"),
                entry("s", "音乐库内搜索"),
                entry("d", "每日推荐"),
                entry("h / a / r", "排行榜 / 收藏专辑 / 收藏歌手"),
                entry("i / m / u", "我喜欢的音乐 / 私人 FM / 最近播放"),
                entry("f / v", "收藏歌曲 / 查看详情与相关推荐"),
                entry("x / g", "退出登录 / 重新扫码"),
            ],
        },
        HelpPage {
            title: "内嵌 Shell",
            entries: vec![
                entry("F4 / Alt+S", "收起 Shell（进程保持运行）"),
                entry("Alt+F", "搜索 scrollback"),
                entry(&hotkeys.shell_previous_tab, "上一个标签"),
                entry(&hotkeys.shell_next_tab, "下一个标签"),
                entry("Alt+T / Alt+W", "新建 / 关闭标签"),
                entry("Alt+O", "打开最近的 OSC8 链接"),
                entry("滚轮", "浏览 scrollback"),
                entry("Shift+拖动", "选择并复制文本"),
                entry("Enter / 输入", "发送给 Shell 进程"),
            ],
        },
    ]
}

pub fn is_help_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::F(1)
        || (matches!(key.code, KeyCode::Char('?'))
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT))
        || (matches!(key.code, KeyCode::Char('h' | 'H'))
            && key.modifiers.contains(KeyModifiers::ALT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_help_pages_are_nonempty() {
        let pages = pages(&Hotkeys::default());
        assert_eq!(pages.len(), 5);
        assert!(pages.iter().all(|page| !page.entries.is_empty()));
        assert!(
            pages[3]
                .entries
                .iter()
                .any(|(_, action)| action.contains("每日推荐"))
        );
    }

    #[test]
    fn help_key_accepts_f1_and_question_mark_only() {
        assert!(is_help_key(&KeyEvent::new(
            KeyCode::F(1),
            KeyModifiers::NONE
        )));
        assert!(is_help_key(&KeyEvent::new(
            KeyCode::Char('h'),
            KeyModifiers::ALT
        )));
        assert!(!is_help_key(&KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
    }
}
