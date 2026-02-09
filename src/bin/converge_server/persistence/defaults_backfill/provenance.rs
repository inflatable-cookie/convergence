use super::*;

pub(crate) fn backfill_provenance_user_ids(
    repo: &mut Repo,
    handle_to_id: &HashMap<String, String>,
) {
    for p in &mut repo.publications {
        if p.publisher_user_id.is_none() {
            p.publisher_user_id = handle_to_id.get(&p.publisher).cloned();
        }
    }
    for b in &mut repo.bundles {
        if b.created_by_user_id.is_none() {
            b.created_by_user_id = handle_to_id.get(&b.created_by).cloned();
        }
        if b.approval_user_ids.is_empty() && !b.approvals.is_empty() {
            for a in &b.approvals {
                if let Some(id) = handle_to_id.get(a) {
                    b.approval_user_ids.push(id.clone());
                }
            }
            b.approval_user_ids.sort();
            b.approval_user_ids.dedup();
        }
    }
    for p in &mut repo.promotions {
        if p.promoted_by_user_id.is_none() {
            p.promoted_by_user_id = handle_to_id.get(&p.promoted_by).cloned();
        }
    }

    for r in &mut repo.releases {
        if r.released_by_user_id.is_none() {
            r.released_by_user_id = handle_to_id.get(&r.released_by).cloned();
        }
    }
}
