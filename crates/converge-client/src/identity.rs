//! Personal key material (batch 19.1, doc 19 §4).
//!
//! An X25519 keypair per person. The private half is encrypted at rest
//! under a passphrase and never leaves the machine; the public half is
//! registered with the server so secrets can be sealed to it.
//!
//! Keys live under the *user's* home, not the workspace: an identity is
//! a person, not a checkout, and a second workspace must not mean a
//! second identity that existing secrets were never encrypted to.

use std::path::{Path, PathBuf};

use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

/// Where personal keys live. `CONVERGE_HOME` overrides, which is what
/// makes this testable without touching a developer's real keys.
pub fn converge_home() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("CONVERGE_HOME") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("no HOME set; use CONVERGE_HOME to place personal keys")?;
    Ok(PathBuf::from(home).join(".converge"))
}

fn keys_dir_in(home: &Path) -> PathBuf {
    home.join("keys")
}

/// The public half of a key, plus what a person needs to recognise it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicKey {
    /// Short stable handle: blake3 of the recipient string. Not a
    /// secret, and short enough to read out loud.
    pub key_id: String,
    /// age recipient string (`age1...`).
    pub public_key: String,
    /// Free-text hint, usually the machine the key was made on.
    pub label: String,
    pub created_at: String,
}

/// Local index of this machine's keys, so `key list` and key selection
/// never need the passphrase.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct KeyIndex {
    version: u32,
    keys: Vec<PublicKey>,
}

pub fn key_id_for(public_key: &str) -> String {
    blake3::hash(public_key.as_bytes())
        .to_hex()
        .chars()
        .take(16)
        .collect()
}

/// A usable keypair: the decrypted private half plus its public record.
pub struct KeyPair {
    identity: age::x25519::Identity,
    pub public: PublicKey,
}

impl KeyPair {
    /// Generate a keypair and write the private half encrypted under
    /// `passphrase`.
    ///
    /// There is no recovery path (doc 19 §1): a lost passphrase is lost
    /// secrets. Callers are expected to have said so out loud before
    /// getting here.
    pub fn create(passphrase: &SecretString, label: &str, now: &str) -> Result<Self> {
        Self::create_in(&converge_home()?, passphrase, label, now)
    }

    /// As [`KeyPair::create`], against an explicit home. Every path in
    /// this module has one of these: a test must be able to point
    /// somewhere other than the developer's real keys, and threading a
    /// directory is safer than mutating a process-wide env var while
    /// other tests run.
    pub fn create_in(
        home: &Path,
        passphrase: &SecretString,
        label: &str,
        now: &str,
    ) -> Result<Self> {
        let identity = age::x25519::Identity::generate();
        let public_key = identity.to_public().to_string();
        let public = PublicKey {
            key_id: key_id_for(&public_key),
            public_key,
            label: label.to_string(),
            created_at: now.to_string(),
        };

        let secret = identity.to_string();
        let sealed = age::encrypt(
            &age::scrypt::Recipient::new(passphrase.clone()),
            secret.expose_secret().as_bytes(),
        )
        .map_err(|err| anyhow!("encrypt private key: {err}"))?;

        let dir = keys_dir_in(home);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let path = dir.join(format!("{}.age", public.key_id));
        write_private(&path, &sealed)?;

        let mut index = load_index(home)?;
        index.version = 1;
        index.keys.push(public.clone());
        save_index(home, &index)?;

        Ok(Self { identity, public })
    }

    /// Load a key by id, or the newest one when `key_id` is `None`.
    pub fn load(key_id: Option<&str>, passphrase: &SecretString) -> Result<Self> {
        Self::load_in(&converge_home()?, key_id, passphrase)
    }

