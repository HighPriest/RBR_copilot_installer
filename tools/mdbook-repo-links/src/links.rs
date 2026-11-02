//! Finding the link targets in Markdown, and replacing the ones a resolver
//! has an answer for. Code is left alone: fenced blocks and inline code
//! spans show links as text, and rewriting them would change what they show.

/// `content` with every link target `resolve` answers for replaced by its
/// answer: inline links and images, `[text](target)`, and reference
/// definitions, `[label]: target`.
pub fn rewrite_links(content: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(content.len());
    let mut fence: Option<&str> = None;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(open) = fence {
            if trimmed.starts_with(open) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some(marker) = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m)) {
            fence = Some(marker);
            out.push_str(line);
            continue;
        }
        out.push_str(&rewrite_line(line, resolve));
    }
    out
}

fn rewrite_line(line: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    if let Some(rewritten) = rewrite_reference_definition(line, resolve) {
        return rewritten;
    }
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    let mut rest = line;
    while let Some(at) = rest.find(['`', ']']) {
        let (before, from) = rest.split_at(at);
        out.push_str(before);
        if let Some(after_tick) = from.strip_prefix('`') {
            in_code = !in_code;
            out.push('`');
            rest = after_tick;
            continue;
        }
        let after = match from.strip_prefix("](") {
            Some(after) if !in_code => after,
            _ => {
                out.push(']');
                rest = &from[1..];
                continue;
            }
        };
        out.push_str("](");
        let length = target_length(after);
        let target = &after[..length];
        match resolve(target.trim_matches(['<', '>'])) {
            Some(replacement) => out.push_str(&replacement),
            None => out.push_str(target),
        }
        rest = &after[length..];
    }
    out.push_str(rest);
    out
}

/// How much of `text`, which follows `](`, is the target: up to the space
/// before a title or the closing parenthesis, or all of an `<…>` target.
fn target_length(text: &str) -> usize {
    if text.starts_with('<') {
        return text.find('>').map_or(text.len(), |end| end + 1);
    }
    text.find([')', ' ', '\t', '\n', '\r'])
        .unwrap_or(text.len())
}

/// `[label]: target` at the start of a line, the target rewritten.
fn rewrite_reference_definition(
    line: &str,
    resolve: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let indent = line.len() - line.trim_start().len();
    let body = &line[indent..];
    let label_end = body.strip_prefix('[')?.find("]:")? + 1;
    let after_colon = &body[label_end + 2..];
    let spaces = after_colon.len() - after_colon.trim_start().len();
    let rest = &after_colon[spaces..];
    let length = target_length(rest);
    let replacement = resolve(rest[..length].trim_matches(['<', '>']))?;
    Some(format!(
        "{}{}{replacement}{}",
        &line[..indent + label_end + 2],
        &after_colon[..spaces],
        &rest[length..]
    ))
}

#[cfg(test)]
mod tests {
    fn to_github(target: &str) -> Option<String> {
        target
            .ends_with(".rs")
            .then(|| format!("https://example.test/{target}"))
    }

    fn rewrite(content: &str) -> String {
        super::rewrite_links(content, &to_github)
    }

    #[test]
    fn a_link_the_resolver_answers_for_is_replaced() {
        assert_eq!(
            rewrite("see [the zip reader](src/zip.rs) and [design](DESIGN.md)"),
            "see [the zip reader](https://example.test/src/zip.rs) and [design](DESIGN.md)"
        );
    }

    #[test]
    fn a_title_and_an_image_keep_their_shape() {
        assert_eq!(
            rewrite("![x](a.rs \"title\") [y](<b.rs>)"),
            "![x](https://example.test/a.rs \"title\") [y](https://example.test/b.rs)"
        );
    }

    #[test]
    fn code_shows_links_as_text_and_is_left_alone() {
        let fenced = "```md\n[x](a.rs)\n```\n[x](a.rs)\n";
        assert_eq!(
            rewrite(fenced),
            "```md\n[x](a.rs)\n```\n[x](https://example.test/a.rs)\n"
        );
        assert_eq!(
            rewrite("`[x](a.rs)` [x](a.rs)"),
            "`[x](a.rs)` [x](https://example.test/a.rs)"
        );
    }

    #[test]
    fn a_reference_definition_is_rewritten() {
        assert_eq!(
            rewrite("[zip]: src/zip.rs \"the reader\"\n"),
            "[zip]: https://example.test/src/zip.rs \"the reader\"\n"
        );
    }

    #[test]
    fn brackets_that_are_not_links_are_untouched() {
        let text = "a [box] then (paren) and ]( alone";
        assert_eq!(rewrite(text), text);
    }
}
