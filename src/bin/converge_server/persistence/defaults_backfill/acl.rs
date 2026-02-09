use super::*;

pub(crate) fn backfill_acl_user_ids(repo: &mut Repo, handle_to_id: &HashMap<String, String>) {
    if repo.owner_user_id.is_none() {
        repo.owner_user_id = handle_to_id.get(&repo.owner).cloned();
    }
    if repo.reader_user_ids.is_empty() && !repo.readers.is_empty() {
        for h in &repo.readers {
            if let Some(id) = handle_to_id.get(h) {
                repo.reader_user_ids.insert(id.clone());
            }
        }
    }
    if repo.publisher_user_ids.is_empty() && !repo.publishers.is_empty() {
        for h in &repo.publishers {
            if let Some(id) = handle_to_id.get(h) {
                repo.publisher_user_ids.insert(id.clone());
            }
        }
    }

    for lane in repo.lanes.values_mut() {
        if lane.member_user_ids.is_empty() && !lane.members.is_empty() {
            for h in &lane.members {
                if let Some(id) = handle_to_id.get(h) {
                    lane.member_user_ids.insert(id.clone());
                }
            }
        }
    }
}
