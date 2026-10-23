use super::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptMode {
    #[default]
    Url,
    SongSearch,
    LyricSearch,
}

impl App {
    pub fn begin_prompt(&mut self) {
        self.prompt.get_or_insert_with(String::new);
        self.prompt_mode = PromptMode::Url;
    }

    pub fn cancel_prompt(&mut self) {
        self.prompt = None;
        self.prompt_mode = PromptMode::Url;
    }

    pub fn push_prompt(&mut self, text: &str) {
        let target = self.prompt.get_or_insert_with(String::new);
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            target.push(ch);
        }
    }

    pub fn backspace_prompt(&mut self) {
        if let Some(prompt) = &mut self.prompt {
            prompt.pop();
        }
    }

    pub fn take_prompt(&mut self) -> Option<String> {
        self.prompt.take()
    }

    pub fn prompt_active(&self) -> bool {
        self.prompt.is_some()
    }

    pub fn begin_search(&mut self) {
        self.prompt = Some(String::new());
        self.prompt_mode = PromptMode::SongSearch;
        self.status = "search NetEase songs…".into();
    }

    pub fn begin_lyric_search(&mut self) {
        self.prompt = Some(String::new());
        self.prompt_mode = PromptMode::LyricSearch;
        self.status = "search loaded lyrics…".into();
    }

    pub fn take_prompt_mode(&mut self) -> PromptMode {
        std::mem::take(&mut self.prompt_mode)
    }

    pub fn search_lyrics(&mut self, query: &str) {
        let found = self.lyric.search(query);
        self.status = if found {
            "lyric match found".into()
        } else {
            "lyric text not found".into()
        };
    }
}
