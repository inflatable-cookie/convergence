use std::collections::BTreeMap;

use super::EntrySig;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status")]
pub enum DiffLine {
    Added {
        path: String,
        to: EntrySig,
    },
    Deleted {
        path: String,
        from: EntrySig,
    },
    Modified {
        path: String,
        from: EntrySig,
        to: EntrySig,
    },
}

pub fn diff_trees(
    from: &BTreeMap<String, EntrySig>,
    to: &BTreeMap<String, EntrySig>,
) -> Vec<DiffLine> {
    let mut out = Vec::new();

    for (path, from_sig) in from {
        match to.get(path) {
            None => out.push(DiffLine::Deleted {
                path: path.clone(),
                from: from_sig.clone(),
            }),
            Some(to_sig) => {
                if from_sig != to_sig {
                    out.push(DiffLine::Modified {
                        path: path.clone(),
                        from: from_sig.clone(),
                        to: to_sig.clone(),
                    });
                }
            }
        }
    }

    for (path, to_sig) in to {
        if !from.contains_key(path) {
            out.push(DiffLine::Added {
                path: path.clone(),
                to: to_sig.clone(),
            });
        }
    }

    out.sort_by(|a, b| line_path(a).cmp(line_path(b)));
    out
}

fn line_path(d: &DiffLine) -> &str {
    match d {
        DiffLine::Added { path, .. } => path,
        DiffLine::Deleted { path, .. } => path,
        DiffLine::Modified { path, .. } => path,
    }
}
