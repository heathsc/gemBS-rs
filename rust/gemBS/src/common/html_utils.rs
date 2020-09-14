use std::path::Path;
use std::io::{Write, BufWriter};
use std::{fs, fmt};

pub trait Table {
	fn add_header(&mut self, hdr: Vec<&'static str>) -> &mut Self;
	fn add_row(&mut self, row: Vec<String>) -> &mut Self;	
}

pub enum Content {
	Text(String),
	Element(HtmlElement),
	Table(HtmlTable),
}

impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Content::Text(s) => writeln!(f, "{}", s),
			Content::Element(s) => writeln!(f, "{}", s),
			Content::Table(s) => writeln!(f, "{}", s),
		}
	}		
}

pub struct HtmlElement {
	tag: &'static str,
	options: Option<String>,
	close: bool,
	content: Vec<Content>,
}

impl fmt::Display for HtmlElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if let Some(opt) = &self.options { write!(f, "<{} {}>", self.tag, opt)? }
		else { write!(f, "<{}>", self.tag)? }
		if !self.content.is_empty() {
			writeln!(f)?;
			for x in self.content.iter() { write!(f, "{}", x)? }
		}
		if self.close { write!(f, "</{}>", self.tag)? }
		Ok(())
	}
}

impl HtmlElement {
	pub fn new(tag: &'static str, opt: Option<&str>, close: bool) -> Self { 
		let options = if let Some(s) = opt { Some(s.to_owned()) } else { None };
		HtmlElement{ tag, options, close, content: Vec::new() }
	}
	pub fn push(&mut self, content: Content) { self.content.push(content) }	
	pub fn push_str(&mut self, s: &str) { self.content.push(Content::Text(s.to_string())) }
	pub fn push_string(&mut self, s: String) { self.content.push(Content::Text(s)) }
	pub fn push_element(&mut self, e: HtmlElement) { self.content.push(Content::Element(e)) }
}

pub struct HtmlTable {
	id: &'static str,
	header: Vec<&'static str>,
	rows: Vec<Vec<String>>,
}

impl HtmlTable {
	pub fn new(id: &'static str) -> Self {
		HtmlTable{id, header: Vec::new(), rows: Vec::new() }
	}
}

impl Table for HtmlTable {
	fn add_header(&mut self, hdr: Vec<&'static str>) -> &mut Self {
		self.header = hdr;
		self
	}
	fn add_row(&mut self, row: Vec<String>) -> &mut Self {
		self.rows.push(row);
		self
	}
}

impl fmt::Display for HtmlTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "<TABLE id=\"{}\">", self.id)?;
		// Header row
		if !self.header.is_empty() {
			writeln!(f, "<TR>")?;
			for s in self.header.iter() { write!(f, "<TH scope=\"col\">{}</TH>", s)? }
			writeln!(f, "</TR>")?;
		}
		// Get number of rows
		let mut n_rows = 0;
		for r in self.rows.iter() {
			if !r.is_empty() {
				n_rows = r.len();
				break;
			}
		}
		// Other rows
		let mut odd = false;
		for r in self.rows.iter() {
			if r.is_empty() {
				writeln!(f, "<TR class=\"empty\">")?;
				for _ in 0..n_rows { write!(f, "<TD></TD>")? }				
			} else {
				if odd { writeln!(f, "<TR class=\"odd\">")? }
				else { writeln!(f, "<TR>")? }
				for s in r.iter() { write!(f, "<TD>{}</TD>", s)? }
				odd = !odd;
			}
			writeln!(f, "</TR>")?;
		}
		writeln!(f, "</TABLE>")?;
		Ok(())
	}
}

pub struct HtmlPage {
	content: Vec<Content>,
	writer: Box<dyn Write>,
}

impl HtmlPage {
	pub fn new(path: &Path) -> Result<Self, String> { 
		let ofile = match fs::File::create(path) {
			Err(e) => return Err(format!("Couldn't open {}: {}", path.to_string_lossy(), e)),
			Ok(f) => f,
		};
		let writer = Box::new(BufWriter::new(ofile));
		Ok(HtmlPage{ content: Vec::new(), writer })
	}
	pub fn push_element(&mut self, e: HtmlElement) { self.content.push(Content::Element(e)) }

}

impl Drop for HtmlPage {
	fn drop(&mut self) {
		let _ = writeln!(self.writer, "<HTML>");
		for x in self.content.iter() {
			let _ = write!(self.writer, "{}", x);
		}
		let _ = writeln!(self.writer, "</HTML>");
	}
}

