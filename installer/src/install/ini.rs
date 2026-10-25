/// What `replace_key` found: the key replaced, or where it has to go.
enum Search {
    Replaced,
    /// The section ends before the line at this index.
    SectionEndsAt(usize),
    SectionRunsToEnd,
    NoSection,
}

/// Sets one key in one section and leaves every other byte as it was, since
/// the file is the package author's and parsing it into a map would rewrite it.
///
/// Comment lines never match, the spacing around `=` and any inline comment are
/// kept, line endings stay the file's, and a missing key joins its own section
/// rather than the end of the file.
pub fn set_ini_key(text: &str, section: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let ends_with_newline = text.ends_with('\n') || text.is_empty();

    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect();
    // The empty piece after a final newline is put back when joining.
    if ends_with_newline && lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    let entry = format!("{key}={value}");
    match replace_key(&mut lines, section, key, value) {
        Search::Replaced => {}
        Search::SectionEndsAt(at) => lines.insert(at, entry),
        Search::SectionRunsToEnd => {
            let after_last_content = lines
                .iter()
                .rposition(|line| !line.trim().is_empty())
                .map_or(lines.len(), |at| at + 1);
            lines.insert(after_last_content, entry);
        }
        Search::NoSection => {
            if lines.last().is_some_and(|line| !line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{section}]"));
            lines.push(entry);
        }
    }

    let mut out = lines.join(newline);
    if ends_with_newline && !out.is_empty() {
        out.push_str(newline);
    }
    out
}

fn replace_key(lines: &mut [String], section: &str, key: &str, value: &str) -> Search {
    let mut in_section = false;
    let mut section_seen = false;
    let mut section_end = None;
    for (index, line) in lines.iter_mut().enumerate() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            if in_section {
                section_end = Some(index);
            }
            in_section = name.eq_ignore_ascii_case(section);
            section_seen |= in_section;
            continue;
        }
        if !in_section || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if left.trim().eq_ignore_ascii_case(key) {
            let indent: String = right.chars().take_while(|c| c.is_whitespace()).collect();
            let tail = inline_comment(right).unwrap_or("");
            *line = format!("{left}={indent}{value}{tail}");
            return Search::Replaced;
        }
    }
    match (section_seen, section_end) {
        (true, Some(at)) => Search::SectionEndsAt(at),
        (true, None) => Search::SectionRunsToEnd,
        (false, _) => Search::NoSection,
    }
}

/// The whitespace-led `;` or `#` comment at the end of a value, with its
/// alignment; a `;` inside a value, as in `a;b;c`, is not one.
fn inline_comment(value: &str) -> Option<&str> {
    value.char_indices().find_map(|(at, c)| {
        let before = &value[..at];
        let starts_comment = (c == ';' || c == '#') && before.ends_with(char::is_whitespace);
        starts_comment.then(|| &value[before.trim_end().len()..])
    })
}

#[cfg(test)]
mod tests;
