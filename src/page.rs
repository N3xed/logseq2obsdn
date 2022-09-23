use std::collections::HashMap;
use std::iter::Peekable;
use std::path::PathBuf;
use std::ptr::slice_from_raw_parts_mut;
use std::str::Lines;

use anyhow::Result;
use itertools::Itertools;
use strum::IntoEnumIterator;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct RefsFile {
    refs: HashMap<String, Ref>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Ref {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct Page {
    pub title: String,
    pub alias: Vec<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub struct Block {
    pub text: String,
    pub id: Option<String>,
    pub children: Vec<Block>,
}

impl Block {
    pub fn parse(text: &str) -> Result<Self> {
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

        let children = if let Some(first_child) = first_child {
            let num_spaces = first_child
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            let c = first_child.trim_start().chars().next().unwrap();

            let lines = unsafe { union_str(first_child, text) }.lines().map(|l| {
                let n = l
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .count()
                    .min(num_spaces);
                &l[n..]
            });

            blocks(lines, c)
                .into_iter()
                .map(|l| Block::parse(l))
                .try_collect()?
        } else {
            vec![]
        };

        Ok(Self {
            text: body.to_string(),
            children,
            id: id.map(Into::into),
        })
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
    pub fn parse(text: &str) -> Result<Self> {
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

        let lines = text
            .lines()
            .skip_while(|l| l.trim().is_empty() || !l.starts_with("-"));

        let blocks: Vec<_> = blocks(lines, '-')
            .into_iter()
            .map(|l| Block::parse(l))
            .try_collect()?;

        Ok(Self {
            title,
            alias,
            blocks,
        })
    }
}

fn blocks<'a>(mut lines: impl Iterator<Item = &'a str> + 'a, delim: char) -> Vec<&'a str> {
    let mut start_line = None;
    let mut last_line = None;

    let mut result = Vec::<&str>::new();

    while let Some(line) = lines.next() {
        if line.starts_with(delim) {
            if let Some(start) = start_line.take() {
                result.push(unsafe { union_str(start, last_line.take().unwrap()) });
            }
            start_line = Some(line);
        }
        last_line = Some(line);
    }
    result
}

unsafe fn union_str<'a>(first: &'a str, last: &'a str) -> &'a str {
    let len = last.as_ptr().offset(last.len() as isize) as usize - first.as_ptr() as usize;
    std::str::from_utf8_unchecked(std::slice::from_raw_parts(first.as_ptr(), len))
}
