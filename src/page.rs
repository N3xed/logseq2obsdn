use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::OnceCell;
use regex::Regex;
use strum::IntoEnumIterator;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct RefsFile {
    pub refs: HashMap<String, Ref>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Ref {
    pub file: String,
    pub id: String,
}

impl Ref {
    fn get_link(&self, curr_title: &str) -> String {
        if self.file == curr_title.trim() {
            self.id.clone()
        } else {
            format!("{}{}", self.file, self.id)
        }
    }
}

pub trait Data {
    fn page_title(&mut self, title: &str);
    fn copy_asset(&mut self, path: &str) -> String;
    fn register_id(&mut self, id: &Id);
    fn query_id(&self, logseq_id: &str) -> Option<&Ref>;
    fn curr_title(&self) -> &str;
}

#[derive(Debug)]
pub struct Page {
    pub title: String,
    pub alias: Vec<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub struct Id {
    pub logseq_id: String,
    pub obsdn_id: String,
}

#[derive(Debug)]
pub struct Block {
    pub text: String,
    pub id: Option<Id>,
    pub header: Option<String>,
    pub children: Vec<Block>,
    pub is_list_item: bool,
    pub self_border: bool,
}

fn self_border_re() -> &'static Regex {
    static SELF_BORDER_RE: OnceCell<Regex> = OnceCell::new();
    SELF_BORDER_RE.get_or_init(|| Regex::new(r"( )?#\.v-self-border").unwrap())
}

fn only_math_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"(?s)^\s*(?:- )?\${2}.*\${2}\s*$").unwrap())
}

fn image_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"(?s)!\[([^\]]*)\]\(([^\)]*)\)").unwrap())
}

fn only_image_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"(?s)^\s*-?\s*!\[([^\]]*)\]\(([^\)]*)\)\s*$").unwrap())
}

fn header_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"^\s*-?\s*#+\s(.*[^\s])\s*$").unwrap())
}

fn header_san_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"[^a-zA-Z0-9\-_öäüÖÄÜèàé]+").unwrap())
}

/// Groups:
/// 0: whole
/// 1: title
/// 2: url
fn file_link_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(\[{2}([^\]]+)\]{2}\)").unwrap())
}

/// Groups:
/// 1: url
/// 2: id
fn standalone_id_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| {
        Regex::new(
            r"[^\(](\({2}([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\){2})",
        )
        .unwrap()
    })
}

/// Groups:
/// 0: whole
/// 1: title
/// 2: id
fn link_id_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(\({2}([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\){2}\)").unwrap())
}

/// Groups:
/// 0: whole
/// 1: id
fn embed_id_re() -> &'static Regex {
    static RE: OnceCell<Regex> = OnceCell::new();
    RE.get_or_init(|| Regex::new(r"\{\{embed \({2}([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\){2}\}\}").unwrap())
}

impl Block {
    fn transform(
        &mut self,
        parent: Option<&Block>,
        prev_sibling: Option<&Block>,
        data: &mut dyn Data,
    ) {
        let mut children = std::mem::replace(&mut self.children, Vec::new());
        for i in 0..children.len() {
            let (prev, rest) = children.as_mut_slice().split_at_mut(i);
            let (curr, _rest) = rest.split_at_mut(1);
            let curr = curr.first_mut().unwrap();
            curr.transform(Some(self), prev.last(), data);
        }
        self.children = children;

        let parent_none_or_normal = parent.map(|p| !p.is_list_item).unwrap_or(true);
        let prev_none_or_normal = prev_sibling.map(|p| !p.is_list_item).unwrap_or(true);

        if parent_none_or_normal && only_math_re().is_match(&self.text) {
            self.set_list_item(false);
        }

        if parent.is_none() && self.text.starts_with("- ## ") {
            self.text.remove(2);
        }
        if parent.is_none() && self.text.starts_with("## ") {
            self.text.remove(0);
        }

        if parent_none_or_normal && prev_none_or_normal && only_image_re().is_match(&self.text) {
            self.set_list_item(false);
        }

        let mut text = self.text.clone();
        for m in image_re()
            .captures_iter(&self.text)
            .collect_vec()
            .into_iter()
            .rev()
        {
            let (_name, path) = match (m.get(1), m.get(2)) {
                (Some(n), Some(p)) => (n, p),
                _ => continue,
            };

            let new_path = data.copy_asset(path.as_str());
            text.replace_range(path.range(), &new_path);
        }

        self.text = text.clone();
        for m in file_link_re()
            .captures_iter(&self.text)
            .collect_vec()
            .into_iter()
            .rev()
        {
            let (whole, title, url) = match (m.get(0), m.get(1), m.get(2)) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => continue,
            };
            let url = url.as_str();
            let title = title.as_str();
            text.replace_range(whole.range(), &format!("[[{url}|{title}]]"));
        }

        self.text = text.clone();
        for m in embed_id_re()
            .captures_iter(&self.text)
            .collect_vec()
            .into_iter()
            .rev()
        {
            let (whole, id) = match (m.get(0), m.get(1)) {
                (Some(n), Some(p)) => (n, p),
                _ => continue,
            };
            if let Some(r) = data.query_id(id.as_str()) {
                let link = r.get_link(data.curr_title());
                text.replace_range(whole.range(), &format!("![[{link}]]"));
            }
        }

