//! Batch 19.4: attack the claims.
//!
//! Every property doc 19 asserts is worth exactly as much as the test
//! that tries to break it. These are the four that matter: a wrong key
//! opens nothing, tampering fails loudly rather than yielding altered
//! plaintext, the server cannot decrypt what it holds, and the
//! workspace no longer carries a bearer token in cleartext.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use anyhow::Result;
use converge_client::remote::RemoteClient;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn converge_with_stdin(dir: &Path, home: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .env("CONVERGE_HOME", home)
        .env("CONVERGE_PASSPHRASE", "correct horse battery staple")
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn converge");
    if let Some(text) = stdin {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(text.as_bytes())
            .expect("write stdin");
    }
    child.wait_with_output().expect("run converge")
}

fn converge(dir: &Path, home: &Path, args: &[&str]) -> Output {
    converge_with_stdin(dir, home, args, None)
}

fn start_server(data_dir: &Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "default", "2026-07-25T00:00:00Z")?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "secret", "admin"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([("token-a".to_string(), "alice".to_string())]),
        gc_running: Default::default(),
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt listener");
            axum::serve(listener, router(state)).await.expect("serve");
        });
    });
    Ok(format!("http://{addr}"))
}

fn logged_in(base_url: &str) -> Result<(tempfile::TempDir, tempfile::TempDir)> {
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    assert!(
        converge(
            ws.path(),
            home.path(),
            &[
                "login", "--url", base_url, "--token", "token-a", "--repo", "repo", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );
    Ok((ws, home))
}

/// A key that did not encrypt the secret opens nothing, and says so
/// rather than returning something plausible.
#[test]
fn another_persons_key_opens_nothing() -> Result<()> {
    let identity = age::x25519::Identity::generate();
    let stranger = age::x25519::Identity::generate();

    let sealed = converge_client::identity::seal(&[identity.to_public()], b"hunter2")?;
    let opened = age::decrypt(&stranger, sealed.as_bytes());
    assert!(
        opened.is_err(),
        "a key that never received the secret decrypted it"
    );

    // And the right key still works, so the refusal was the key rather
    // than a broken envelope.
    let reader = age::armor::ArmoredReader::new(sealed.as_bytes());
    let decryptor = age::Decryptor::new(reader)?;
    let mut stream = decryptor.decrypt(std::iter::once(&identity as &dyn age::Identity))?;
    let mut plaintext = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut plaintext)?;
    assert_eq!(plaintext, b"hunter2");
    Ok(())
}

/// Tampering must fail, not silently alter. An envelope that decrypted
/// to *something* after modification would be worse than one that
/// refuses: the caller would act on it.
#[test]
fn tampered_ciphertext_is_refused_rather_than_altered() -> Result<()> {
    let identity = age::x25519::Identity::generate();
    let sealed = converge_client::identity::seal(&[identity.to_public()], b"hunter2")?;

    // Flip a byte in the armored body, leaving the header intact.
    let mut lines: Vec<String> = sealed.lines().map(str::to_string).collect();
    let body = lines
        .iter()
        .position(|line| !line.starts_with("-----") && !line.starts_with("age-encryption"))
        .expect("a body line");
    let mut bytes = lines[body].clone().into_bytes();
    let last = bytes.len() - 1;
    bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
    lines[body] = String::from_utf8(bytes)?;
    let tampered = lines.join("\n");

    let opened = age::decrypt(&identity, tampered.as_bytes());
    assert!(
        opened.is_err(),
        "modified ciphertext decrypted; the AEAD is not being checked"
    );
    Ok(())
}

/// The server holds ciphertext and nothing that opens it. Stated as a
/// test rather than a comment, because "the server cannot read your
/// secrets" is the entire product claim.
#[test]
fn the_server_holds_nothing_that_can_decrypt() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = logged_in(&base_url)?;
    assert!(
        converge(ws.path(), home.path(), &["key", "init", "--yes"])
            .status
            .success()
    );
    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "db-password"],
        Some("hunter2"),
    );

    // Everything the server persisted, as bytes.
    let mut persisted = std::fs::read(server_dir.path().join("meta.sqlite"))?;
    for entry in walkdir(server_dir.path()) {
        if entry.extension().is_some_and(|e| e == "sqlite") {
            continue;
        }
        persisted.extend(std::fs::read(&entry).unwrap_or_default());
    }
    let text = String::from_utf8_lossy(&persisted);

    assert!(!text.contains("hunter2"), "plaintext reached the server");
    assert!(
        !text.contains("AGE-SECRET-KEY"),
        "a private key reached the server"
    );
    // Armored age base64-encodes its own header, so the recognisable
    // marker is the PEM-style boundary rather than the version line.
    assert!(
        text.contains("BEGIN AGE ENCRYPTED FILE"),
        "expected the stored ciphertext to be an armored age file"
    );

    // The ciphertext is there and is genuinely sealed: a fresh key
    // cannot open it.
    let client = RemoteClient::new(&base_url, "token-a");
    let record = client.get_secret("repo", "db-password")?;
    let stranger = age::x25519::Identity::generate();
    assert!(
        age::decrypt(&stranger, record.ciphertext.as_bytes()).is_err(),
        "the stored envelope opened with an unrelated key"
    );
    Ok(())
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// The workspace must not carry a bearer token in cleartext — the
/// finding that opened this whole roadmap.
#[test]
fn the_workspace_no_longer_holds_a_plaintext_token() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = logged_in(&base_url)?;

    let state = std::fs::read_to_string(ws.path().join(".converge/state.json"))?;
    assert!(
        !state.contains("token-a"),
        "the token is still sitting in the workspace: {state}"
    );

    // It moved to the user's home, encrypted, owner-only.
    let tokens: Vec<std::path::PathBuf> = walkdir(&home.path().join("tokens"));
    assert_eq!(tokens.len(), 1, "expected one stored token");
    // Binary age here rather than armored: nothing reads this file by
    // eye, so the armor would only add bytes.
    let sealed = std::fs::read(&tokens[0])?;
    let head = String::from_utf8_lossy(&sealed);
    assert!(
        head.starts_with("age-encryption.org/"),
        "the token is not stored as an age file"
    );
    assert!(!head.contains("token-a"), "the token is not encrypted");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&tokens[0])?.permissions().mode();
        assert_eq!(mode & 0o077, 0, "stored token is group- or world-readable");
    }

    // And it still works: remote commands do not prompt for anything.
    let listed = converge(ws.path(), home.path(), &["--json", "secret", "list"]);
    assert!(
        listed.status.success(),
        "the migrated token stopped working: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    Ok(())
}

/// A workspace written before this batch is migrated on first read, not
/// left with both copies.
#[test]
fn a_legacy_plaintext_token_is_migrated_and_erased() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = logged_in(&base_url)?;

    // Put the workspace back into the old shape by hand.
    let state_path = ws.path().join(".converge/state.json");
    let mut state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path)?)?;
    state["remote_tokens"] = serde_json::json!({ format!("{base_url}#repo"): "token-a" });
    std::fs::write(&state_path, serde_json::to_vec_pretty(&state)?)?;
    for path in walkdir(&home.path().join("tokens")) {
        std::fs::remove_file(path)?;
    }

    // Any remote command reads the token, and the read migrates it.
    let out = converge(ws.path(), home.path(), &["--json", "secret", "list"]);
    assert!(
        out.status.success(),
        "the legacy token was not usable: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = std::fs::read_to_string(&state_path)?;
    assert!(
        !after.contains("token-a"),
        "the plaintext copy survived the migration: {after}"
    );
    assert_eq!(
        walkdir(&home.path().join("tokens")).len(),
        1,
        "the migrated token was not written to the user's home"
    );
    Ok(())
}
