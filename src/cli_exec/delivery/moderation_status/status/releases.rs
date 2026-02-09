pub(super) fn latest_by_channel(
    releases: Vec<converge::remote::Release>,
) -> std::collections::BTreeMap<String, converge::remote::Release> {
    let mut latest_by_channel = std::collections::BTreeMap::new();
    for r in releases {
        match latest_by_channel.get(&r.channel) {
            None => {
                latest_by_channel.insert(r.channel.clone(), r);
            }
            Some(prev) => {
                if r.released_at > prev.released_at {
                    latest_by_channel.insert(r.channel.clone(), r);
                }
            }
        }
    }
    latest_by_channel
}