        self.text = text.clone();
        for m in link_id_re()
            .captures_iter(&self.text)
            .collect_vec()
            .into_iter()
            .rev()
        {
            let (whole, title, id) = match (m.get(0), m.get(1), m.get(2)) {
                (Some(w), Some(a), Some(b)) => (w, a, b),
                _ => continue,
            };
            if let Some(r) = data.query_id(id.as_str()) {
                let link = r.get_link(data.curr_title());
                let title = title.as_str();
                text.replace_range(whole.range(), &format!("[[{link}|{title}]]"));
            }
        }

        self.text = text.clone();
        for m in standalone_id_re()
            .captures_iter(&self.text)
            .collect_vec()
            .into_iter()
            .rev()
        {
            let (url, id) = match (m.get(1), m.get(2)) {
                (Some(n), Some(p)) => (n, p),
                _ => continue,
            };
            if let Some(r) = data.query_id(id.as_str()) {
                let link = r.get_link(data.curr_title());
                text.replace_range(url.range(), &format!("[[{link}]]"));
            }
        }
    }

    pub fn parse(text: &str, data: &mut dyn Data) -> Result<Self> {
        let mut first_child = None;
        let mut id = None;
        let body = text
            .lines()
            .chain([""])
            .tuple_windows::<(&str, &str)>()
            .find_map(|(l, curr)| {
                if let Some((Prop::Id, val)) = parse_prop(curr) {
                    id = Some(val);
                    return None;
                }

                if curr.trim_start().starts_with('-') {
                    first_child = Some(curr);
                    Some(l)
                } else {
                    None
                }
            })
            .map(|l| unsafe { union_str(text, l) })
            .unwrap_or(text);

        let mut is_list_item = body.starts_with("- ");

        let mut body = body
            .strip_prefix("- ")
            .unwrap_or(&body)
            .lines()
            .filter(|l| match parse_prop(l) {
                Some((Prop::Id, val)) => {
                    id = Some(val);
                    false
                }
                None => true,
                _ => false,
            })
            .join("\n");
        if is_list_item {
            body = format! {"- {}", trim_start_up_to(2, &body)}
        }

        let mut self_border = false;

        if body.starts_with("- **") || body.starts_with("- #") {
            body = list_item_to_normal(&body);
            is_list_item = false;
        }
        if self_border_re().is_match(&body) {
            self_border = true;
            body = self_border_re().replace_all(&body, "").to_string();
        }

        let header = body.lines().next().and_then(|l| {
            let c = header_re().captures(l)?;
            Some(c.get(1)?.as_str().to_owned())
        });

        let id = id.map(|id| {
            let obsdn_id = if let Some(header) = &header {
                let h = header_san_re().replace_all(&header, " ").trim().to_string();
                format!("#{h}")
            } else {
                let mut hasher = DefaultHasher::new();
                body.hash(&mut hasher);
                let hash = hasher.finish();
                format!("^{hash:x}")
            };

            let id = Id {
                obsdn_id,
                logseq_id: id.to_string(),
            };
            data.register_id(&id);
            id
        });

        let children = if let Some(first_child) = first_child {
            let num_spaces = first_child
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            let c = first_child.trim_start().chars().next().unwrap();

            let lines = unsafe { union_str(first_child, text) }
                .lines()
                .map(|l| trim_start_up_to(num_spaces, l));

            blocks(lines, c)
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .map(|l| Block::parse(&l, data))
                .try_collect()?
        } else {
            vec![]
        };

        Ok(Self {
            text: body,
            header,
            children,
            id: id.map(Into::into),
            self_border,
            is_list_item,
        })
    }

    pub fn set_list_item(&mut self, is_list_item: bool) {
        if is_list_item == self.is_list_item {
            return;
        }
        self.is_list_item = is_list_item;
        if is_list_item {
            self.text = normal_to_list_item(&self.text);
        } else {
            self.text = list_item_to_normal(&self.text);
        }
    }

    pub fn to_string(&self, is_last: bool) -> String {
        let n = self.children.len().saturating_sub(1);

        let children = self
            .children
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let indent = if c.is_list_item && self.is_list_item {
                    repeat_space(4)
                } else if self.is_list_item {
                    repeat_space(2)
                } else {
                    repeat_space(0)
                };
                c.to_string(i == n)
                    .split("\n")
                    .map(|l| format!("{indent}{l}",))
                    .join("\n")
            })
            .collect_vec()
            .join("\n");
        let text = &self.text;
        let id = self
            .id
            .as_ref()
            .and_then(|id| {
                if self.header.is_some() {
                    return None;
                }
                if self.self_border {
                    Some(format!("\n{}\n", id.obsdn_id))
                } else {
                    Some(format!(" {}", id.obsdn_id))
                }
            })
            .unwrap_or_default();

        let before = (!self.children.is_empty())
            .then_some("\n")
            .unwrap_or_default();
        let after = (is_last && self.children.is_empty())
            .then_some("\n")
            .unwrap_or_default();

        if self.self_border {
            let children = children.trim_end();

            format!("```ad-def\n{text}{before}{children}\n```\n{id}")
        } else {
            format!("{text}{id}{before}{children}{after}")
        }
    }
}

