use anyhow::{Context, Result};

use crate::model::RemoteConfig;

use super::LocalStore;

/// Remote bearer tokens (batch 19.4).
///
/// These used to sit in `state.json` in cleartext, inside the workspace,
/// where anything reading the repository read them — a backup, a stray
/// `cat`, and above all an agent exploring the tree (doc 19 §10a).
///
/// They now live under `CONVERGE_HOME`, encrypted at rest under a
/// machine-local key. What that buys is precise and worth stating: the
/// credential is no longer *in the repository*, and no longer readable
/// by eye. It is not protection against a determined attacker running
/// as you — the machine key is on the same disk, necessarily.
/// Encrypting to the personal key would fix that and would also prompt
/// for a passphrase on every remote command, which nobody would
/// tolerate; people would keep a plaintext copy elsewhere instead, which
/// is worse than this.
impl LocalStore {
    pub fn remote_token_key(&self, remote: &RemoteConfig) -> String {
        format!("{}#{}", remote.base_url, remote.repo_id)
    }

    pub fn get_remote_token(&self, remote: &RemoteConfig) -> Result<Option<String>> {
        let key = self.remote_token_key(remote);

        // Migrate on read: a workspace written before this batch still
        // has the token in `state.json`, and leaving it there would make
        // the encrypted copy pointless.
        let state = self.read_state()?;
        if state.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", state.version);
        }
        if let Some(legacy) = state.remote_tokens.get(&key).cloned() {
            self.write_token_file(&key, &legacy)?;
            self.mutate_state(|st| {
                st.remote_tokens.remove(&key);
                Ok(())
            })?;
            return Ok(Some(legacy));
        }

        self.read_token_file(&key)
    }

    pub fn set_remote_token(&self, remote: &RemoteConfig, token: &str) -> Result<()> {
        let key = self.remote_token_key(remote);
        self.write_token_file(&key, token)?;
        // Belt and braces: a workspace that had a plaintext copy loses
        // it here too, not only on the read path.
        self.mutate_state(|st| {
            st.remote_tokens.remove(&key);
            Ok(())
        })
    }

    pub fn clear_remote_token(&self, remote: &RemoteConfig) -> Result<()> {
        let key = self.remote_token_key(remote);
        let path = token_path(&key)?;
        if path.exists() {
            std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
        self.mutate_state(|st| {
            st.remote_tokens.remove(&key);
            Ok(())
        })
    }

    fn write_token_file(&self, key: &str, token: &str) -> Result<()> {
        let sealed = age::encrypt(
            &age::scrypt::Recipient::new(machine_key()?),
            token.as_bytes(),
        )
        .map_err(|err| anyhow::anyhow!("encrypt token: {err}"))?;
        let path = token_path(key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        crate::store::write_atomic(&path, &sealed)?;
        restrict(&path)
    }

    fn read_token_file(&self, key: &str) -> Result<Option<String>> {
        let path = token_path(key)?;
        if !path.exists() {
            return Ok(None);
        }
        let sealed = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let plaintext = age::decrypt(&age::scrypt::Identity::new(machine_key()?), &sealed)
            .map_err(|_| {
                anyhow::anyhow!(
                    "stored token for this remote could not be decrypted; \
                     run `converge login` again to replace it"
                )
            })?;
        Ok(Some(
            String::from_utf8(plaintext).context("token is not utf-8")?,
        ))
    }
}

/// Hashed, so a directory listing does not enumerate which servers this
/// machine talks to.
fn token_path(key: &str) -> Result<std::path::PathBuf> {
    let name = blake3::hash(key.as_bytes()).to_hex().to_string();
    Ok(crate::identity::converge_home()?
        .join("tokens")
        .join(format!("{name}.age")))
}

/// A random machine-local key, created once, readable only by its owner.
fn machine_key() -> Result<age::secrecy::SecretString> {
    let path = crate::identity::converge_home()?.join("machine.key");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(age::secrecy::SecretString::from(trimmed.to_string()));
        }
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("read system randomness: {err}"))?;
    let key = blake3::hash(&bytes).to_hex().to_string();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    crate::store::write_atomic(&path, key.as_bytes())?;
    restrict(&path)?;
    Ok(age::secrecy::SecretString::from(key))
}

fn restrict(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("restrict {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
