//! Batch 19.1: personal key material end to end.
//!
//! The properties worth pinning are the ones the whole substrate rests
//! on: the private key is useless without the passphrase, the server
//! only ever sees public halves, and nobody can register a key as
//! somebody else.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use anyhow::Result;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn converge(dir: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .env("CONVERGE_HOME", home)
        .env("CONVERGE_PASSPHRASE", "correct horse battery staple")
        .args(args)
        .output()
        .expect("run converge")
}

fn json_data(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).expect("parse envelope");
    assert_eq!(v["ok"], true, "envelope not ok: {v}");
    v["data"].clone()
}

fn start_server(data_dir: &Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "default", "2026-07-25T00:00:00Z")?;
    for subject in ["alice", "bob"] {
        meta.upsert_user(subject)?;
        meta.add_grant(subject, "repo", "*", "read")?;
        meta.add_grant(subject, "repo", "*", "publish")?;
    }
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([
            ("token-a".to_string(), "alice".to_string()),
            ("token-b".to_string(), "bob".to_string()),
        ]),
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

fn login(dir: &Path, home: &Path, base_url: &str, token: &str) {
    let out = converge(
        dir,
        home,
        &[
            "login", "--url", base_url, "--token", token, "--repo", "repo", "--scope", "default",
            "--gate", "intake",
        ],
    );
    assert!(out.status.success(), "login failed");
}

#[test]
fn key_init_registers_a_public_half_and_keeps_the_private_one_sealed() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    let ws = ws.path();
    assert!(converge(ws, home.path(), &["init"]).status.success());
    login(ws, home.path(), &base_url, "token-a");

    let key = json_data(&converge(
        ws,
        home.path(),
        &["--json", "key", "init", "--label", "laptop", "--yes"],
    ));
    let key_id = key["key_id"].as_str().expect("key id").to_string();
    assert_eq!(key["registered"], true);
    assert!(
        key["public_key"].as_str().unwrap().starts_with("age1"),
        "public half is an age recipient: {key}"
    );

    // On disk: the private key is an age file, not a key. Nothing in it
    // resembles the secret it protects.
    let sealed = std::fs::read(home.path().join("keys").join(format!("{key_id}.age")))?;
    let text = String::from_utf8_lossy(&sealed);
    assert!(
        text.starts_with("age-encryption.org/"),
        "private key is not stored as an age file"
    );
    assert!(
        !text.contains("AGE-SECRET-KEY"),
        "the private key is sitting in plaintext"
    );

    // Owner-only permissions where the platform has them.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.path().join("keys").join(format!("{key_id}.age")))?
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "private key is group- or world-readable");
    }

    // The server stores the public half against the *token's* subject.
    let listed = json_data(&converge(ws, home.path(), &["--json", "key", "list"]));
    let repo_keys = listed["repo"].as_array().expect("repo keys");
    assert_eq!(repo_keys.len(), 1);
    assert_eq!(repo_keys[0]["subject"], "alice");
    assert_eq!(repo_keys[0]["key_id"], key_id.as_str());
    assert_eq!(
        listed["local"].as_array().map(Vec::len),
        Some(1),
        "the machine knows its own key"
    );

    // Nothing the server holds can decrypt anything: the whole keys
    // table is public halves.
    let dump = std::fs::read(server_dir.path().join("meta.sqlite"))?;
    assert!(
        !String::from_utf8_lossy(&dump).contains("AGE-SECRET-KEY"),
        "a private key reached the server"
    );
    Ok(())
}

#[test]
fn a_wrong_passphrase_does_not_open_the_key() -> Result<()> {
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    assert!(
        converge(ws.path(), home.path(), &["--json", "key", "init", "--yes"])
            .status
            .success(),
        "key init should work without a remote"
    );

    let keys = converge_client::identity::local_keys_in(home.path())?;
    let key_id = keys.last().expect("a key").key_id.clone();

    let wrong = age::secrecy::SecretString::from("not the passphrase".to_string());
    let message =
        match converge_client::identity::KeyPair::load_in(home.path(), Some(&key_id), &wrong) {
            Ok(_) => panic!("a wrong passphrase must not open the key"),
            Err(err) => format!("{err:#}").to_lowercase(),
        };
    assert!(
        message.contains("passphrase"),
        "the error should name the passphrase, got: {message}"
    );

    // The right one still works, so the failure was the passphrase and
    // not a corrupt file.
    let right = age::secrecy::SecretString::from("correct horse battery staple".to_string());
    let pair = converge_client::identity::KeyPair::load_in(home.path(), Some(&key_id), &right)?;
    assert_eq!(pair.public.key_id, key_id);
    Ok(())
}

#[test]
fn key_init_without_a_remote_still_produces_a_usable_key() -> Result<()> {
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());

    let key = json_data(&converge(
        ws.path(),
        home.path(),
        &["--json", "key", "init", "--yes"],
    ));
    assert_eq!(
        key["registered"], false,
        "no remote configured, and the command says so rather than failing"
    );
    assert!(key["key_id"].as_str().is_some_and(|id| !id.is_empty()));
    Ok(())
}

#[test]
fn rotation_registers_a_new_key_and_keeps_the_old_one() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    let ws = ws.path();
    assert!(converge(ws, home.path(), &["init"]).status.success());
    login(ws, home.path(), &base_url, "token-a");

    let first = json_data(&converge(
        ws,
        home.path(),
        &["--json", "key", "init", "--yes"],
    ));
    let second = json_data(&converge(ws, home.path(), &["--json", "key", "rotate"]));
    assert_ne!(first["key_id"], second["key_id"]);
    assert_eq!(second["registered"], true);

    // Both keys survive: secrets sealed to the old one must stay
    // readable until something re-encrypts them (19.3).
    let listed = json_data(&converge(ws, home.path(), &["--json", "key", "list"]));
    assert_eq!(listed["local"].as_array().map(Vec::len), Some(2));
    assert_eq!(listed["repo"].as_array().map(Vec::len), Some(2));
    Ok(())
}

#[test]
fn a_member_cannot_register_a_key_as_someone_else() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    login(ws.path(), home.path(), &base_url, "token-b");
    converge(ws.path(), home.path(), &["--json", "key", "init", "--yes"]);

    // Bob's token registered the key, so the record says bob no matter
    // what any request body might have claimed.
    let listed = json_data(&converge(
        ws.path(),
        home.path(),
        &["--json", "key", "list"],
    ));
    let repo_keys = listed["repo"].as_array().expect("repo keys");
    assert_eq!(repo_keys.len(), 1);
    assert_eq!(
        repo_keys[0]["subject"], "bob",
        "the subject comes from the token, not the caller"
    );
    Ok(())
}
