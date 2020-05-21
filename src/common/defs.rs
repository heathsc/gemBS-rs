use std::str::FromStr;

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Clone)]
#[derive(Copy)]
pub enum Section {
	Default, Index, Mapping, Calling, Extract, Report,
}

impl FromStr for Section {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Section::Default),
            "index" => Ok(Section::Index),
            "mapping" => Ok(Section::Mapping),
            "calling" => Ok(Section::Calling),
            "extract" => Ok(Section::Extract),
            "report" => Ok(Section::Report),
            _ => Err("no match"),
        }
    }
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
pub enum VarType {
	StringVar, BoolVar, IntVar, FloatVar
}

