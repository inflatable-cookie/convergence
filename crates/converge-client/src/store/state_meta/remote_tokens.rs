use anyhow::Result;

use crate::model::RemoteConfig;

use super::LocalStore;

impl LocalStore {
    pub fn remote_token_key(&self, remote: &RemoteConfig) -> String {
        format!("{}#{}", remote.base_url, remote.repo_id)
    }

    pub fn get_remote_token(&self, remote: &RemoteConfig) -> Result<Option<String>> {
        let st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        Ok(st
            .remote_tokens
            .get(&self.remote_token_key(remote))
            .cloned())
    }

    pub fn set_remote_token(&self, remote: &RemoteConfig, token: &str) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.remote_tokens
            .insert(self.remote_token_key(remote), token.to_string());
        self.write_state(&st)
    }

    pub fn clear_remote_token(&self, remote: &RemoteConfig) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.remote_tokens.remove(&self.remote_token_key(remote));
        self.write_state(&st)
    }
}