#[derive(strum::EnumIter, strum::AsRefStr, Debug, Clone, Copy)]
pub enum Prop {
    #[strum(serialize = "title::")]
    Title,
    #[strum(serialize = "alias::")]
    Alias,
    #[strum(serialize = "id::")]
    Id,
    #[strum(serialize = "collapsed::")]
    Collapsed,
}
fn parse_prop(line: &str) -> Option<(Prop, &str)> {
    let line = line.trim();

    for e in Prop::iter() {
        if let Some(suffix) = line.strip_prefix(e.as_ref()) {
            return Some((e, suffix.trim_start()));
        }
    }
    None
}

impl Page {
    pub fn to_string(&self) -> String {
        let blocks = self.blocks.iter().map(|b| b.to_string(true)).join("\n");

        let alias = if !self.alias.is_empty() {
            format!("---\naliases: [{}]\n---\n\n", self.alias.join(", "))
        } else {
            String::new()
        };

        format!("{alias}{blocks}")
    }

    pub fn parse(text: &str, data: &mut dyn Data) -> Result<Self> {
        let (title, alias) = {
            let mut title = String::new();
            let mut alias = vec![];

            for (prop, val) in text
                .lines()
                .filter(|l| !l.is_empty())
                .take_while(|l| !l.trim_start().starts_with('-'))
                .filter_map(|l| parse_prop(l))
            {
                match prop {
                    Prop::Alias => alias.push(val.to_string()),
                    Prop::Title => title = val.to_string(),
                    _ => (),
                }
            }
            (title, alias)
        };
        data.page_title(&title);

        let lines = text
            .lines()
            .skip_while(|l| l.trim().is_empty() || !l.starts_with("-"));

        let blocks: Vec<_> = blocks(lines, '-')
            .into_iter()
            .map(|l| Block::parse(&l, data))
            .try_collect()?;

        Ok(Self {
            title,
            alias,
            blocks,
        })
    }

    pub fn transform(&mut self, data: &mut dyn Data) {
        for i in 0..self.blocks.len() {
            let (prev, rest) = self.blocks.as_mut_slice().split_at_mut(i);
            let (curr, _rest) = rest.split_at_mut(1);
            let curr = curr.first_mut().unwrap();
            curr.transform(None, prev.last(), data);
        }
    }
}

fn blocks<'a>(lines: impl Iterator<Item = &'a str> + 'a, delim: char) -> Vec<String> {
    let mut lines = lines.peekable();
    let mut lines_accu: Option<String> = None;

    let mut result = Vec::<String>::new();

    while let Some(line) = lines.next() {
        if line.starts_with(delim) {
            if let Some(accu) = lines_accu.take() {
                result.push(accu);
            }
            lines_accu = Some(String::with_capacity(line.len()));
        }
        if let Some(accu) = &mut lines_accu {
            if !accu.is_empty() {
                accu.push('\n');
            }
            accu.push_str(line);
        }

        if let None = lines.peek() {
            let accu = lines_accu.take().unwrap_or_else(|| line.to_string());
            result.push(accu);
        }
    }

    result
}

unsafe fn union_str<'a>(first: &'a str, last: &'a str) -> &'a str {
    let len = last.as_ptr().offset(last.len() as isize) as usize - first.as_ptr() as usize;
    std::str::from_utf8_unchecked(std::slice::from_raw_parts(first.as_ptr(), len))
}

fn trim_start_up_to(n: usize, s: &str) -> &str {
    let idx = s
        .char_indices()
        .take(n)
        .take_while(|(_, c)| c.is_whitespace())
        .last()
        .map(|(i, c)| (i + c.len_utf8()).min(s.len() - 1))
        .unwrap_or(0);
    &s[idx..]
}

#[test]
fn test_trim_start_up_to() {
    assert_eq!(trim_start_up_to(2, "   a"), " a");
    assert_eq!(trim_start_up_to(0, "   a"), "   a");
    assert_eq!(trim_start_up_to(1, "   a"), "  a");
    assert_eq!(trim_start_up_to(3, "  a"), "a");
}

fn list_item_to_normal(s: &str) -> String {
    let b = s.strip_prefix("- ").unwrap();
    let mut lines = b.lines();
    let first_line = lines.next().unwrap();
    std::iter::once(first_line)
        .chain(lines.map(|l| trim_start_up_to(2, l)))
        .join("\n")
}

fn normal_to_list_item(s: &str) -> String {
    let mut lines = s.lines();
    let mut result = format!("- {}", lines.next().unwrap());
    result.extend(lines.flat_map(|l| ["\n", repeat_space(2), l]));
    result
}

fn repeat_space(n: usize) -> &'static str {
    const LUT: &str = "                ";
    if n > LUT.len() {
        unimplemented!()
    } else {
        &LUT[0..n]
    }
}
