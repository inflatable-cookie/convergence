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
    /// Identifies the stored token for this workspace's login.
    ///
    /// The workspace root is part of the key (batch 21.1). Batch 19.4
    /// moved tokens to a shared home keyed by `(url, repo)` alone,
    /// which quietly made two workspaces on one machine share one
    /// credential: logging in as a second person replaced the first
    /// person's token in *their* workspace. Before 19.4 the token lived
    /// in the workspace, so this restores that scoping while keeping the
    /// credential out of the repository.
    pub fn remote_token_key(&self, remote: &RemoteConfig) -> String {
        format!(
            "{}#{}#{}",
            remote.base_url,
            remote.repo_id,
            self.root_dir().display()
        )
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
        // Pre-19.4 workspaces keyed by `url#repo`; the key gained the
        // workspace root in 21.1. Both shapes are looked for, because a
        // migration that only understood the newer one would silently
        // leave the plaintext where it was.
        let legacy_key = format!("{}#{}", remote.base_url, remote.repo_id);
        for candidate in [key.clone(), legacy_key] {
            if let Some(legacy) = state.remote_tokens.get(&candidate).cloned() {
                self.write_token_file(&key, &legacy)?;
                self.mutate_state(|st| {
                    st.remote_tokens.remove(&candidate);
                    Ok(())
                })?;
                return Ok(Some(legacy));
            }
        }

        self.read_token_file(&key)
    }

    pub fn set_remote_token(&self, remote: &RemoteConfig, token: &str) -> Result<()> {
        let key = self.remote_token_key(remote);
        self.write_token_file(&key, token)?;
        // Belt and braces: a workspace that had a plaintext copy loses
        // it here too, not only on the read path, in either key shape.
        let legacy_key = format!("{}#{}", remote.base_url, remote.repo_id);
        self.mutate_state(|st| {
            st.remote_tokens.remove(&key);
            st.remote_tokens.remove(&legacy_key);
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
        let record = serde_json::to_vec(&TokenRecord {
            key: key.to_string(),
            token: token.to_string(),
        })?;
        let sealed = age::encrypt(&age::scrypt::Recipient::new(machine_key()?), &record)
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
        let plaintext = String::from_utf8(plaintext).context("token is not utf-8")?;
        match serde_json::from_str::<TokenRecord>(&plaintext) {
            Ok(record) => Ok(Some(record.token)),
            // Written before the file recorded what it belonged to. Now
            // that the key is in hand, rewrite it in the current shape so
            // ordinary use migrates the store; what stays in the old
            // shape is what nothing has opened, which is the definition
            // of the debris `prune` is looking for.
            Err(_) => {
                self.write_token_file(key, &plaintext)?;
                Ok(Some(plaintext))
            }
        }
    }

    /// Follow a server that moved (`remote set-url`).
    ///
    /// The stored token is keyed by `url#repo#workspace_root` and its
    /// encrypted body embeds that key, so a URL change orphans a
    /// perfectly good credential. This decrypts under the machine key,
    /// re-encrypts under the new key string, and removes the old file —
    /// which is exactly what a person would otherwise be told to fix by
    /// logging in again, with a token they no longer have anywhere.
    ///
    /// Returns false when nothing was stored for the old remote.
    pub fn move_remote_token(&self, old: &RemoteConfig, new: &RemoteConfig) -> Result<bool> {
        let old_key = self.remote_token_key(old);
        let Some(token) = self.read_token_file(&old_key)? else {
            return Ok(false);
        };
        let new_key = self.remote_token_key(new);
        self.write_token_file(&new_key, &token)?;
        let old_path = token_path(&old_key)?;
        if old_path.exists() {
            std::fs::remove_file(&old_path)
                .with_context(|| format!("remove {}", old_path.display()))?;
        }
        Ok(true)
    }
}

/// What a token file holds.
///
/// The token alone was not enough. The filename is
/// `blake3(url#repo#workspace_root)` — hashed so a directory listing
/// does not enumerate which servers this machine talks to — which means
/// a deleted workspace leaves a credential nothing can attribute and
/// nothing removes. Batch 22.4 found 493 of them on one machine, almost
/// all from temporary test workspaces, with no way to tell the live one
/// from the dead.
///
/// Storing the key inside the encrypted body keeps the directory
/// listing as opaque as it was, while making staleness decidable: the
/// workspace either still exists or it does not.
#[derive(serde::Serialize, serde::Deserialize)]
struct TokenRecord {
    key: String,
    token: String,
}

/// A cached login that `prune` can account for.
#[derive(Debug)]
pub struct StaleToken {
    pub path: std::path::PathBuf,
    /// The workspace it was issued for, when the file says.
    pub workspace: Option<std::path::PathBuf>,
}

/// What the token store holds, and what of it is dead.
///
/// Deletion is never inferred from a failure to decrypt: a file this
/// machine's key cannot open is somebody else's problem, not garbage.
#[derive(Debug, Default)]
pub struct TokenStoreSurvey {
    pub live: usize,
    /// Workspaces that no longer exist on disk.
    pub stale: Vec<StaleToken>,
    /// Written before files recorded their key, and not opened since.
    pub unattributable: Vec<StaleToken>,
}

/// Survey the cached logins on this machine.
pub fn survey_token_store() -> Result<TokenStoreSurvey> {
    let dir = crate::identity::converge_home()?.join("tokens");
    let mut survey = TokenStoreSurvey::default();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(survey),
    };
    let key = machine_key()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("age") {
            continue;
        }
        let Ok(sealed) = std::fs::read(&path) else {
            continue;
        };
        let Ok(plaintext) = age::decrypt(&age::scrypt::Identity::new(key.clone()), &sealed) else {
            // Not ours to judge, so not ours to delete.
            survey.live += 1;
            continue;
        };
        let Ok(text) = String::from_utf8(plaintext) else {
            survey.live += 1;
            continue;
        };
        match serde_json::from_str::<TokenRecord>(&text) {
            Ok(record) => {
                // `url#repo#root`, and only the root may itself contain
                // a `#`, so it is the remainder rather than a field.
                let root = record
                    .key
                    .splitn(3, '#')
                    .nth(2)
                    .map(std::path::PathBuf::from);
                // The key holds `root_dir()`, which is the `.converge`
                // directory itself and not the workspace above it. The
                // first version of this check appended `.converge` a
                // second time, classified the one live credential on the
                // machine as stale, and would have deleted it — which is
                // why this command reports before it removes.
                let gone = root
                    .as_ref()
                    .is_some_and(|r| !r.join("config.json").exists());
                if gone {
                    survey.stale.push(StaleToken {
                        path,
                        // Report the workspace, not the `.converge`
                        // directory inside it: the former is what
                        // somebody recognises.
                        workspace: root.map(|r| r.parent().map(|p| p.to_path_buf()).unwrap_or(r)),
                    });
                } else {
                    survey.live += 1;
                }
            }
            Err(_) => survey.unattributable.push(StaleToken {
                path,
                workspace: None,
            }),
        }
    }
    survey.stale.sort_by(|a, b| a.path.cmp(&b.path));
    survey.unattributable.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(survey)
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
