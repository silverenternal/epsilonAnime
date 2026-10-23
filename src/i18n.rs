use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Locale {
    #[default]
    ZhCn,
    En,
    Ja,
}

impl Locale {
    pub fn next(self) -> Self {
        match self {
            Self::ZhCn => Self::En,
            Self::En => Self::Ja,
            Self::Ja => Self::ZhCn,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::ZhCn => "中文",
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }
}

pub fn tr(locale: Locale, key: &str) -> &str {
    match (locale, key) {
        (Locale::ZhCn, "now_playing") => "正在播放",
        (Locale::ZhCn, "cover") => "封面",
        (Locale::ZhCn, "lyrics") => "歌词与控制",
        (Locale::ZhCn, "spectrum") => "频谱",
        (Locale::ZhCn, "logs") => "日志",
        (Locale::ZhCn, "library") => "网易云音乐库",
        (Locale::Ja, "now_playing") => "再生中",
        (Locale::Ja, "cover") => "カバー",
        (Locale::Ja, "lyrics") => "歌詞と操作",
        (Locale::Ja, "spectrum") => "スペクトル",
        (Locale::Ja, "logs") => "ログ",
        (Locale::Ja, "library") => "NetEase ライブラリ",
        (_, "now_playing") => "NOW PLAYING",
        (_, "cover") => "COVER",
        (_, "lyrics") => "LYRICS + CONTROLS",
        (_, "spectrum") => "SPECTRUM",
        (_, "logs") => "LOGS",
        (_, "library") => "NETEASE LIBRARY",
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_has_a_title_and_cycles() {
        for locale in [Locale::ZhCn, Locale::En, Locale::Ja] {
            assert_ne!(tr(locale, "lyrics"), "lyrics");
            assert_ne!(locale.next(), locale);
        }
    }
}
