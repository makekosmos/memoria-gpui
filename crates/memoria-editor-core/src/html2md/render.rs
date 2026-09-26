//! Streaming HTML→Markdown renderer over `tok` tokens.
//! Buffers stack per open blockquote; lists/table state tracked minimally.

use super::tok::{decode_entities, tokenize, Tok};

struct Rend {
    /// Buffer stack: base = out, +1 per open blockquote.
    bufs: Vec<String>,
    lists: Vec<(char, usize)>,
    links: Vec<String>,
    pre: bool,
    skip: usize,
    /// per-table: header separator not yet emitted
    table_hdr: Vec<bool>,
    cell_open: bool,
}

impl Rend {
    fn cur(&mut self) -> &mut String {
        self.bufs.last_mut().unwrap()
    }

    /// Ensure at least `n` trailing newlines in current buffer.
    fn sep(&mut self, n: usize) {
        if self.skip > 0 || self.pre {
            return;
        }
        let have = self.cur().chars().rev().take_while(|&c| c == '\n').count();
        for _ in have..n {
            self.cur().push('\n');
        }
    }

    fn emit(&mut self, s: &str) {
        if self.skip == 0 {
            self.cur().push_str(s);
        }
    }

    fn text(&mut self, raw: &str) {
        if self.skip > 0 {
            return;
        }
        let d = decode_entities(raw);
        if self.pre {
            self.cur().push_str(&d);
            return;
        }
        let mut parts = d.split_whitespace().peekable();
        if d.chars().next().is_some_and(|c| c.is_whitespace())
            && !self.cur().ends_with(['\n', ' '])
            && !self.cur().is_empty()
        {
            self.cur().push(' ');
        }
        while let Some(w) = parts.next() {
            self.cur().push_str(w);
            if parts.peek().is_some() || d.chars().last().is_some_and(|c| c.is_whitespace()) {
                self.cur().push(' ');
            }
        }
    }

    fn open(&mut self, name: &str, attrs: &[(String, String)]) {
        let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
        if matches!(
            name,
            "script" | "style" | "head" | "title" | "meta" | "link" | "noscript" | "template"
        ) {
            self.skip += 1;
            return;
        }
        if self.skip > 0 {
            return;
        }
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.sep(2);
                self.emit(&"#".repeat((name.as_bytes()[1] - b'0').min(6) as usize));
                self.emit(" ");
            }
            "p" | "div" | "section" | "article" => self.sep(2),
            "br" => self.emit("\n"),
            "hr" => {
                self.sep(2);
                self.emit("---\n\n");
            }
            "strong" | "b" => self.emit("**"),
            "em" | "i" => self.emit("*"),
            "del" | "s" | "strike" => self.emit("~~"),
            "code" if !self.pre => self.emit("`"),
            "pre" => {
                self.sep(2);
                self.emit("```\n");
                self.pre = true;
            }
            "blockquote" => {
                self.sep(2);
                self.bufs.push(String::new());
            }
            "ul" => self.lists.push(('u', 0)),
            "ol" => {
                let start = get("start").and_then(|s| s.parse().ok()).unwrap_or(1);
                self.lists.push(('o', start));
            }
            "li" => {
                self.sep(1);
                let ind = "  ".repeat(self.lists.len().saturating_sub(1));
                match self.lists.last_mut() {
                    Some(('o', c)) => {
                        let n = *c;
                        *c += 1;
                        self.emit(&format!("{ind}{n}. "));
                    }
                    _ => self.emit(&format!("{ind}- ")),
                }
            }
            "input" if get("type") == Some("checkbox") => {
                let on = attrs.iter().any(|(a, _)| a == "checked");
                self.emit(if on { "[x] " } else { "[ ] " });
            }
            "a" => {
                self.links.push(get("href").unwrap_or("").to_string());
                self.emit("[");
            }
            "img" => {
                self.emit(&format!(
                    "![{}]({})",
                    get("alt").unwrap_or(""),
                    get("src").unwrap_or("")
                ));
            }
            "table" => {
                self.sep(2);
                self.table_hdr.push(true);
            }
            "tr" => {
                self.sep(1);
                self.cell_open = false;
            }
            "td" | "th" => {
                if self.cell_open {
                    self.emit(" | ");
                } else {
                    self.emit("| ");
                    self.cell_open = true;
                }
            }
            _ => {}
        }
    }

    fn close(&mut self, name: &str) {
        if self.skip > 0 {
            if matches!(
                name,
                "script" | "style" | "head" | "title" | "noscript" | "template"
            ) {
                self.skip -= 1;
            }
            return;
        }
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "div" | "section" | "article" => {
                self.sep(2)
            }
            "strong" | "b" => self.emit("**"),
            "em" | "i" => self.emit("*"),
            "del" | "s" | "strike" => self.emit("~~"),
            "code" if !self.pre => self.emit("`"),
            "pre" => {
                self.pre = false;
                if !self.cur().ends_with('\n') {
                    self.cur().push('\n');
                }
                self.emit("```\n\n");
            }
            "blockquote" if self.bufs.len() > 1 => {
                let inner = self.bufs.pop().unwrap();
                let body = inner.trim_matches('\n');
                for line in body.split('\n') {
                    if line.trim().is_empty() {
                        self.emit(">\n");
                    } else {
                        self.emit(&format!("> {}\n", line.trim_end()));
                    }
                }
                self.sep(2);
            }
            "blockquote" => {}
            "ul" | "ol" => {
                self.lists.pop();
                self.sep(if self.lists.is_empty() { 2 } else { 1 });
            }
            "a" => {
                let href = self.links.pop().unwrap_or_default();
                self.emit(&format!("]({href})"));
            }
            "tr" => {
                if self.cell_open {
                    self.emit(" |");
                }
                if self.table_hdr.last() == Some(&true) {
                    self.emit("\n| ---");
                    self.sep(1);
                    if let Some(h) = self.table_hdr.last_mut() {
                        *h = false;
                    }
                }
            }
            "table" => {
                self.table_hdr.pop();
                self.sep(2);
            }
            _ => {}
        }
    }
}

/// Convert an HTML clipboard fragment to Markdown. Always succeeds; unknown
/// markup degrades to its text content.
pub fn html_to_markdown(html: &str) -> String {
    let mut r = Rend {
        bufs: vec![String::new()],
        lists: Vec::new(),
        links: Vec::new(),
        pre: false,
        skip: 0,
        table_hdr: Vec::new(),
        cell_open: false,
    };
    for t in tokenize(html) {
        match t {
            Tok::Text { text } => r.text(&text),
            Tok::Open { name, attrs } => r.open(&name, &attrs),
            Tok::Close { name } => r.close(&name),
        }
    }
    while r.bufs.len() > 1 {
        let inner = r.bufs.pop().unwrap();
        let body = inner.trim_matches('\n');
        for line in body.split('\n') {
            if !line.trim().is_empty() {
                r.emit(&format!("> {}\n", line.trim_end()));
            }
        }
    }
    let out = r.bufs.pop().unwrap_or_default();
    let trimmed = out.trim_matches('\n').to_string();
    if trimmed.is_empty() {
        trimmed
    } else {
        trimmed + "\n"
    }
}
