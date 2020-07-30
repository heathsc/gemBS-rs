use std::path::{Path, PathBuf};
use std::io::{Write, BufWriter};
use std::{fs, fmt};
use regex::{Regex, Captures};
use lazy_static::lazy_static;

use super::html_utils::Table;

pub fn latex_escape_str(s: &str) -> String {
	lazy_static! { 
		static ref RE1: Regex = Regex::new(r"([\\])").unwrap(); 
		static ref RE2: Regex = Regex::new(r"([#&$_{}])").unwrap(); 
	}
	let s = RE1.replace_all(s, "\\textbackslash ");
	RE2.replace_all(&s, |caps: &Captures| { format!("\\{}", &caps[1]) }).into_owned()
}

pub enum LatexContent {
	Text(String),
	Env(LatexEnv),
	SecArray(SectionArray),
	Table(LatexTable),
}

impl fmt::Display for LatexContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		lazy_static! { static ref RE: Regex = Regex::new(r"([%])").unwrap(); }
		match self {
			LatexContent::Text(s) => writeln!(f, "{}", RE.replace_all(s, "\\%")),
			LatexContent::Env(s) => writeln!(f, "{}", s),
			LatexContent::SecArray(s) => writeln!(f, "{}", s),
			LatexContent::Table(s) => writeln!(f, "{}", s),
		}
	}		
}

pub enum PageSize {
	A4,
	Letter,
}

pub struct LatexEnv {
	name: &'static str,
	content: Vec<LatexContent>,
}

impl fmt::Display for LatexEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "\\begin{{{}}}", self.name)?;
		if !self.content.is_empty() {
			writeln!(f)?;
			for x in self.content.iter() { write!(f, "{}", x)? }
		}
		writeln!(f, "\\end{{{}}}", self.name)?;
		Ok(())
	}
}

impl LatexEnv {
	pub fn new(name: &'static str) -> Self { 
		LatexEnv{ name, content: Vec::new() }
	}
	pub fn push(&mut self, content: LatexContent) { self.content.push(content) }	
	pub fn push_str(&mut self, s: &str) { self.content.push(LatexContent::Text(s.to_string())) }
	pub fn push_string(&mut self, s: String) { self.content.push(LatexContent::Text(s)) }
	pub fn push_env(&mut self, e: LatexEnv) { self.content.push(LatexContent::Env(e)) }
}

pub struct LatexTable {
	header: Vec<&'static str>,
	rows: Vec<Vec<String>>,
}

impl LatexTable {
	pub fn new() -> Self {
		LatexTable{header: Vec::new(), rows: Vec::new() }
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
		lazy_static! { static ref RE: Regex = Regex::new(r"([%])").unwrap(); }
		let mut ncol = 1;
		if self.header.len() > ncol { ncol = self.header.len() }
		for r in self.rows.iter() { 
			if r.len() > ncol { ncol = r.len(); }
		}
		let mut s = "|".to_string();
		for _ in 0..ncol {s.push_str("c|");}
		writeln!(f, "\\begin{{tabular}}{{{}}}\n\\hline", s)?;		
		// Header row
		if !self.header.is_empty() {
			let mut first = true;
			for s in self.header.iter() {
				if first { first = false } else { write!(f, " & ")? } 
				write!(f, "{}", RE.replace_all(s, "\\%"))?
			}
			writeln!(f, "\\\\\n\\hline")?;
		}
		for r in self.rows.iter() {
			if r.is_empty() { writeln!(f, "\\hline")?; } else {
				let mut first = true;
				for s in r.iter() {
					if first { first = false } else { write!(f, " & ")? } 
					write!(f, "{{\\small {}}}", RE.replace_all(s, "\\%"))?
				}
			}
			writeln!(f, "\\\\")?;
		}
		writeln!(f, "\\hline\n\\end{{tabular}}")?;		
		Ok(())
	}
}

pub struct LatexSection {
	sort_tag: String,
	content: Vec<LatexContent>,	
}

impl fmt::Display for LatexSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for x in self.content.iter() { write!(f, "{}", x)? }
		Ok(())
	}
}

impl LatexSection {
	pub fn new(tag: &str) -> Self {
		LatexSection{ sort_tag: tag.to_owned(), content: Vec::new() }
	}
	pub fn push_env(&mut self, e: LatexEnv) { self.content.push(LatexContent::Env(e)) }
	pub fn push(&mut self, content: LatexContent) { self.content.push(content) }	
	pub fn push_str(&mut self, s: &str) { self.content.push(LatexContent::Text(s.to_string())) }
	pub fn push_string(&mut self, s: String) { self.content.push(LatexContent::Text(s)) }
}

pub struct SectionArray(Vec<LatexSection>);

impl SectionArray {
	pub fn new() -> Self { SectionArray(Vec::new()) }
	pub fn push(&mut self, s: LatexSection) { self.0.push(s) }
}

impl fmt::Display for SectionArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// self can not be mutable so we will have to sort a temporary index array
		let l = self.0.len();
		let mut idx: Vec<usize> = (0..l).collect();
		idx.sort_by(|a, b| self.0[*a].sort_tag.cmp(&self.0[*b].sort_tag));
		for x in idx.iter() { write!(f, "{}", self.0[*x])? }
		Ok(())
	}
}

pub struct LatexDoc {
	title: String,
	author: String,
	page_size: PageSize,
	body: Vec<LatexContent>,
	sections: SectionArray,
	path: PathBuf,
//	writer: Box<dyn Write>,
}

impl LatexDoc {
	pub fn new(path: &Path, page_size: PageSize, title: &str, author: &str) -> Result<Self, String> { 
		Ok(LatexDoc{ title: latex_escape_str(title), author: author.to_owned(), page_size, sections: SectionArray::new(), body: Vec::new(), path: path.to_owned() })
	}
	pub fn push_section(&mut self, s: LatexSection) { self.sections.push(s) }
	pub fn push(&mut self, c: LatexContent) { self.body.push(c) }
}

impl Drop for LatexDoc {
	fn drop(&mut self) {
		if let Ok(ofile) = fs::File::create(&self.path) {
			let mut writer = Box::new(BufWriter::new(ofile));
			let sz = match self.page_size {
				PageSize::A4 => "a4paper",
				PageSize::Letter => "letter",
			};
			let _ = writeln!(writer, "\\documentclass[12pt]{{article}}");
			let _ = writeln!(writer, "\\usepackage{{geometry}}\n\\geometry{{{}, left=15mm, top=20mm}}", sz);
			let _ = writeln!(writer, "\\usepackage{{graphicx}}");
			let _ = writeln!(writer, "\\usepackage{{hyperref}}\n\\hypersetup{{colorlinks=true}}");
			let _ = writeln!(writer, "\\title{{{}}}", self.title);
			let _ = writeln!(writer, "\\author{{{}}}", self.author);
			let _ = writeln!(writer, "\\date{{\\today}}");
			let _ = writeln!(writer, "\\begin{{document}}");
			let _ = writeln!(writer, "\\begin{{titlepage}}\n\\maketitle\n\\tableofcontents\n\\end{{titlepage}}");
			for x in &self.body { let _ = write!(writer, "{}", x); }
			let _ = write!(writer, "{}", self.sections);
			let _ = writeln!(writer, "\\end{{document}}");
		}
	}
}
