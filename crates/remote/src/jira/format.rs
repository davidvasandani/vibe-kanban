//! Conversion between Jira REST API v2 wiki markup and Vibe Kanban Markdown.
//!
//! This intentionally supports the formatting shared by Jira issue
//! descriptions and VK's editor. Unknown Jira macros are left untouched so a
//! partial converter never silently discards user content.

/// Convert a Jira REST API v2 wiki-markup description to Markdown.
pub(crate) fn jira_to_markdown(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut output = Vec::with_capacity(lines.len());
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if let Some((language, closing)) = jira_code_block_start(line) {
            output.push(match language {
                Some(language) => format!("```{language}"),
                None => "```".to_string(),
            });
            index += 1;
            while index < lines.len() && lines[index].trim() != closing {
                output.push(lines[index].to_string());
                index += 1;
            }
            output.push("```".to_string());
            if index < lines.len() {
                index += 1;
            }
            continue;
        }

        if line.starts_with("||") && line.ends_with("||") {
            let headers = split_jira_row(line, true);
            if !headers.is_empty() {
                output.push(markdown_table_row(&headers));
                output.push(markdown_table_row(
                    &headers
                        .iter()
                        .map(|_| "---".to_string())
                        .collect::<Vec<_>>(),
                ));
                index += 1;
                while index < lines.len()
                    && lines[index].starts_with('|')
                    && lines[index].ends_with('|')
                    && !lines[index].starts_with("||")
                {
                    let cells = split_jira_row(lines[index], false);
                    output.push(markdown_table_row(&cells));
                    index += 1;
                }
                continue;
            }
        }

        output.push(convert_jira_line(line));
        index += 1;
    }

    let mut result = output.join("\n");
    if input.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Convert a Vibe Kanban Markdown description to Jira REST API v2 wiki markup.
pub(crate) fn markdown_to_jira(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut output = Vec::with_capacity(lines.len());
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if let Some(language) = markdown_fence_start(line) {
            let opening = language
                .filter(|value| !value.is_empty())
                .map_or_else(|| "{code}".to_string(), |value| format!("{{code:{value}}}"));
            output.push(opening);
            index += 1;
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                output.push(lines[index].to_string());
                index += 1;
            }
            output.push("{code}".to_string());
            if index < lines.len() {
                index += 1;
            }
            continue;
        }

        if index + 1 < lines.len() {
            if let (Some(headers), Some(delimiters)) = (
                split_markdown_row(line),
                split_markdown_row(lines[index + 1]),
            ) {
                if headers.len() == delimiters.len()
                    && !headers.is_empty()
                    && delimiters.iter().all(|cell| is_table_delimiter(cell))
                {
                    output.push(jira_table_row(&headers, true));
                    index += 2;
                    while index < lines.len() {
                        let Some(cells) = split_markdown_row(lines[index]) else {
                            break;
                        };
                        output.push(jira_table_row(&cells, false));
                        index += 1;
                    }
                    continue;
                }
            }
        }

        output.push(convert_markdown_line(line));
        index += 1;
    }

    let mut result = output.join("\n");
    if input.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn jira_code_block_start(line: &str) -> Option<(Option<&str>, &'static str)> {
    let trimmed = line.trim();
    if trimmed == "{noformat}" {
        return Some((None, "{noformat}"));
    }
    if trimmed == "{code}" {
        return Some((None, "{code}"));
    }
    trimmed
        .strip_prefix("{code:")
        .and_then(|rest| rest.strip_suffix('}'))
        .map(|language| (Some(language), "{code}"))
}

fn markdown_fence_start(line: &str) -> Option<Option<&str>> {
    line.trim_start()
        .strip_prefix("```")
        .map(|language| Some(language.trim()))
}

fn convert_jira_line(line: &str) -> String {
    for level in 1..=6 {
        let prefix = format!("h{level}. ");
        if let Some(content) = line.strip_prefix(&prefix) {
            return format!("{} {}", "#".repeat(level), jira_inline_to_markdown(content));
        }
    }

    let marker_count = line.chars().take_while(|c| *c == '*' || *c == '#').count();
    if marker_count > 0 && line.as_bytes().get(marker_count) == Some(&b' ') {
        let markers = &line[..marker_count];
        if markers.chars().all(|c| c == '*') || markers.chars().all(|c| c == '#') {
            let marker = if markers.starts_with('*') { "-" } else { "1." };
            return format!(
                "{}{} {}",
                "  ".repeat(marker_count - 1),
                marker,
                jira_inline_to_markdown(&line[marker_count + 1..])
            );
        }
    }

    jira_inline_to_markdown(line)
}

