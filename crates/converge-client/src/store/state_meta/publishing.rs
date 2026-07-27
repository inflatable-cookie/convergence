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

    pub fn get_last_seen_bundle(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<Option<String>> {
        let st = self.read_state()?;
        Ok(st
            .last_seen_bundle
            .get(&self.publish_key(remote, scope, gate))
            .cloned())
    }

    pub fn set_last_seen_bundle(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
        bundle_id: &str,
    ) -> Result<()> {
        let key = self.publish_key(remote, scope, gate);
        self.mutate_state(|st| {
            st.last_seen_bundle.insert(key, bundle_id.to_string());
            Ok(())
        })
    }

    /// Forget the bundle this workspace last saw for a target.
    ///
    /// Used when the server does not recognise it (batch 22.4) — after a
    /// rebuild or a restore whose bundle history differs. The recorded
    /// base is a claim about what *this* client saw; a server that never
    /// issued it cannot act on the claim, so keeping it only wedges the
    /// next publish.
    pub fn clear_last_seen_bundle(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<()> {
        let key = self.publish_key(remote, scope, gate);
        self.mutate_state(|st| {
            st.last_seen_bundle.remove(&key);
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
