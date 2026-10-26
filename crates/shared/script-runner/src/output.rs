//! Reading a line of a command's output: escape codes out, links and the
//! SPFx debug query string found.

/// Strips ANSI escape sequences — SGR/CSI color and cursor codes
/// (`ESC [ ... <final byte>`) and OSC sequences such as hyperlinks
/// (`ESC ] ... (BEL | ESC \)`) — from a line of subprocess output.
///
/// Tools like Vite assume they're printing to a real terminal and colorize
/// their output accordingly; the Script Runner's output is plain text, not a
/// terminal emulator, so left unstripped those escapes show up as literal
/// noise (`[32m[1mVITE[22m`) instead of being interpreted.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                loop {
                    match chars.next() {
                        None | Some('\u{7}') => break,
                        Some('\u{1b}') => {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        Some(_) => {}
                    }
                }
            }
            // An unrecognized escape kind: drop just the ESC byte itself
            // rather than guessing how far to skip.
            _ => {}
        }
    }
    out
}

/// Detect an SPFx workbench debug query-string in a line of output:
/// `?debug=true&...` printed by `gulp serve` / `heft start`.
pub fn debug_querystring_in(line: &str) -> Option<String> {
    let start = line.find("?debug")?;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// The end of the URL starting at byte offset `start` in `line`: scans to
/// the next whitespace/quote/angle-bracket, then trims trailing sentence
/// punctuation, so `https://host/,` becomes `https://host/`.
fn url_end(line: &str, start: usize) -> usize {
    let candidate = &line[start..];
    let raw_end = candidate
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
        .unwrap_or(candidate.len());
    let trimmed = candidate[..raw_end].trim_end_matches([',', '.', ';', ':', '!', '?', ')', ']']);
    start + trimmed.len()
}

/// Byte ranges of every `http(s)://` URL in `line`, leftmost first.
pub fn url_ranges(line: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut pos = 0;
    while pos < line.len() {
        let rest = &line[pos..];
        let rel_start = match (rest.find("http://"), rest.find("https://")) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => break,
        };
        let start = pos + rel_start;
        let end = url_end(line, start);
        if end > start {
            ranges.push((start, end));
        }
        pos = end.max(start + 1);
    }
    ranges
}

/// Every `http(s)://` URL in `line` (already ANSI-stripped), in order.
pub fn find_urls(line: &str) -> Vec<String> {
    url_ranges(line)
        .into_iter()
        .map(|(start, end)| line[start..end].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_querystring_is_cut_at_whitespace_and_quotes() {
        let line = "To debug, append ?debug=true&noredir=true&debugManifestsFile=https://localhost:4321/temp/manifests.js to the URL";
        assert_eq!(
            debug_querystring_in(line).as_deref(),
            Some(
                "?debug=true&noredir=true&debugManifestsFile=https://localhost:4321/temp/manifests.js"
            )
        );
        assert_eq!(
            debug_querystring_in("href=\"/workbench.aspx?debug=true&x=1\" rel").as_deref(),
            Some("?debug=true&x=1")
        );
        assert_eq!(
            debug_querystring_in("ends with ?debug=true").as_deref(),
            Some("?debug=true")
        );
    }

    #[test]
    fn debug_querystring_absent() {
        assert_eq!(debug_querystring_in("Server started on port 4321"), None);
        assert_eq!(debug_querystring_in(""), None);
        assert_eq!(debug_querystring_in("https://example.test/?page=2"), None);
    }

    #[test]
    fn strip_ansi_removes_color_codes() {
        assert_eq!(
            strip_ansi("\u{1b}[32m\u{1b}[1mVITE\u{1b}[22m v5.0.0\u{1b}[39m"),
            "VITE v5.0.0"
        );
        assert_eq!(strip_ansi("\u{1b}[2K\u{1b}[1Gprogress 50%"), "progress 50%");
        assert_eq!(strip_ansi("plain text"), "plain text");
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn strip_ansi_removes_osc_sequences() {
        // Hyperlink terminated by BEL.
        assert_eq!(
            strip_ansi("\u{1b}]8;;https://example.test\u{7}link\u{1b}]8;;\u{7} done"),
            "link done"
        );
        // Window title terminated by ST (ESC \).
        assert_eq!(strip_ansi("\u{1b}]0;my title\u{1b}\\ready"), "ready");
    }

    #[test]
    fn strip_ansi_tolerates_malformed_sequences() {
        // An unknown escape kind drops only the ESC byte.
        assert_eq!(strip_ansi("a\u{1b}Zb"), "aZb");
        // Unterminated sequences run to the end of the line without panicking.
        assert_eq!(strip_ansi("x\u{1b}[31"), "x");
        assert_eq!(strip_ansi("y\u{1b}]0;never closed"), "y");
        assert_eq!(strip_ansi("z\u{1b}"), "z");
    }

    #[test]
    fn strip_ansi_keeps_non_ascii_text() {
        assert_eq!(
            strip_ansi("\u{1b}[32m✓\u{1b}[0m héllo → done"),
            "✓ héllo → done"
        );
    }

    #[test]
    fn urls_are_found_and_trailing_punctuation_trimmed() {
        assert_eq!(
            find_urls("  Local: http://localhost:5173/, Network: https://10.0.0.2:5173/."),
            vec!["http://localhost:5173/", "https://10.0.0.2:5173/"]
        );
        assert_eq!(
            find_urls("see https://example.test/docs!"),
            vec!["https://example.test/docs"]
        );
        assert_eq!(
            find_urls("<a href=\"https://example.test/x?y=1\">x</a>"),
            vec!["https://example.test/x?y=1"]
        );
    }

    #[test]
    fn url_ranges_are_byte_offsets_into_the_line() {
        assert_eq!(url_ranges("go https://a.io now"), vec![(3, 15)]);

        // Offsets stay valid slice boundaries after multi-byte characters.
        let line = "→ ready at https://a.io/é and http://b.io";
        let urls: Vec<&str> = url_ranges(line)
            .into_iter()
            .map(|(s, e)| &line[s..e])
            .collect();
        assert_eq!(urls, vec!["https://a.io/é", "http://b.io"]);
    }

    #[test]
    fn lines_without_urls() {
        assert!(find_urls("no links here").is_empty());
        assert!(find_urls("").is_empty());
        assert!(find_urls("ftp://example.test/file").is_empty());
        assert!(url_ranges("httpx://nope").is_empty());
    }
}
