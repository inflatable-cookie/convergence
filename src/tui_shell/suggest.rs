use super::CommandDef;

pub(super) fn score_match(q: &str, candidate: &str) -> i32 {
    let q = q.to_lowercase();
    let c = candidate.to_lowercase();
    if c == q {
        return 100;
    }
    if c.starts_with(&q) {
        return 50 - (c.len() as i32 - q.len() as i32);
    }
    if c.contains(&q) {
        return 10;
    }
    0
}

pub(super) fn sort_scored_suggestions(scored: &mut [(i32, CommandDef)], hint_order: &[String]) {
    let mut hint_pos = std::collections::HashMap::<String, usize>::new();
    for (i, h) in hint_order.iter().enumerate() {
        hint_pos.insert(h.clone(), i);
    }

    scored.sort_by(|(sa, a), (sb, b)| {
        let ha = hint_pos
            .get(a.name)
            .copied()
            .or_else(|| a.aliases.iter().find_map(|al| hint_pos.get(*al).copied()));
        let hb = hint_pos
            .get(b.name)
            .copied()
            .or_else(|| b.aliases.iter().find_map(|al| hint_pos.get(*al).copied()));

        match (ha, hb) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => sb.cmp(sa).then_with(|| a.name.cmp(b.name)),
        }
    });
}

#[cfg(test)]
#[path = "../tests/tui_shell/suggest_tests.rs"]
mod tests;
