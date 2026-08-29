//! Parameterized URI expansion ({a,b}, [0-9])
//!
//! ### Architectural Overview
//! - **What it does**: Expands parameterized URI expressions containing enumerated sets (e.g. `{http,ftp}`) or numeric/alphabetic ranges (e.g. `[01-10]`, `[a-z]`) into full individual URL lists.
//! - **How it does**: Scans bracket pairs recursively, splits comma-delimited set lists, computes integer/character ranges preserving leading zero padding, and generates the Cartesian expansion.
//! - **Where it comes from**: Called by `rpc::handlers::add_uri` when adding batch download requests containing bracket patterns.
//! - **Where it leads to**: Returns a `Vec<String>` of expanded individual URLs handed off to `manager.spawn_task()`.

/// Expands a single URI pattern containing set `{a,b,c}` or range `[0-9]`, `[a-z]` bracket notation.
///
/// Example expansions:
/// - `"http://site.com/image_{1,2}.jpg"` -> `["http://site.com/image_1.jpg", "http://site.com/image_2.jpg"]`
/// - `"http://site.com/part_[01-03].bin"` -> `["http://site.com/part_01.bin", "http://site.com/part_02.bin", "http://site.com/part_03.bin"]`
pub fn expand_uris(uri: &str) -> Vec<String> {
    let mut results = vec![uri.to_string()];

    // Stage 1: Process curly brace sets {a,b,c}
    let mut changed = true;
    while changed {
        changed = false;
        let mut next_results = Vec::new();

        for s in results {
            if let Some((start, end)) = find_bracket_pair(&s, '{', '}') {
                changed = true;
                let prefix = &s[..start];
                let inner = &s[start + 1..end];
                let suffix = &s[end + 1..];

                // Expand each comma-separated option
                for part in inner.split(',') {
                    next_results.push(format!("{}{}{}", prefix, part.trim(), suffix));
                }
            } else {
                next_results.push(s);
            }
        }
        results = next_results;
    }

    // Stage 2: Process square bracket ranges [start-end]
    changed = true;
    while changed {
        changed = false;
        let mut next_results = Vec::new();

        for s in results {
            if let Some((start, end)) = find_bracket_pair(&s, '[', ']') {
                let prefix = &s[..start];
                let inner = &s[start + 1..end];
                let suffix = &s[end + 1..];

                if let Some(expanded_parts) = expand_range(inner) {
                    changed = true;
                    for part in expanded_parts {
                        next_results.push(format!("{}{}{}", prefix, part, suffix));
                    }
                } else {
                    next_results.push(s);
                }
            } else {
                next_results.push(s);
            }
        }
        results = next_results;
    }

    results
}

/// Locates the first matching pair of open and close bracket delimiters in a string.
fn find_bracket_pair(s: &str, open: char, close: char) -> Option<(usize, usize)> {
    let start = s.find(open)?;
    let end = s[start..].find(close).map(|idx| start + idx)?;
    if start < end {
        Some((start, end))
    } else {
        None
    }
}

/// Expands a range string (e.g. "01-10", "1-5", "a-z", "A-Z") into a list of strings.
fn expand_range(range_str: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = range_str.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start_str = parts[0].trim();
    let end_str = parts[1].trim();

    // 1. Check if numeric range (e.g. 01-10 or 1-5)
    if let (Ok(start_num), Ok(end_num)) = (start_str.parse::<u64>(), end_str.parse::<u64>()) {
        // Detect leading zero width for zero-padded formatting (e.g. "01" -> width 2)
        let width = if start_str.starts_with('0') && start_str.len() > 1 {
            start_str.len()
        } else {
            0
        };

        let mut items = Vec::new();
        if start_num <= end_num {
            for n in start_num..=end_num {
                if width > 0 {
                    items.push(format!("{:0width$}", n, width = width));
                } else {
                    items.push(format!("{}", n));
                }
            }
        } else {
            for n in (end_num..=start_num).rev() {
                if width > 0 {
                    items.push(format!("{:0width$}", n, width = width));
                } else {
                    items.push(format!("{}", n));
                }
            }
        }
        return Some(items);
    }

    // 2. Check if alphabetic range (e.g. a-z or A-Z)
    if start_str.len() == 1 && end_str.len() == 1 {
        let start_char = start_str.chars().next()?;
        let end_char = end_str.chars().next()?;

        if (start_char.is_ascii_lowercase() && end_char.is_ascii_lowercase())
            || (start_char.is_ascii_uppercase() && end_char.is_ascii_uppercase())
        {
            let mut items = Vec::new();
            if start_char <= end_char {
                for c in (start_char as u8)..=(end_char as u8) {
                    items.push((c as char).to_string());
                }
            } else {
                for c in ((end_char as u8)..=(start_char as u8)).rev() {
                    items.push((c as char).to_string());
                }
            }
            return Some(items);
        }
    }

    None
}
