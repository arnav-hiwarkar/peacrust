use std::path::Path;

pub const HISTORY_CAP: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Insert(char),
    Left,
    Right,
    Home,
    End,
    Delete,
    Backspace,
    Clear,
    Prev,
    Next,
}

#[derive(Default)]
pub struct Prompt {
    input: Vec<char>,
    cursor: usize,
    pub history: Vec<String>,
    browse: Option<(usize, String)>,
}

/// Case-insensitive subsequence match: `vb` matches `view binary`.
pub fn fuzzy(query: &str, entry: &str) -> bool {
    let mut e = entry.chars().flat_map(char::to_lowercase);
    query.chars().flat_map(char::to_lowercase).all(|q| e.any(|c| c == q))
}

impl Prompt {
    pub fn text(&self) -> String {
        self.input.iter().collect()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn set(&mut self, s: &str) {
        self.input = s.chars().collect();
        self.cursor = self.input.len();
    }

    pub fn edit(&mut self, e: Edit) {
        if !matches!(e, Edit::Prev | Edit::Next) {
            self.browse = None;
        }
        let len = self.input.len();
        match e {
            Edit::Insert(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += 1;
            }
            Edit::Left => self.cursor = self.cursor.saturating_sub(1),
            Edit::Right => self.cursor = (self.cursor + 1).min(len),
            Edit::Home => self.cursor = 0,
            Edit::End => self.cursor = len,
            Edit::Delete if self.cursor < len => {
                self.input.remove(self.cursor);
            }
            Edit::Delete | Edit::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.input.remove(self.cursor);
            }
            Edit::Delete | Edit::Backspace => {}
            Edit::Clear => self.set(""),
            Edit::Prev => {
                let (from, query) = self.browse.take().unwrap_or((self.history.len(), self.text()));
                let hit = self.history[..from].iter().rposition(|h| fuzzy(&query, h));
                match hit {
                    Some(i) => {
                        let entry = self.history[i].clone();
                        self.set(&entry);
                        self.browse = Some((i, query));
                    }
                    None => self.browse = Some((from, query)),
                }
            }
            Edit::Next => {
                let Some((from, query)) = self.browse.take() else { return };
                let start = (from + 1).min(self.history.len());
                match self.history[start..].iter().position(|h| fuzzy(&query, h)) {
                    Some(i) => {
                        let entry = self.history[start + i].clone();
                        self.set(&entry);
                        self.browse = Some((start + i, query));
                    }
                    None => self.set(&query),
                }
            }
        }
    }

    /// Take the input, record it in history and reset the editor.
    pub fn submit(&mut self) -> String {
        let line = self.text();
        self.set("");
        self.browse = None;
        if !line.trim().is_empty() && self.history.last() != Some(&line) {
            self.history.push(line.clone());
            let over = self.history.len().saturating_sub(HISTORY_CAP);
            self.history.drain(..over);
        }
        line
    }

    pub fn reset(&mut self) {
        self.set("");
        self.browse = None;
    }
}

pub fn load_history(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
    let over = lines.len().saturating_sub(HISTORY_CAP);
    lines.drain(..over);
    lines
}

pub fn save_history(path: &Path, history: &[String]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, history.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(s: &str) -> Prompt {
        let mut p = Prompt::default();
        s.chars().for_each(|c| p.edit(Edit::Insert(c)));
        p
    }

    #[test]
    fn editing_keys() {
        let mut p = typed("vew");
        p.edit(Edit::Left);
        p.edit(Edit::Left);
        p.edit(Edit::Insert('i'));
        assert_eq!((p.text(), p.cursor()), ("view".into(), 2));
        p.edit(Edit::Home);
        p.edit(Edit::Delete);
        assert_eq!(p.text(), "iew");
        p.edit(Edit::End);
        p.edit(Edit::Delete);
        assert_eq!(p.text(), "ie");
        p.edit(Edit::Backspace);
        p.edit(Edit::Right);
        assert_eq!((p.text(), p.cursor()), ("i".into(), 1));
        p.edit(Edit::Clear);
        p.edit(Edit::Backspace);
        assert_eq!((p.text(), p.cursor()), ("".into(), 0));
    }

    #[test]
    fn submit_records_history_without_repeats() {
        let mut p = typed("mode timer");
        assert_eq!(p.submit(), "mode timer");
        "mode timer".chars().for_each(|c| p.edit(Edit::Insert(c)));
        p.submit();
        p.submit();
        assert_eq!(p.history, ["mode timer"]);
        assert_eq!(p.text(), "");
    }

    #[test]
    fn history_navigation_matches_fuzzily() {
        let mut p = Prompt { history: vec!["view binary".into(), "mode timer".into(), "view ascii".into()], ..Default::default() };
        "vb".chars().for_each(|c| p.edit(Edit::Insert(c)));
        p.edit(Edit::Prev);
        assert_eq!(p.text(), "view binary");
        p.edit(Edit::Prev);
        assert_eq!(p.text(), "view binary");
        p.edit(Edit::Next);
        assert_eq!(p.text(), "vb");

        p.edit(Edit::Clear);
        p.edit(Edit::Prev);
        assert_eq!(p.text(), "view ascii");
        p.edit(Edit::Prev);
        assert_eq!(p.text(), "mode timer");
        p.edit(Edit::Next);
        assert_eq!(p.text(), "view ascii");
    }

    #[test]
    fn fuzzy_rules() {
        assert!(fuzzy("VA", "view ascii"));
        assert!(fuzzy("", "anything"));
        assert!(!fuzzy("av", "view ascii x"));
        assert!(fuzzy("av", "ascii view"));
    }

    #[test]
    fn history_persists() {
        let path = std::env::temp_dir().join(format!("peacrust-hist-{}/history", std::process::id()));
        save_history(&path, &["a".into(), "b".into()]).unwrap();
        assert_eq!(load_history(&path), ["a", "b"]);
        assert_eq!(load_history(Path::new("/nonexistent/peacrust")), Vec::<String>::new());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