    pub fn load_in(home: &Path, key_id: Option<&str>, passphrase: &SecretString) -> Result<Self> {
        let index = load_index(home)?;
        let public = match key_id {
            Some(id) => index
                .keys
                .iter()
                .find(|k| k.key_id == id)
                .ok_or_else(|| anyhow!("no local key {id}; run `converge key init`"))?,
            None => index.keys.last().ok_or_else(|| {
                anyhow!("no personal key on this machine; run `converge key init`")
            })?,
        }
        .clone();

        let path = keys_dir_in(home).join(format!("{}.age", public.key_id));
        let sealed =
            std::fs::read(&path).with_context(|| format!("read private key {}", path.display()))?;
        let plaintext = age::decrypt(&age::scrypt::Identity::new(passphrase.clone()), &sealed)
            .map_err(|_| anyhow!("wrong passphrase for key {}", public.key_id))?;
        let secret = String::from_utf8(plaintext).context("private key is not utf-8")?;
        let identity: age::x25519::Identity = secret
            .trim()
            .parse()
            .map_err(|err| anyhow!("parse private key: {err}"))?;

        // A file swapped under us would otherwise decrypt to someone
        // else's key without complaint.
        let actual = identity.to_public().to_string();
        if actual != public.public_key {
            anyhow::bail!(
                "key {} does not match its recorded public half",
                public.key_id
            );
        }
        Ok(Self { identity, public })
    }

    pub fn recipient(&self) -> age::x25519::Recipient {
        self.identity.to_public()
    }

    pub fn identity(&self) -> &age::x25519::Identity {
        &self.identity
    }
}

/// Every key this machine holds, newest last. Public data only.
pub fn local_keys() -> Result<Vec<PublicKey>> {
    local_keys_in(&converge_home()?)
}

pub fn local_keys_in(home: &Path) -> Result<Vec<PublicKey>> {
    Ok(load_index(home)?.keys)
}

fn index_path(home: &Path) -> PathBuf {
    keys_dir_in(home).join("index.json")
}

fn load_index(home: &Path) -> Result<KeyIndex> {
    let path = index_path(home);
    if !path.exists() {
        return Ok(KeyIndex::default());
    }
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn save_index(home: &Path, index: &KeyIndex) -> Result<()> {
    let path = index_path(home);
    let bytes = serde_json::to_vec_pretty(index).context("serialize key index")?;
    crate::store::write_atomic(&path, &bytes)
}

/// Write the sealed key with owner-only permissions where the platform
/// has them. The file is already encrypted; this is the second lock.
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    crate::store::write_atomic(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("restrict {}", path.display()))?;
    }
    Ok(())
}

/// Seal `plaintext` to every recipient (batch 19.3).
///
/// Armored so the ciphertext is ASCII: a database row and a JSON field
/// both carry it without escaping, and anyone looking at storage can
/// see at a glance that it is an age file rather than a value.
pub fn seal(recipients: &[age::x25519::Recipient], plaintext: &[u8]) -> Result<String> {
    if recipients.is_empty() {
        anyhow::bail!("no recipients: the result could never be decrypted");
    }
    let refs: Vec<&dyn age::Recipient> = recipients
        .iter()
        .map(|r| r as &dyn age::Recipient)
        .collect();
    let encryptor = age::Encryptor::with_recipients(refs.into_iter())
        .map_err(|err| anyhow!("prepare encryption: {err}"))?;

    let mut armored = Vec::new();
    {
        use std::io::Write;
        let writer =
            age::armor::ArmoredWriter::wrap_output(&mut armored, age::armor::Format::AsciiArmor)?;
        let mut stream = encryptor
            .wrap_output(writer)
            .map_err(|err| anyhow!("start encryption: {err}"))?;
        stream.write_all(plaintext).context("write plaintext")?;
        stream
            .finish()
            .and_then(|armor| armor.finish())
            .map_err(|err| anyhow!("finish encryption: {err}"))?;
    }
    String::from_utf8(armored).context("armored output is not utf-8")
}

/// Open `ciphertext` with the first key that fits.
///
/// Trying every local key is what makes rotation survivable: a secret
/// sealed before a rotation is still readable afterwards, because the
/// old key is still on the machine (batch 19.1 keeps it deliberately).
pub fn open(keys: &[KeyPair], ciphertext: &str) -> Result<Vec<u8>> {
    if keys.is_empty() {
        anyhow::bail!("no personal key on this machine; run `converge key init`");
    }
    let reader = age::armor::ArmoredReader::new(ciphertext.as_bytes());
    let decryptor = age::Decryptor::new(reader).map_err(|err| anyhow!("read age file: {err}"))?;
    let identities: Vec<&dyn age::Identity> = keys
        .iter()
        .map(|k| k.identity() as &dyn age::Identity)
        .collect();
    let mut stream = decryptor
        .decrypt(identities.into_iter())
        .map_err(|_| anyhow!("none of this machine\'s keys can open that secret"))?;
    let mut plaintext = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut plaintext).context("read plaintext")?;
    Ok(plaintext)
}
