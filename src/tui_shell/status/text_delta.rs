fn myers_edit_distance_lines(a: &[String], b: &[String]) -> usize {
    let n = a.len();
    let m = b.len();
    let max = n + m;
    let offset = max as isize;
    let mut v = vec![0isize; 2 * max + 1];

    for d in 0..=max {
        let d_isize = d as isize;
        let mut k = -d_isize;
        while k <= d_isize {
            let idx = (k + offset) as usize;
            let x = if k == -d_isize || (k != d_isize && v[idx - 1] < v[idx + 1]) {
                v[idx + 1]
            } else {
                v[idx - 1] + 1
            };

            let mut x2 = x;
            let mut y2 = x2 - k;
            while (x2 as usize) < n && (y2 as usize) < m && a[x2 as usize] == b[y2 as usize] {
                x2 += 1;
                y2 += 1;
            }
            v[idx] = x2;
            if (x2 as usize) >= n && (y2 as usize) >= m {
                return d;
            }

            k += 2;
        }
    }

    max
}

pub(super) fn line_delta_utf8(old_bytes: &[u8], new_bytes: &[u8]) -> Option<(usize, usize)> {
    const MAX_TEXT_BYTES: usize = 256 * 1024;
    if old_bytes.len().max(new_bytes.len()) > MAX_TEXT_BYTES {
        return None;
    }

    let old_s = std::str::from_utf8(old_bytes).ok()?;
    let new_s = std::str::from_utf8(new_bytes).ok()?;
    let old_lines: Vec<String> = old_s.lines().map(|l| l.to_string()).collect();
    let new_lines: Vec<String> = new_s.lines().map(|l| l.to_string()).collect();

    let d = myers_edit_distance_lines(&old_lines, &new_lines);
    let lcs = (old_lines.len() + new_lines.len()).saturating_sub(d) / 2;
    let added = new_lines.len().saturating_sub(lcs);
    let deleted = old_lines.len().saturating_sub(lcs);
    Some((added, deleted))
}

pub(super) fn count_lines_utf8(bytes: &[u8]) -> Option<usize> {
    const MAX_TEXT_BYTES: usize = 256 * 1024;
    if bytes.len() > MAX_TEXT_BYTES {
        return None;
    }
    let s = std::str::from_utf8(bytes).ok()?;
    Some(s.lines().count())
}

pub(super) fn fmt_line_delta(added: usize, deleted: usize) -> String {
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("+{}", added));
    }
    if deleted > 0 {
        parts.push(format!("-{}", deleted));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(" "))
    }
}
