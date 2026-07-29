use anyhow::Result;

use crate::model::RemoteConfig;

use super::LocalStore;

impl LocalStore {
    fn publish_key(&self, remote: &RemoteConfig, scope: &str, gate: &str) -> String {
        format!("{}#{}#{}#{}", remote.base_url, remote.repo_id, scope, gate)
    }

    pub fn get_last_published(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<Option<String>> {
        let st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        Ok(st
            .last_published
            .get(&self.publish_key(remote, scope, gate))
            .cloned())
    }

    pub fn get_last_seen_candidate(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<Option<String>> {
        let st = self.read_state()?;
        Ok(st
            .last_seen_candidate
            .get(&self.publish_key(remote, scope, gate))
            .cloned())
    }

    pub fn set_last_seen_candidate(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
        candidate_id: &str,
    ) -> Result<()> {
        let key = self.publish_key(remote, scope, gate);
        self.mutate_state(|st| {
            st.last_seen_candidate.insert(key, candidate_id.to_string());
            Ok(())
        })
    }

    /// Forget the candidate this workspace last saw for a target.
    ///
    /// Used when the server does not recognise it (batch 22.4) — after a
    /// rebuild or a restore whose candidate history differs. The recorded
    /// base is a claim about what *this* client saw; a server that never
    /// issued it cannot act on the claim, so keeping it only wedges the
    /// next publish.
    pub fn clear_last_seen_candidate(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<()> {
        let key = self.publish_key(remote, scope, gate);
        self.mutate_state(|st| {
            st.last_seen_candidate.remove(&key);
            Ok(())
        })
    }

    pub fn set_last_published(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
        snap_id: &str,
    ) -> Result<()> {
        let key = self.publish_key(remote, scope, gate);
        self.mutate_state(|st| {
            st.last_published.insert(key, snap_id.to_string());
            Ok(())
        })
    }
}

impl LocalStore {
    /// Rewrite every URL-keyed entry in `state.json` after the server
    /// moved (`remote set-url`). The values are all still true — the
    /// same deployment answers at the new address — so dropping them
    /// would needlessly re-derive publish bases and lane cursors.
    pub fn rekey_state_urls(&self, old_url: &str, new_url: &str) -> Result<()> {
        let old_prefix = format!("{old_url}#");
        let new_prefix = format!("{new_url}#");
        self.mutate_state(|state| {
            let rekey = |map: &mut std::collections::HashMap<String, String>| {
                let moved: Vec<(String, String)> = map
                    .iter()
                    .filter(|(k, _)| k.starts_with(&old_prefix))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (k, v) in moved {
                    map.remove(&k);
                    map.insert(k.replacen(&old_prefix, &new_prefix, 1), v);
                }
            };
            rekey(&mut state.last_seen_candidate);
            rekey(&mut state.last_published);
            // lane_sync is keyed by lane id alone, so it survives a URL
            // change untouched.
            Ok(())
        })
    }
}
