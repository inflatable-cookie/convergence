//! Encrypted secret verbs and the identity/key helpers they share.
use anyhow::{Context, Result};

use converge_client::workspace::Workspace;

use crate::dispatch::remote_client;
use crate::{OutputMode, Session};

/// `db-password` becomes `DB_PASSWORD`: the conventional shape, and
/// predictable enough that nobody has to look it up.
pub(crate) fn env_name_for(secret_name: &str) -> String {
    secret_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Single-quote for dotenv, escaping embedded quotes. A secret with a
/// newline or a space in it is still a secret.
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn restrict_file(path: &std::path::Path) -> Result<()> {
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

/// Add the written path to `.convergeignore` if it is not covered.
///
/// A plaintext dotenv captured into a snap would be the leak this whole
/// roadmap exists to prevent, so the escape hatch closes that door
/// behind itself rather than trusting anyone to remember.
pub(crate) fn ensure_ignored(ws: &Workspace, path: &std::path::Path) -> Result<bool> {
    let entry = path.display().to_string();
    let ignore_path = ws.root.join(".convergeignore");
    let existing = std::fs::read_to_string(&ignore_path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| line.trim() == entry || line.trim() == entry.trim_end_matches('/'))
    {
        return Ok(false);
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&entry);
    updated.push('\n');
    std::fs::write(&ignore_path, updated)
        .with_context(|| format!("update {}", ignore_path.display()))?;
    Ok(true)
}

/// Write a new value, keeping whoever could already read it.
///
/// The recipient list is *preserved and re-resolved*: sealing to only
/// the caller's keys would silently unshare everyone else (the defect
/// batch 20.3 found), and sealing to the stored key ids would lock out
/// anyone who has rotated since. Both failures are quiet, which is what
/// makes them worth spelling out here.
pub(crate) fn write_value(
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    name: &str,
    value: &str,
) -> Result<converge_client::model::SecretSummary> {
    let existing = client.get_secret(repo_id, name).ok();
    let registered = client.list_keys(repo_id)?;

    let (recipients, key_ids) = match &existing {
        Some(record) => {
            // Subjects who can read it now, resolved to their current
            // keys.
            let mut subjects: Vec<String> = record
                .recipients
                .iter()
                .filter_map(|key_id| {
                    registered
                        .iter()
                        .find(|k| &k.key_id == key_id)
                        .map(|k| k.subject.clone())
                })
                .collect();
            subjects.push(record.owner.clone());
            subjects.sort();
            subjects.dedup();

            let mut keys = Vec::new();
            let mut ids = Vec::new();
            for key in registered.iter().filter(|k| subjects.contains(&k.subject)) {
                keys.push(
                    key.public_key
                        .parse::<age::x25519::Recipient>()
                        .map_err(|err| anyhow::anyhow!("key {} is unusable: {err}", key.key_id))?,
                );
                ids.push(key.key_id.clone());
            }
            (keys, ids)
        }
        None => {
            let mine = my_recipients(client, repo_id)?;
            (mine.keys, mine.key_ids)
        }
    };

    // Preserving recipients is right (20.3) and leaving a departed
    // member's key on a secret is right (20.2) — but together they mean
    // a rotation re-seals the new value to someone who has left. They
    // cannot fetch it while their grants are gone; re-adding them later
    // would hand them everything rotated in between. Say so here, where
    // the person can act on it.
    warn_about_departed_recipients(client, repo_id, name, &key_ids)?;

    let ciphertext = converge_client::identity::seal(&recipients, value.as_bytes())?;
    // Read-modify-write against the version guard from 19.2: if someone
    // else wrote while we were typing, this is refused rather than
    // erasing them.
    let current = existing.map(|record| record.version).unwrap_or(0);
    client.write_secret(repo_id, name, &ciphertext, &key_ids, current, true)
}

/// Warn when a preserved recipient list still seals to people who have
/// left the repo.
///
/// A warning rather than a refusal: someone rotating mid-incident needs
/// the new value stored, and a hard stop would send them to a worse
/// workaround. Written to stderr so `--json` output stays parseable.
fn warn_about_departed_recipients(
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    name: &str,
    key_ids: &[String],
) -> Result<()> {
    let members = client.list_members(repo_id)?;
    let keys = client.list_keys(repo_id)?;
    let mut departed: Vec<String> = key_ids
        .iter()
        .filter_map(|key_id| keys.iter().find(|k| &k.key_id == key_id))
        .filter(|key| !members.iter().any(|m| m.subject == key.subject))
        .map(|key| key.subject.clone())
        .collect();
    departed.sort();
    departed.dedup();
    if departed.is_empty() {
        return Ok(());
    }
    eprintln!(
        "warning: this secret is still sealed to {}, who left the repo.",
        departed.join(", ")
    );
    eprintln!("  They cannot reach the server now, but would regain this value if");
    // Both the secret and the people are known here -- batch 22.4 found
    // this printing `<name>` and `<subject>` at a person who had just
    // been told exactly which secret and exactly who.
    eprintln!("  re-added. To close that:");
    for subject in &departed {
        eprintln!("    converge secret unshare {name} --from {subject}");
    }
    Ok(())
}

/// Re-seal a secret to a changed recipient set (batch 20.1).
///
/// Sharing is an encryption-time decision, so it costs a decrypt and a
/// re-encrypt by someone who can already read the secret. There is no
/// server-side shortcut, and doc 19 §7 says there must not be one.
pub(crate) fn reseal(
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    name: &str,
    add: &[String],
    remove: &[String],
) -> Result<(converge_client::model::SecretSummary, Vec<String>)> {
    let record = client.get_secret(repo_id, name)?;
    let keys = unlock_local_keys()?;
    let plaintext = converge_client::identity::open(&keys, &record.ciphertext)?;

    let registered = client.list_keys(repo_id)?;
    let subject_of = |key_id: &str| {
        registered
            .iter()
            .find(|k| k.key_id == key_id)
            .map(|k| k.subject.clone())
    };

    // Start from who can read it now, minus anyone being removed.
    let mut subjects: Vec<String> = record
        .recipients
        .iter()
        .filter_map(|key_id| subject_of(key_id))
        .collect();
    subjects.push(record.owner.clone());
    subjects.retain(|subject| !remove.contains(subject));
    for subject in add {
        if !subjects.contains(subject) {
            subjects.push(subject.clone());
        }
    }
    subjects.sort();
    subjects.dedup();

    // Every registered key of every recipient: a teammate who rotated
    // must not be locked out by a share that only saw their old key.
    let mut recipients = Vec::new();
    let mut key_ids = Vec::new();
    for record in &registered {
        if !subjects.contains(&record.subject) {
            continue;
        }
        recipients.push(
            record
                .public_key
                .parse::<age::x25519::Recipient>()
                .map_err(|err| anyhow::anyhow!("key {} is unusable: {err}", record.key_id))?,
        );
        key_ids.push(record.key_id.clone());
    }
    let missing: Vec<&String> = subjects
        .iter()
        .filter(|s| !registered.iter().any(|k| &&k.subject == s))
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "no registered key for {}; they need to run `converge key init` first",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let ciphertext = converge_client::identity::seal(&recipients, &plaintext)?;
    // A re-share is not a rotation: leaving `value_changed` false keeps
    // the audit's answer to "when did this credential last change?"
    // truthful across any number of membership edits.
    let summary =
        client.write_secret(repo_id, name, &ciphertext, &key_ids, record.version, false)?;
    let changed: Vec<String> = if add.is_empty() {
        remove.to_vec()
    } else {
        add.to_vec()
    };
    Ok((summary, changed))
}

/// Device-code sign-in against the server's identity provider
/// (batch 21.3).
///
/// The browser dance lives here rather than in the server: a server that
/// owned refresh cycles and provider quirks would be a second identity
/// system rather than a seam.
pub(crate) fn sign_in_with_provider(base_url: &str, mode: OutputMode) -> Result<String> {
    use converge_client::remote::RemoteClient;

    let config = RemoteClient::auth_config(base_url)?;
    if !config["oidc"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            config["detail"]
                .as_str()
                .unwrap_or("this server has no identity provider configured")
        );
    }
    let issuer = config["issuer"].as_str().context("server gave no issuer")?;
    let client_id = config["client_id"]
        .as_str()
        .context("server gave no client id")?;

    let http = reqwest::blocking::Client::new();
    let start: serde_json::Value = http
        .post(format!("{}/device/code", issuer.trim_end_matches('/')))
        .form(&[("client_id", client_id), ("scope", "openid profile email")])
        .send()
        .context("start device sign-in")?
        .json()
        .context("parse device response")?;

    let device_code = start["device_code"]
        .as_str()
        .context("provider gave no device code")?;
    if mode == OutputMode::Human {
        println!(
            "To sign in, visit {} and enter the code {}",
            start["verification_uri"]
                .as_str()
                .unwrap_or("(the URL it gave)"),
            start["user_code"].as_str().unwrap_or("(the code it gave)")
        );
    }

    // Poll at the provider's pace. `authorization_pending` is the normal
    // answer while the person is still in the browser, so it is a wait
    // rather than a failure.
    let interval = start["interval"].as_u64().unwrap_or(5).max(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("sign-in timed out; run `converge login --oidc` again");
        }
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let polled: serde_json::Value = http
            .post(format!("{}/token", issuer.trim_end_matches('/')))
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .context("poll for sign-in")?
            .json()
            .context("parse sign-in response")?;

        if let Some(id_token) = polled["id_token"].as_str() {
            let issued =
                converge_client::remote::RemoteClient::exchange_identity(base_url, id_token)?;
            if mode == OutputMode::Human {
                println!("signed in as {}", issued.record.subject);
                if !issued.record.expires_at.is_empty() {
                    println!("  this session expires {}", issued.record.expires_at);
                }
            }
            return Ok(issued.token);
        }
        match polled["error"].as_str() {
            Some("authorization_pending") | Some("slow_down") | None => continue,
            Some(other) => anyhow::bail!("sign-in refused: {other}"),
        }
    }
}