fn convert_markdown_line(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    if let Some((level, content)) = markdown_heading(trimmed) {
        return format!("h{level}. {}", markdown_inline_to_jira(content));
    }

    if let Some(content) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return format!(
            "{} {}",
            "*".repeat(indent / 2 + 1),
            markdown_inline_to_jira(content)
        );
    }
    if let Some(content) = strip_ordered_list_marker(trimmed) {
        return format!(
            "{} {}",
            "#".repeat(indent / 2 + 1),
            markdown_inline_to_jira(content)
        );
    }

    markdown_inline_to_jira(line)
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&level).then(|| {
        line.get(level..)?
            .strip_prefix(' ')
            .map(|content| (level, content))
    })?
}

fn strip_ordered_list_marker(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    line.get(digits..)?.strip_prefix(". ")
}

fn jira_inline_to_markdown(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        let rest = &input[index..];
        if let Some(escaped) = rest.strip_prefix('\\') {
            if let Some(ch) = escaped.chars().next() {
                output.push('\\');
                output.push(ch);
                index += 1 + ch.len_utf8();
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix("{{") {
            if let Some(end) = after.find("}}") {
                output.push_str(&markdown_code_span(&after[..end]));
                index += 2 + end + 2;
                continue;
            }
        }
        if rest.starts_with('[') {
            if let Some(end) = find_unescaped(rest, ']', 1) {
                let content = &rest[1..end];
                let (label, url) = content.split_once('|').unwrap_or((content, content));
                if looks_like_url(url) {
                    output.push('[');
                    output.push_str(label);
                    output.push_str("](");
                    output.push_str(url);
                    output.push(')');
                    index += end + 1;
                    continue;
                }
            }
        }
        if let Some((replacement, consumed)) = jira_emphasis(rest) {
            output.push_str(&replacement);
            index += consumed;
            continue;
        }
        let ch = rest.chars().next().expect("non-empty remainder");
        output.push(ch);
        index += ch.len_utf8();
    }

    output
}

fn markdown_inline_to_jira(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with('`') {
            let ticks = rest.chars().take_while(|c| *c == '`').count();
            let delimiter = "`".repeat(ticks);
            if let Some(end) = rest[ticks..].find(&delimiter) {
                let content = &rest[ticks..ticks + end];
                output.push_str("{{");
                output.push_str(content.trim_matches(' '));
                output.push_str("}}");
                index += ticks + end + ticks;
                continue;
            }
        }
        if rest.starts_with('[') {
            if let Some(label_end) = find_unescaped(rest, ']', 1) {
                let after_label = &rest[label_end + 1..];
                if let Some(url_part) = after_label.strip_prefix('(') {
                    if let Some(url_end) = find_unescaped(url_part, ')', 0) {
                        output.push('[');
                        output.push_str(&rest[1..label_end]);
                        output.push('|');
                        output.push_str(&url_part[..url_end]);
                        output.push(']');
                        index += label_end + 2 + url_end + 1;
                        continue;
                    }
                }
            }
        }
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**").filter(|end| *end > 0) {
                output.push('*');
                output.push_str(&markdown_inline_to_jira(&after[..end]));
                output.push('*');
                index += 2 + end + 2;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('*') {
            if let Some(end) = after.find('*').filter(|end| *end > 0) {
                let content = &after[..end];
                if !content.starts_with(char::is_whitespace)
                    && !content.ends_with(char::is_whitespace)
                {
                    output.push('_');
                    output.push_str(&markdown_inline_to_jira(content));
                    output.push('_');
                    index += 1 + end + 1;
                    continue;
                }
            }
        }
        let ch = rest.chars().next().expect("non-empty remainder");
        output.push(ch);
        index += ch.len_utf8();
    }

    output
}

fn jira_emphasis(rest: &str) -> Option<(String, usize)> {
    let delimiter = rest.chars().next()?;
    let wrapper = match delimiter {
        '*' => "**",
        '_' => "*",
        _ => return None,
    };
    let after = &rest[delimiter.len_utf8()..];
    let end = find_unescaped(after, delimiter, 0)?;
    let content = &after[..end];
    if content.is_empty()
        || content.starts_with(char::is_whitespace)
        || content.ends_with(char::is_whitespace)
    {
        return None;
    }
    Some((
        format!("{wrapper}{}{wrapper}", jira_inline_to_markdown(content)),
        delimiter.len_utf8() + end + delimiter.len_utf8(),
    ))
}

fn find_unescaped(input: &str, target: char, start: usize) -> Option<usize> {
    let mut escaped = false;
    for (offset, ch) in input.char_indices().filter(|(offset, _)| *offset >= start) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == target {
            return Some(offset);
        }
    }
    None
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("mailto:")
}

