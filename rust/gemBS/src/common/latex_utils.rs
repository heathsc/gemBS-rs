use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt, fs};

use super::html_utils::Table;

pub fn latex_escape_str(s: &str) -> String {
    lazy_static! {
        static ref RE1: Regex = Regex::new(r"([\\])").unwrap();
        static ref RE2: Regex = Regex::new(r"([&$_{}])").unwrap();
    }
    let s = RE1.replace_all(s, "\\textbackslash ");
    RE2.replace_all(&s, |caps: &Captures| format!("\\{}", &caps[1]))
        .into_owned()
}

pub enum LatexContent {
    Text(String),
    SecArray(SectionArray),
    Table(LatexTable),
}

impl fmt::Display for LatexContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"([%#])").unwrap();
        }
        match self {
            LatexContent::Text(s) => writeln!(
                f,
                "{}",
                RE.replace_all(s, |caps: &Captures| { format!("\\{}", &caps[1]) })
                    .into_owned()
            ),
            LatexContent::SecArray(s) => writeln!(f, "{}", s),
            LatexContent::Table(s) => writeln!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PageSize {
    A4,
    Letter,
}

impl FromStr for PageSize {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "a4" => Ok(PageSize::A4),
            "letter" => Ok(PageSize::Letter),
            _ => Err("PageSize: no match"),
        }
    }
}

pub struct LatexTable {
    header: Vec<&'static str>,
    rows: Vec<Vec<String>>,
    col_desc: Option<String>,
}

impl LatexTable {
    pub fn new() -> Self {
        LatexTable {
            header: Vec::new(),
            rows: Vec::new(),
            col_desc: None,
        }
    }
    pub fn set_col_desc(&mut self, desc: &str) {
        self.col_desc = Some(desc.to_owned());
    }
}
impl Table for LatexTable {
    fn add_header(&mut self, hdr: Vec<&'static str>) -> &mut Self {
        self.header = hdr;
        self
    }
    fn add_row(&mut self, row: Vec<String>) -> &mut Self {
        self.rows.push(row);
        self
    }
}

impl fmt::Display for LatexTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"([%#])").unwrap();
        }
        let mut ncol = 1;
        if self.header.len() > ncol {
            ncol = self.header.len()
        }
        for r in self.rows.iter() {
            if r.len() > ncol {
                ncol = r.len();
            }
        }
        let s = if let Some(desc) = &self.col_desc {
            desc.clone()
        } else {
            let mut ts = "|".to_string();
            for _ in 0..ncol {
                ts.push_str("l|");
            }
            ts
        };
        let start_row = if self.header.is_empty() { 1 } else { 2 };
        writeln!(
            f,
            "{{\\rowcolors{{{}}}{{green!10!white!90}}{{blue!10!white!90}}",
            start_row
        )?;
        writeln!(f, "\\begin{{tabular}}{{{}}}\n\\hline", s)?;
        // Header row
        if !self.header.is_empty() {
            let mut first = true;
            for s in self.header.iter() {
                if first {
                    first = false
                } else {
                    write!(f, " & ")?
                }
                write!(
                    f,
                    "{{\\small\\textbf{{{}}}}}",
                    RE.replace_all(s, |caps: &Captures| { format!("\\{}", &caps[1]) })
                        .into_owned()
                )?;
            }
            writeln!(f, "\\\\\n\\hline")?;
        }
        for r in self.rows.iter() {
            if r.is_empty() {
                writeln!(f, "\\hline")?;
            } else {
                let mut first = true;
                for s in r.iter() {
                    if first {
                        first = false
                    } else {
                        write!(f, " & ")?
                    }
                    write!(
                        f,
                        "{{\\small {}}}",
                        RE.replace_all(s, |caps: &Captures| { format!("\\{}", &caps[1]) })
                            .into_owned()
                    )?;
                }
                writeln!(f, "\\\\")?;
            }
        }
        writeln!(f, "\\hline\n\\end{{tabular}}}}")?;
        Ok(())
    }
}

pub struct LatexSection {
    sort_tag: String,
    content: Vec<LatexContent>,
}

impl fmt::Display for LatexSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for x in self.content.iter() {
            write!(f, "{}", x)?
        }
        Ok(())
    }
}

impl LatexSection {
    pub fn new(tag: &str) -> Self {
        LatexSection {
            sort_tag: tag.to_owned(),
            content: Vec::new(),
        }
    }
    pub fn push(&mut self, content: LatexContent) {
        self.content.push(content)
    }
    pub fn push_str(&mut self, s: &str) {
        self.content.push(LatexContent::Text(s.to_string()))
    }
    pub fn push_string(&mut self, s: String) {
        self.content.push(LatexContent::Text(s))
    }
    pub fn content(&mut self) -> &mut Vec<LatexContent> {
        &mut self.content
    }
}

pub struct SectionArray(Vec<LatexSection>);

impl SectionArray {
    pub fn new() -> Self {
        SectionArray(Vec::new())
    }
    pub fn push(&mut self, s: LatexSection) {
        self.0.push(s)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for SectionArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // self can not be mutable so we will have to sort a temporary index array
        let l = self.0.len();
        let mut idx: Vec<usize> = (0..l).collect();
        idx.sort_by(|a, b| self.0[*a].sort_tag.cmp(&self.0[*b].sort_tag));
        for x in idx.iter() {
            write!(f, "{}", self.0[*x])?
        }
        Ok(())
    }
}

// Just a bare collection of latex sections with no preamble etc.
pub struct LatexBare {
    body: Vec<LatexContent>,
    sections: SectionArray,
    path: PathBuf,
    sec_hash: HashMap<String, usize>,
}

impl LatexBare {
    pub fn new(path: &Path) -> Result<Self, String> {
        Ok(LatexBare {
            sections: SectionArray::new(),
            body: Vec::new(),
            path: path.to_owned(),
            sec_hash: HashMap::new(),
        })
    }
    pub fn push_section(&mut self, s: LatexSection) -> Result<(), String> {
        if self.sec_hash.contains_key(&s.sort_tag) {
            Err(format!(
                "Error inserting LatexSection: tag {} already exists",
                s.sort_tag
            ))
        } else {
            self.sec_hash
                .insert(s.sort_tag.clone(), self.sections.len());
            self.sections.push(s);
            Ok(())
        }
    }
    pub fn find_section(&mut self, tag: &str) -> Option<&mut LatexSection> {
        match self.sec_hash.get(tag) {
            None => None,
            Some(x) => Some(&mut self.sections.0[*x]),
        }
    }
    pub fn push(&mut self, c: LatexContent) {
        self.body.push(c)
    }
}

impl Drop for LatexBare {
    fn drop(&mut self) {
        if let Ok(ofile) = fs::File::create(&self.path) {
            let mut writer = Box::new(BufWriter::new(ofile));
            for x in &self.body {
                let _ = write!(writer, "{}", x);
            }
            let _ = write!(writer, "{}", self.sections);
        }
    }
}