/// The caller's own registered keys in this repo.
///
/// Every one of them, not just the newest: sealing only to the latest
/// key would make a rotation strand every secret written before it.
struct MyKeys {
    keys: Vec<age::x25519::Recipient>,
    key_ids: Vec<String>,
}

fn my_recipients(client: &converge_client::remote::RemoteClient, repo_id: &str) -> Result<MyKeys> {
    let local = converge_client::identity::local_keys()?;
    if local.is_empty() {
        anyhow::bail!("no personal key on this machine; run `converge key init`");
    }
    let registered = client.list_keys(repo_id)?;
    let mine: Vec<&converge_client::model::PublicKeyRecord> = registered
        .iter()
        .filter(|record| local.iter().any(|k| k.key_id == record.key_id))
        .collect();
    if mine.is_empty() {
        anyhow::bail!(
            "none of this machine's keys are registered with this repo; \
             run `converge key rotate` to register one"
        );
    }
    let mut keys = Vec::new();
    let mut key_ids = Vec::new();
    for record in mine {
        keys.push(
            record
                .public_key
                .parse::<age::x25519::Recipient>()
                .map_err(|err| {
                    anyhow::anyhow!("registered key {} is unusable: {err}", record.key_id)
                })?,
        );
        key_ids.push(record.key_id.clone());
    }
    Ok(MyKeys { keys, key_ids })
}

