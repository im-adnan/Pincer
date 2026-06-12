pub fn expand_uris(uri: &str) -> Vec<String> {
    let mut results = vec![uri.to_string()];

    // Process {} sets
    loop {
        let mut new_results = Vec::new();
        let mut changed = false;

        for s in &results {
            if let Some(start) = s.find('{') {
                if let Some(end) = s[start..].find('}') {
                    let end = start + end;
                    let prefix = &s[..start];
                    let suffix = &s[end + 1..];
                    let inside = &s[start + 1..end];

                    for part in inside.split(',') {
                        new_results.push(format!("{}{}{}", prefix, part, suffix));
                    }
                    changed = true;
                    continue;
                }
            }
            new_results.push(s.clone());
        }
        results = new_results;
        if !changed {
            break;
        }
    }

    // Process [] ranges
    loop {
        let mut new_results = Vec::new();
        let mut changed = false;

        for s in &results {
            if let Some(start) = s.find('[') {
                if let Some(end) = s[start..].find(']') {
                    let end = start + end;
                    let prefix = &s[..start];
                    let suffix = &s[end + 1..];
                    let inside = &s[start + 1..end];

                    if let Some(expansions) = expand_range(inside) {
                        for part in expansions {
                            new_results.push(format!("{}{}{}", prefix, part, suffix));
                        }
                        changed = true;
                        continue;
                    }
                }
            }
            new_results.push(s.clone());
        }
        results = new_results;
        if !changed {
            break;
        }
    }

    results
}

fn expand_range(inside: &str) -> Option<Vec<String>> {
    // format: start-end:step or start-end
    let parts: Vec<&str> = inside.split(':').collect();
    let range_str = parts[0];
    let step = if parts.len() > 1 {
        parts[1].parse::<usize>().unwrap_or(1)
    } else {
        1
    };

    if step == 0 {
        return None;
    }

    let bounds: Vec<&str> = range_str.split('-').collect();
    if bounds.len() != 2 {
        return None;
    }

    let start_str = bounds[0];
    let end_str = bounds[1];

    // Alphabetic [a-z] or [A-Z]
    if start_str.len() == 1 && end_str.len() == 1 {
        let sc = start_str.chars().next().unwrap();
        let ec = end_str.chars().next().unwrap();
        if sc.is_ascii_alphabetic() && ec.is_ascii_alphabetic() {
            let mut res = Vec::new();
            let mut curr = sc as u8;
            let end_val = ec as u8;
            if curr <= end_val {
                while curr <= end_val {
                    res.push((curr as char).to_string());
                    let (next, overflow) = curr.overflowing_add(step as u8);
                    if overflow || next < curr {
                        break;
                    }
                    curr = next;
                }
            } else {
                while curr >= end_val {
                    res.push((curr as char).to_string());
                    let (next, overflow) = curr.overflowing_sub(step as u8);
                    if overflow || next > curr {
                        break;
                    }
                    curr = next;
                }
            }
            return Some(res);
        }
    }

    // Numeric
    let start_num = start_str.parse::<i64>().ok()?;
    let end_num = end_str.parse::<i64>().ok()?;
    let pad = start_str.len();

    let mut res = Vec::new();
    let mut curr = start_num;
    if start_num <= end_num {
        while curr <= end_num {
            res.push(format!("{:0width$}", curr, width = pad));
            curr += step as i64;
        }
    } else {
        while curr >= end_num {
            res.push(format!("{:0width$}", curr, width = pad));
            curr -= step as i64;
        }
    }

    Some(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_set() {
        assert_eq!(
            expand_uris("http://{a,b}/file.txt"),
            vec!["http://a/file.txt", "http://b/file.txt"]
        );
    }

    #[test]
    fn test_expand_range() {
        assert_eq!(
            expand_uris("http://host/file[01-03].txt"),
            vec![
                "http://host/file01.txt",
                "http://host/file02.txt",
                "http://host/file03.txt"
            ]
        );
        assert_eq!(
            expand_uris("http://host/file[a-c].txt"),
            vec![
                "http://host/filea.txt",
                "http://host/fileb.txt",
                "http://host/filec.txt"
            ]
        );
        assert_eq!(
            expand_uris("http://host/file[1-5:2].txt"),
            vec![
                "http://host/file1.txt",
                "http://host/file3.txt",
                "http://host/file5.txt"
            ]
        );
    }

    #[test]
    fn test_expand_complex() {
        assert_eq!(
            expand_uris("http://{x,y}/[1-2]"),
            vec!["http://x/1", "http://x/2", "http://y/1", "http://y/2"]
        );
    }
}
