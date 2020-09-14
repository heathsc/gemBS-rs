// Given a &str, we want to recognize patterns like:
//
//    ${name} or ${section:name}
//
// The aim is to give the coordinates of the regular source, the section (if present)
// and the name.  We do this with a FSM as I want to avoid having to bring in a regex
// crate just for this...

enum State {
	State0,
	State1,
	State2,
	State3,
	State4,
	State5,
	State6,
}

#[derive(Debug)]
pub enum Segment {
	NameOnly([usize; 4]),
	SectionName([usize; 6]),
	End(usize),
}

pub fn find_var(name: &str, segments: &mut Vec<Segment>) {
	segments.clear();
	let mut state = State::State0;
	let mut idx = Vec::new();
	for (i, c) in name.char_indices() {
		match &state {
			// Initial state
			State::State0 => {
				idx.push(i);
				if c == '$' {
					idx.push(i);
					state = State::State2;
				} else {
					state = State::State1;
				}				
			},
			// In source state
			State::State1 => {
				if c == '$' {
					idx.push(i); // Push index of char after source segment
					state = State::State2;
				}
			},
			// After having seen a '$' char
			State::State2 => {
				if c == '{' {
					state = State::State3;
				} else {
					idx.pop();
					if c == '$' {
						// Try again with new $ char
						idx.push(i);
					} else {
						// Back to initial state
						state = State::State1;
					}
				}	
			},
			// First character after seeing "${"
			State::State3 => {
				// Push start of section / name segment 
				idx.push(i);
				if c == '}' {
					// End of section / name
					idx.push(i);
					state = State::State0;
				} else if c == ':' {
					// End of section
					idx.push(i);
					state = State::State5;
				} else {
					state = State::State4;
				}
			},
			// In section / name
			State::State4 => {
				if c == '}' {
					// end of name
					idx.push(i);
					state = State::State0;
				} else if c == ':' {
					// End of section
					idx.push(i);
					state = State::State5;
				} 
			},
			// First character after ':'
			State::State5 => {
				// Push start of name segment
 				idx.push(i);
				if c == '}' {
					// End of name
					idx.push(i);
					state = State::State0;
				} else {
					state = State::State6;
				}
			},
			// In name (after section)
			State::State6 => {
				if c == '}' {
					// end of name
					idx.push(i);
					state = State::State0;
				}
			},
		}
		if let State::State0 = state {
			if idx.len() == 4 {
				// No section
				segments.push(Segment::NameOnly([idx[0], idx[1], idx[2], idx[3]]));
			} else if idx.len() == 6 {
				// Section and name
				segments.push(Segment::SectionName([idx[0], idx[1], idx[2], idx[3], idx[4], idx[5]]));
			} else {
				debug!("Internal error in find_var() - unexpected length");
			}
			idx.clear();
		}
	}
	if !idx.is_empty() {
		// End
		segments.push(Segment::End(idx[0]));
	}
}
