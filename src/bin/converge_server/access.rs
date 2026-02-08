use super::*;

pub(super) fn can_read(repo: &Repo, subject: &Subject) -> bool {
    repo.owner == subject.user
        || repo.readers.contains(&subject.user)
        || repo
            .owner_user_id
            .as_ref()
            .is_some_and(|u| u == &subject.user_id)
        || repo.reader_user_ids.contains(&subject.user_id)
}

pub(super) fn can_publish(repo: &Repo, subject: &Subject) -> bool {
    repo.owner == subject.user
        || repo.publishers.contains(&subject.user)
        || repo
            .owner_user_id
            .as_ref()
            .is_some_and(|u| u == &subject.user_id)
        || repo.publisher_user_ids.contains(&subject.user_id)
}