fn markdown_code_span(content: &str) -> String {
    if content.contains('`') {
        format!("`` {content} ``")
    } else {
        format!("`{content}`")
    }
}

fn split_jira_row(line: &str, header: bool) -> Vec<String> {
    let delimiter = if header { "||" } else { "|" };
    let Some(inner) = line
        .strip_prefix(delimiter)
        .and_then(|value| value.strip_suffix(delimiter))
    else {
        return Vec::new();
    };
    split_unescaped_jira_cells(inner, header)
        .into_iter()
        .map(|cell| jira_inline_to_markdown(cell.trim()))
        .collect()
}

fn split_unescaped_jira_cells(inner: &str, header: bool) -> Vec<String> {
    let chars: Vec<char> = inner.chars().collect();
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '\\' && chars.get(index + 1) == Some(&'|') {
            cell.push('|');
            index += 2;
        } else if chars[index] == '|' && (!header || chars.get(index + 1) == Some(&'|')) {
            cells.push(cell);
            cell = String::new();
            index += if header { 2 } else { 1 };
        } else {
            cell.push(chars[index]);
            index += 1;
        }
    }
    cells.push(cell);
    cells
}

fn markdown_table_row(cells: &[String]) -> String {
    format!(
        "| {} |",
        cells
            .iter()
            .map(|cell| cell.replace('|', "\\|"))
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

fn split_markdown_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = trimmed[1..trimmed.len() - 1].chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            cell.push('|');
            chars.next();
        } else if ch == '|' {
            cells.push(cell.trim().to_string());
            cell.clear();
        } else {
            cell.push(ch);
        }
    }
    cells.push(cell.trim().to_string());
    Some(cells)
}

fn is_table_delimiter(cell: &str) -> bool {
    let trimmed = cell.trim().trim_matches(':');
    trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '-')
}

fn jira_table_row(cells: &[String], header: bool) -> String {
    let delimiter = if header { "||" } else { "|" };
    let converted = cells
        .iter()
        .map(|cell| markdown_inline_to_jira(cell).replace('|', "\\|"))
        .collect::<Vec<_>>()
        .join(delimiter);
    format!("{delimiter}{converted}{delimiter}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_reported_jira_formatting_to_markdown() {
        let jira = "h3. Target\nImplementation for [CSEQ-4020|https://example.test/CSEQ-4020] with *bold*, _italic_, and {{User.Read}}.\n||Field||Value||\n|Tenant|sweetgreen|";
        let markdown = "### Target\nImplementation for [CSEQ-4020](https://example.test/CSEQ-4020) with **bold**, *italic*, and `User.Read`.\n| Field | Value |\n| --- | --- |\n| Tenant | sweetgreen |";

        assert_eq!(jira_to_markdown(jira), markdown);
    }

    #[test]
    fn converts_markdown_blocks_to_jira() {
        let markdown = "## Steps\n- first\n  - nested\n1. one\n\n| Field | Value |\n| :--- | ---: |\n| Scope | `User.Read` |\n\n```powershell\nGet-MgUser\n```";
        let jira = "h2. Steps\n* first\n** nested\n# one\n\n||Field||Value||\n|Scope|{{User.Read}}|\n\n{code:powershell}\nGet-MgUser\n{code}";

        assert_eq!(markdown_to_jira(markdown), jira);
    }

    #[test]
    fn preserves_plain_malformed_and_unknown_content() {
        let input =
            "Plain a * b, C:\\Temp\\file, and [not a link].\n{panel:title=Keep me}\nbody\n{panel}";
        assert_eq!(jira_to_markdown(input), input);
    }

    #[test]
    fn table_conversion_preserves_literal_backslashes() {
        let markdown =
            "| Kind | Value |\n| --- | --- |\n| Path | C:\\Temp\\file |\n| Pipe | a\\|b |";
        let jira = "||Kind||Value||\n|Path|C:\\Temp\\file|\n|Pipe|a\\|b|";

        assert_eq!(markdown_to_jira(markdown), jira);
        assert_eq!(jira_to_markdown(jira), markdown);
    }

    #[test]
    fn preserves_trailing_newline_and_unclosed_code_block_content() {
        assert_eq!(jira_to_markdown("plain\n"), "plain\n");
        assert_eq!(jira_to_markdown("{code}\n*x*"), "```\n*x*\n```");
    }

    #[test]
    fn supported_inline_formatting_is_stable_across_formats() {
        let markdown = "A **bold** and *italic* [link](https://example.test) with `code`.";
        assert_eq!(jira_to_markdown(&markdown_to_jira(markdown)), markdown);
    }
}
