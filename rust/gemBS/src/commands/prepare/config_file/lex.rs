use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::str;
use std::str::FromStr;

use crate::common::defs::Section;
use crate::common::utils::get_inode;
use utils::compress;

struct InFile {
    name: String,
    inode: u64,
    line: usize, // Line in file (zero offset)
    pos: usize,  // position within line (zero offset)
    section: Section,
    bufreader: Box<dyn BufRead>,
}

impl InFile {
    fn new(name: &str, path: &Path) -> Option<Self> {
        let mut pb = PathBuf::from(name);
        if !pb.exists() {
            let tpb: PathBuf = [path, &pb].iter().collect();
            if tpb.exists() {
                pb = tpb
            } else {
                error!("Could not find config file {}", name);
                return None;
            }
        }
        let pname = pb.to_string_lossy();
        match get_inode(&pname) {
            Some(inode) => match compress::open_bufreader(&pb) {
                Ok(reader) => Some(InFile {
                    name: pname.to_string(),
                    inode,
                    line: 0,
                    pos: 0,
                    section: Section::Default,
                    bufreader: reader,
                }),
                Err(_) => {
                    error!("Could not open config file {}", name);
                    None
                }
            },
            None => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LexRawToken {
    LeftSquareBracket,
    RightSquareBracket,
    Hash,
    Equals,
    Comma,
    DoubleQuote,
    SingleQuote,
    Letter,
    Punct,
    Number,
    WhiteSpace,
    LineFeed,
    End,
    Invalid, // Printable but not valid outside of comments or quotes
    Illegal, // Control or non-ascii
    Null,    // Do nothing token
}

#[derive(Debug, Clone, Copy)]
pub enum IntLexToken {
    Section,
    Name,
    Value,
    Include,
    Null,
    End,
}

// These are the tokens that are emitted by the parser
#[derive(Debug)]
pub enum LexToken {
    Name(String),
    Value(String),
    Include(String),
    Section(String),
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteMode {
    Quoted(LexRawToken),
    Comment,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LexState {
    Init,
    InName,
    AfterName,
    AfterEquals,
    InValue,
    AfterValue,
    AfterBracket,
    InSection,
    AfterSection,
    InInclude,
    End,
}

#[derive(Debug, Clone, Copy)]
struct LexAction {
    new_state: LexState,
    consume: bool,
    emit: Option<IntLexToken>,
}

struct LexBuf<'a> {
    buffer: &'a [u8],
    start: usize,
    end: usize,
}

impl<'a> LexBuf<'a> {
    fn len(&self) -> usize {
        self.buffer.len()
    }
    fn get(&self) -> Option<u8> {
        if self.end < self.buffer.len() {
            Some(self.buffer[self.end])
        } else {
            None
        }
    }
    fn get_str_range(&self) -> Option<&'a str> {
        if self.end > self.start {
            match str::from_utf8(&self.buffer[self.start..self.end]) {
                Ok(s) => Some(s),
                Err(_) => None,
            }
        } else {
            None
        }
    }
    fn get_raw_token(&self, raw_token_tab: &[LexRawToken]) -> Option<LexRawToken> {
        if self.end == self.len() {
            if self.len() == 0 {
                Some(LexRawToken::End)
            } else {
                None
            }
        } else {
            self.get().map(|c| raw_token_tab[c as usize])
        }
    }
}

pub struct Lexer {
    action_table: HashMap<LexState, HashMap<LexRawToken, LexAction>>,
    raw_token_tab: [LexRawToken; 256],
    in_files: Vec<InFile>,
    state: LexState,
    tbuf: String,
    config_script_path: PathBuf,
    quote_mode: QuoteMode,
}

impl Lexer {
    pub fn new(config_script_path: &Path) -> Self {
        let mut tab = [LexRawToken::Illegal; 256];
        for t in &mut tab[32..127] {
            *t = LexRawToken::Invalid;
        }
        tab[b'\n' as usize] = LexRawToken::LineFeed;
        tab[b' ' as usize] = LexRawToken::WhiteSpace;
        tab[b'\t' as usize] = LexRawToken::WhiteSpace;
        tab[b'\r' as usize] = LexRawToken::LineFeed;
        tab[12] = LexRawToken::WhiteSpace; // Line Feed
        tab[11] = LexRawToken::WhiteSpace; // Form Feed
        tab[39] = LexRawToken::SingleQuote; // Single quote
        tab[b'$' as usize] = LexRawToken::Punct;
        tab[b',' as usize] = LexRawToken::Comma;
        tab[b'=' as usize] = LexRawToken::Equals;
        tab[b'"' as usize] = LexRawToken::DoubleQuote;
        tab[b'{' as usize] = LexRawToken::Punct;
        tab[b'}' as usize] = LexRawToken::Punct;
        tab[b'[' as usize] = LexRawToken::LeftSquareBracket;
        tab[b']' as usize] = LexRawToken::RightSquareBracket;
        tab[b'#' as usize] = LexRawToken::Hash;
        tab[b'\n' as usize] = LexRawToken::LineFeed;
        tab[b'/' as usize] = LexRawToken::Punct;
        tab[b':' as usize] = LexRawToken::Punct;
        tab[b'_' as usize] = LexRawToken::Letter;
        tab[b'@' as usize] = LexRawToken::Letter;
        tab[b'*' as usize] = LexRawToken::Letter;
        tab[b'?' as usize] = LexRawToken::Letter;
        tab[b'\n' as usize] = LexRawToken::LineFeed;
        tab[b'.' as usize] = LexRawToken::Number;
        tab[b'-' as usize] = LexRawToken::Number;
        for x in b'A'..=b'Z' {
            tab[x as usize] = LexRawToken::Letter;
        }
        for x in b'a'..=b'z' {
            tab[x as usize] = LexRawToken::Letter;
        }
        for x in b'0'..=b'9' {
            tab[x as usize] = LexRawToken::Number;
        }
        Lexer {
            action_table: HashMap::new(),
            raw_token_tab: tab,
            in_files: Vec::new(),
            state: LexState::Init,
            tbuf: String::new(),
            config_script_path: config_script_path.to_owned(),
            quote_mode: QuoteMode::None,
        }
    }

    fn add_in_file(&mut self, name: &str) -> Result<(), String> {
        match InFile::new(name, &self.config_script_path) {
            Some(cf) => {
                for f in &self.in_files {
                    if f.inode == cf.inode {
                        error!(
                            "Loop detected when reading config files: {} and {} are the same file",
                            f.name, cf.name
                        );
                        return Err(format!(
                            "Loop detected when reading config files: {} and {} are the same file",
                            f.name, cf.name
                        ));
                    }
                }
                self.in_files.push(cf);
                Ok(())
            }
            None => Err(format!("Could not open config file {}", name)),
        }
    }

    fn push_file(&mut self, name: &str) -> Result<(), String> {
        self.add_in_file(name)?;
        self.state = LexState::Init;
        self.tbuf.clear();
        Ok(())
    }

    fn pop_file(&mut self) -> Option<InFile> {
        self.state = LexState::Init;
        self.tbuf.clear();
        self.in_files.pop()
    }

    fn set_section(&mut self, section: Section) -> Result<(), &'static str> {
        if let Some(file) = self.in_files.last_mut() {
            file.section = section;
            Ok(())
        } else {
            Err("No input files for Lexer")
        }
    }

    pub fn get_section(&self) -> Option<Section> {
        self.in_files.last().map(|file| file.section)
    }

    fn get_file_pos_str(&self) -> Option<String> {
        self.in_files.last().map(|file| {
            format!(
                "{}: line {}, pos {}",
                file.name,
                file.line + 1,
                file.pos + 1
            )
        })
    }
    fn add_lex_action(
        &mut self,
        state: LexState,
        raw_token: LexRawToken,
        new_state: LexState,
        consume: bool,
        emit: Option<IntLexToken>,
    ) {
        self.action_table
            .entry(state)
            .or_insert_with(HashMap::new)
            .insert(
                raw_token,
                LexAction {
                    new_state,
                    consume,
                    emit,
                },
            );
    }

    fn setup_action_table(&mut self) {
        self.add_lex_action(
            LexState::Init,
            LexRawToken::WhiteSpace,
            LexState::Init,
            true,
            None,
        );
        self.add_lex_action(
            LexState::Init,
            LexRawToken::LineFeed,
            LexState::Init,
            true,
            None,
        );
        self.add_lex_action(
            LexState::Init,
            LexRawToken::LeftSquareBracket,
            LexState::AfterBracket,
            true,
            None,
        );
        self.add_lex_action(
            LexState::Init,
            LexRawToken::Letter,
            LexState::InName,
            false,
            None,
        );
        self.add_lex_action(
            LexState::Init,
            LexRawToken::End,
            LexState::End,
            false,
            Some(IntLexToken::End),
        );
        self.add_lex_action(
            LexState::InName,
            LexRawToken::Letter,
            LexState::InName,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InName,
            LexRawToken::Number,
            LexState::InName,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InName,
            LexRawToken::Punct,
            LexState::InName,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InName,
            LexRawToken::LineFeed,
            LexState::AfterName,
            true,
            Some(IntLexToken::Name),
        );
        self.add_lex_action(
            LexState::InName,
            LexRawToken::WhiteSpace,
            LexState::AfterName,
            true,
            Some(IntLexToken::Name),
        );
        self.add_lex_action(
            LexState::InName,
            LexRawToken::Equals,
            LexState::AfterName,
            false,
            Some(IntLexToken::Name),
        );
        self.add_lex_action(
            LexState::AfterName,
            LexRawToken::Equals,
            LexState::AfterEquals,
            true,
            Some(IntLexToken::Null),
        );
        self.add_lex_action(
            LexState::AfterName,
            LexRawToken::WhiteSpace,
            LexState::AfterName,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterName,
            LexRawToken::LineFeed,
            LexState::AfterName,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterEquals,
            LexRawToken::WhiteSpace,
            LexState::AfterEquals,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterEquals,
            LexRawToken::LineFeed,
            LexState::AfterEquals,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterEquals,
            LexRawToken::Letter,
            LexState::InValue,
            false,
            None,
        );
        self.add_lex_action(
            LexState::AfterEquals,
            LexRawToken::Number,
            LexState::InValue,
            false,
            None,
        );
        self.add_lex_action(
            LexState::AfterEquals,
            LexRawToken::Punct,
            LexState::InValue,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::Letter,
            LexState::InValue,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::Number,
            LexState::InValue,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::Punct,
            LexState::InValue,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::LineFeed,
            LexState::AfterValue,
            true,
            Some(IntLexToken::Value),
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::WhiteSpace,
            LexState::AfterValue,
            true,
            Some(IntLexToken::Value),
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::End,
            LexState::Init,
            false,
            Some(IntLexToken::Value),
        );
        self.add_lex_action(
            LexState::InValue,
            LexRawToken::Comma,
            LexState::AfterValue,
            false,
            Some(IntLexToken::Value),
        );
        self.add_lex_action(
            LexState::AfterValue,
            LexRawToken::Comma,
            LexState::AfterEquals,
            true,
            Some(IntLexToken::Null),
        );
        self.add_lex_action(
            LexState::AfterValue,
            LexRawToken::WhiteSpace,
            LexState::AfterValue,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterValue,
            LexRawToken::LineFeed,
            LexState::AfterValue,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterValue,
            LexRawToken::Letter,
            LexState::InName,
            false,
            None,
        );
        self.add_lex_action(
            LexState::AfterValue,
            LexRawToken::LeftSquareBracket,
            LexState::AfterBracket,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterValue,
            LexRawToken::End,
            LexState::End,
            false,
            Some(IntLexToken::End),
        );
        self.add_lex_action(
            LexState::AfterBracket,
            LexRawToken::WhiteSpace,
            LexState::AfterBracket,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterBracket,
            LexRawToken::LineFeed,
            LexState::AfterBracket,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterBracket,
            LexRawToken::Letter,
            LexState::InSection,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InSection,
            LexRawToken::Letter,
            LexState::InSection,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InSection,
            LexRawToken::Number,
            LexState::InSection,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InSection,
            LexRawToken::Punct,
            LexState::InSection,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InSection,
            LexRawToken::WhiteSpace,
            LexState::AfterSection,
            true,
            Some(IntLexToken::Section),
        );
        self.add_lex_action(
            LexState::InSection,
            LexRawToken::LineFeed,
            LexState::AfterSection,
            true,
            None,
        );
        self.add_lex_action(
            LexState::InSection,
            LexRawToken::RightSquareBracket,
            LexState::Init,
            true,
            Some(IntLexToken::Section),
        );
        self.add_lex_action(
            LexState::AfterSection,
            LexRawToken::RightSquareBracket,
            LexState::Init,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterSection,
            LexRawToken::WhiteSpace,
            LexState::AfterSection,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterSection,
            LexRawToken::LineFeed,
            LexState::AfterSection,
            true,
            None,
        );
        self.add_lex_action(
            LexState::AfterSection,
            LexRawToken::End,
            LexState::End,
            true,
            Some(IntLexToken::End),
        );
        self.add_lex_action(
            LexState::InInclude,
            LexRawToken::Letter,
            LexState::InInclude,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InInclude,
            LexRawToken::Number,
            LexState::InInclude,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InInclude,
            LexRawToken::Punct,
            LexState::InInclude,
            false,
            None,
        );
        self.add_lex_action(
            LexState::InInclude,
            LexRawToken::LineFeed,
            LexState::Init,
            true,
            Some(IntLexToken::Name),
        );
        self.add_lex_action(
            LexState::InInclude,
            LexRawToken::WhiteSpace,
            LexState::Init,
            true,
            Some(IntLexToken::Name),
        );
        self.add_lex_action(
            LexState::InInclude,
            LexRawToken::End,
            LexState::Init,
            false,
            Some(IntLexToken::Name),
        );
    }

    pub fn init_lexer(&mut self, name: &str) -> Result<(), String> {
        self.push_file(name)?;
        self.setup_action_table();
        Ok(())
    }

    fn int_get_token(&mut self) -> Result<LexToken, String> {
        let in_file = match self.in_files.last_mut() {
            Some(file) => file,
            None => return Ok(LexToken::End),
        };
        let reader = &mut in_file.bufreader;
        let action_table = &self.action_table;
        let tbuf = &mut self.tbuf;
        let raw_token_tab = self.raw_token_tab;
        let mut buffer = match reader.fill_buf() {
            Ok(buf) => LexBuf {
                buffer: buf,
                start: 0,
                end: 0,
            },
            Err(_) => {
                return Err(format!(
                    "Error reading data from config file {}",
                    in_file.name
                ))
            }
        };
        let len = buffer.len();
        trace!("len bytes read in {}", len);
        while buffer.end <= len {
            // Get new raw token
            let rawtok = if let Some(x) = buffer.get_raw_token(&raw_token_tab) {
                x
            } else {
                break;
            };
            trace!(
                "Got raw token {}:{}.{} {:?} end:{} start:{}",
                in_file.name,
                in_file.line + 1,
                in_file.pos + 1,
                rawtok,
                buffer.end,
                buffer.start
            );
            let new_line = rawtok == LexRawToken::LineFeed;

            // Handle quote and comment modes
            let x = handle_quotes_and_comments(self.quote_mode, rawtok, tbuf, &buffer);
            self.quote_mode = x.0;
            let rawtok = x.1;
            buffer.start = x.2;

            // Dummy token produced when switching to and from quote or comment mode
            if rawtok == LexRawToken::Null {
                buffer.end += 1;
                in_file.pos += 1;
            } else {
                // Get new action
                let optact = match action_table.get(&self.state) {
                    Some(v) => v.get(&rawtok),
                    None => None,
                };
                match optact {
                    // Handle action
                    Some(action) => {
                        trace!("Got action {:?}", action);
                        // End of a IntLexToken - we should emit or otherwise handle token
                        if let Some(tok) = action.emit {
                            // Copy command to ostr
                            let ostr = match tok {
                                IntLexToken::Name | IntLexToken::Value | IntLexToken::Section => {
                                    if let Some(s) = buffer.get_str_range() {
                                        tbuf.push_str(s);
                                    }
                                    Some(tbuf.clone())
                                }
                                _ => None,
                            };
                            in_file.pos += 1;

                            // Handle include commands.
                            let s = handle_include(self.state, *action, tok, tbuf.as_str());
                            let emit = s.0;
                            self.state = s.1;
                            let token = s.2;

                            // Clean up for next command
                            tbuf.clear();

                            // Deal with new line
                            if new_line {
                                in_file.line += 1;
                                in_file.pos = 0;
                            }

                            if emit {
                                trace!(
                                    "Emitting {:?} {:?} and consuming {}, state = {:?}",
                                    tok,
                                    ostr,
                                    buffer.end,
                                    self.state
                                );
                                if action.consume {
                                    buffer.end += 1
                                }
                                let end = buffer.end;
                                drop(buffer);
                                reader.consume(end);
                                let etok = match token {
                                    IntLexToken::End => Ok(LexToken::End),
                                    IntLexToken::Name => {
                                        if let Some(x) = ostr {
                                            Ok(LexToken::Name(x))
                                        } else {
                                            Err("Internal error - empty Name".to_string())
                                        }
                                    }
                                    IntLexToken::Value => {
                                        if let Some(x) = ostr {
                                            Ok(LexToken::Value(x))
                                        } else {
                                            Err("Internal error - empty Value".to_string())
                                        }
                                    }
                                    IntLexToken::Include => {
                                        if let Some(x) = ostr {
                                            Ok(LexToken::Include(x))
                                        } else {
                                            Err("Internal error - empty Value".to_string())
                                        }
                                    }
                                    IntLexToken::Section => {
                                        if let Some(x) = ostr {
                                            Ok(LexToken::Section(x))
                                        } else {
                                            Err("Internal error - empty Value".to_string())
                                        }
                                    }
                                    _ => {
                                        Err("Internal error - unexpected emitted token".to_string())
                                    }
                                };
                                return etok;
                            } else {
                                buffer.start = buffer.end + 1;
                            }
                        } else {
                            self.state = action.new_state;
                            if action.consume {
                                buffer.start += 1
                            }
                            in_file.pos += 1;
                        }
                        buffer.end += 1;
                    }
                    None => {
                        return Err(format!(
                            "Unexpected token in config file {} at line {}, col {}",
                            in_file.name,
                            in_file.line + 1,
                            in_file.pos + 1
                        ))
                    }
                };
            }
            if new_line {
                in_file.line += 1;
                in_file.pos = 0;
            }
        }
        if let Some(s) = buffer.get_str_range() {
            tbuf.push_str(s);
        }
        reader.consume(len);
        trace!("refilling buffer - current state: {:?}", self.state);
        self.int_get_token()
    }

    pub fn get_token(&mut self) -> Result<LexToken, String> {
        loop {
            let s = self.int_get_token()?;
            match s {
                LexToken::End => {
                    if let Some(file) = self.pop_file() {
                        trace!("Returning from file {}", file.name);
                    } else {
                        break;
                    }
                }
                LexToken::Include(name) => {
                    if let Err(e) = self.push_file(&name) {
                        return Err(format!("File {}, {}", self.get_file_pos_str().unwrap(), e));
                    }
                    trace!("Moving to include file '{}'", name);
                    return self.get_token();
                }
                LexToken::Section(sec) => {
                    if let Ok(section) = Section::from_str(&sec) {
                        if let Err(e) = self.set_section(section) {
                            return Err(e.to_string());
                        }
                    } else {
                        // We can use unwrap() here because if this fails then we should panic!
                        return Err(format!(
                            "Unknown Section {} in file {}",
                            sec,
                            self.get_file_pos_str().unwrap()
                        ));
                    }
                }
                _ => return Ok(s),
            }
        }
        Ok(LexToken::End)
    }
}

// We handle quotes and comments outside of the regular FSM.  If we are not in Quote or Comment mode then we check if tok is a quote or comment,
// and set the mode accordingly.  If we are in quote mode then we look for the matching closing quote, and if we are in comment mode we look for
// the end of the input line.
// While in quote mode all tokens are set to Letter so they are passed automatically.
// In comment mode all tokens are set to Null so they are skipped.
// For Quote mode the token for the start and end quotes are set to Null.  We also copy useful part of the input buffer into tbuf and set
// the buffer start so that the quotes are not included in the parsed output
fn handle_quotes_and_comments(
    mut quote_mode: QuoteMode,
    mut tok: LexRawToken,
    tbuf: &mut String,
    buffer: &LexBuf,
) -> (QuoteMode, LexRawToken, usize) {
    let mut start = buffer.start;
    match quote_mode {
        QuoteMode::None => {
            if tok == LexRawToken::SingleQuote || tok == LexRawToken::DoubleQuote {
                quote_mode = QuoteMode::Quoted(tok);
                tok = LexRawToken::Null;
                trace!("Switch into quote mode");
            } else if tok == LexRawToken::Hash {
                quote_mode = QuoteMode::Comment;
                tok = LexRawToken::Null;
                trace!("Switch into comment mode");
            }
            if tok == LexRawToken::Null {
                if let Some(s) = buffer.get_str_range() {
                    tbuf.push_str(s);
                }
                start = buffer.end + 1;
            }
        }
        QuoteMode::Quoted(x) => {
            if x == tok {
                quote_mode = QuoteMode::None;
                if let Some(s) = buffer.get_str_range() {
                    tbuf.push_str(s);
                }
                tok = LexRawToken::Null;
                start = buffer.end + 1;
                trace!("Switch out of quote mode");
            } else {
                // When we in quote mode, everything looks like a letter!
                tok = LexRawToken::Letter;
            }
        }
        QuoteMode::Comment => {
            if tok == LexRawToken::LineFeed {
                quote_mode = QuoteMode::None;
                trace!("Switching out of comment mode");
            }
            tok = LexRawToken::Null;
            start = buffer.end + 1;
        }
    }
    (quote_mode, tok, start)
}

// Handle include commands
// If we have completed a command (so in AfterName state) we check if the command is 'include'.  If so
// we do not emit the command, but we switch to InInclude state.
// If we have finished the InInclude state then we emit an Include Token with the name of the include file as
// the argument
fn handle_include(
    state: LexState,
    action: LexAction,
    tok: IntLexToken,
    buf: &str,
) -> (bool, LexState, IntLexToken) {
    let mut emit = true;
    let mut new_state = action.new_state;
    let mut token = tok;
    if state != new_state {
        if action.new_state == LexState::AfterName && buf.eq_ignore_ascii_case("include") {
            new_state = LexState::InInclude;
            trace!("Switch to InInclude");
            emit = false;
        } else if state == LexState::InInclude {
            token = IntLexToken::Include;
        }
        if let IntLexToken::Null = token {
            emit = false;
        }
    }
    (emit, new_state, token)
}
