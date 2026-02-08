use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

fn is_glob_query(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

fn normalize_glob_query(q: &str) -> String {
    let q = q.trim().trim_start_matches("./");
    if q.is_empty() {
        return q.to_string();
    }
    if is_glob_query(q) {
        q.to_string()
    } else {
        format!("**/*{}*", q)
    }
}

fn collect_paths(root: &Path) -> Result<Vec<String>> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
            let entry = entry.context("read dir entry")?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.as_ref() == ".converge" || name.as_ref() == ".git" {
                continue;
            }
            if name.as_ref().starts_with(".converge_tmp_") {
                continue;
            }

            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);

            let ft = entry.file_type().context("read file type")?;
            if ft.is_dir() {
                walk(root, &path, out)?;
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

pub(super) fn glob_search(root: &Path, query: &str) -> Result<Vec<String>> {
    let query = normalize_glob_query(query);
    let matcher = globset::Glob::new(&query)
        .with_context(|| format!("invalid glob: {}", query))?
        .compile_matcher();

    let all = collect_paths(root)?;
    let mut out = Vec::new();
    for p in all {
        if matcher.is_match(&p) {
            out.push(p);
        }
    }
    Ok(out)
}