/// Unlock every local key with one passphrase.
///
/// Keys made at different times may have different passphrases; the
/// ones that do not open are skipped rather than failing the command,
/// because only one of them has to fit the secret being read.
pub(crate) fn unlock_local_keys() -> Result<Vec<converge_client::identity::KeyPair>> {
    let passphrase = read_passphrase(false)?;
    let mut opened = Vec::new();
    for key in converge_client::identity::local_keys()? {
        if let Ok(pair) = converge_client::identity::KeyPair::load(Some(&key.key_id), &passphrase) {
            opened.push(pair);
        }
    }
    if opened.is_empty() {
        anyhow::bail!("that passphrase did not open any key on this machine");
    }
    Ok(opened)
}

/// Read a secret value from stdin: hidden prompt on a terminal, piped
/// input otherwise. Never from argv, which shell history and `ps` both
/// capture.
pub(crate) fn read_secret_value() -> Result<String> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        let value = rpassword::prompt_password("value: ").context("read value")?;
        if value.is_empty() {
            anyhow::bail!("value must not be empty");
        }
        return Ok(value);
    }
    let mut value = String::new();
    std::io::stdin()
        .read_to_string(&mut value)
        .context("read value from stdin")?;
    // One trailing newline is the shell's, not the secret's: `echo x |`
    // is the common case and would otherwise store "x\n".
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    if value.is_empty() {
        anyhow::bail!("value must not be empty");
    }
    Ok(value)
}

/// Prompt for a passphrase, or take it from `CONVERGE_PASSPHRASE`.
///
/// The env var exists because tests and CI need one; it is documented as
/// the weaker path since an environment variable is visible to anything
/// running as you.
pub(crate) fn read_passphrase(confirm: bool) -> Result<age::secrecy::SecretString> {
    if let Ok(from_env) = std::env::var("CONVERGE_PASSPHRASE") {
        return Ok(age::secrecy::SecretString::from(from_env));
    }
    let first = rpassword::prompt_password("passphrase: ").context("read passphrase")?;
    if first.is_empty() {
        anyhow::bail!("passphrase must not be empty");
    }
    if confirm {
        let again =
            rpassword::prompt_password("passphrase (again): ").context("read passphrase")?;
        if again != first {
            anyhow::bail!("passphrases did not match");
        }
    }
    Ok(age::secrecy::SecretString::from(first))
}

pub(crate) fn default_label() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "this machine".to_string())
}

pub(crate) fn now_rfc3339() -> Result<String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .context("format timestamp")
}

/// Register a public key when a remote is configured; say so plainly
/// when there is none, rather than failing a local operation that
/// succeeded.
pub(crate) fn register_key_if_possible(
    session: &Session,
    public: &converge_client::identity::PublicKey,
) -> Result<bool> {
    let Ok(ws) = session.workspace() else {
        return Ok(false);
    };
    let Ok((client, remote)) = remote_client(session, &ws, OutputMode::Capture) else {
        return Ok(false);
    };
    client.register_key(&remote.repo_id, &public.public_key, &public.label)?;
    Ok(true)
}
