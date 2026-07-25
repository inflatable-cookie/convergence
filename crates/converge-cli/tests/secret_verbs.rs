//! Batch 19.3: the secret verbs, driven the way a person drives them.
//!
//! The claim under test is the product's whole promise: a value stored
//! from one machine comes back on that machine and nowhere else — not
//! for a teammate holding every capability, and not for anyone reading
//! the server's database.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use anyhow::Result;
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
        for capability in ["read", "publish", "secret", "admin"] {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
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

/// A workspace with a logged-in, keyed identity.
fn member(base_url: &str, token: &str) -> Result<(tempfile::TempDir, tempfile::TempDir)> {
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    assert!(
        converge(
            ws.path(),
            home.path(),
            &[
                "login", "--url", base_url, "--token", token, "--repo", "repo", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );
    assert!(
        converge(ws.path(), home.path(), &["key", "init", "--yes"])
            .status
            .success()
    );
    Ok((ws, home))
}

#[test]
fn a_secret_round_trips_for_its_owner_and_nobody_else() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    let stored = json_data(&converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "set", "db-password"],
        Some("hunter2\n"),
    ));
    assert_eq!(stored["name"], "db-password");
    assert_eq!(stored["version"], 1);

    // The owner reads it back, and the trailing newline the shell added
    // is not part of the secret.
    let got = converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "get", "db-password"],
    );
    assert!(
        got.status.success(),
        "{}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "hunter2");

    // Bob holds every capability including admin, and cannot read it.
    let denied = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "db-password"],
    );
    assert!(
        !denied.status.success(),
        "a non-recipient read the secret: {}",
        String::from_utf8_lossy(&denied.stdout)
    );
    assert!(
        !String::from_utf8_lossy(&denied.stdout).contains("hunter2"),
        "the value leaked into a failed read"
    );

    // Bob can see that it exists — that is what lets him ask for access.
    let listed = json_data(&converge(
        bob_ws.path(),
        bob_home.path(),
        &["--json", "secret", "list"],
    ));
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    assert_eq!(listed[0]["name"], "db-password");
    assert_eq!(listed[0]["owner"], "alice");
    assert!(
        !listed.to_string().contains("hunter2"),
        "a listing carried the value"
    );

    // Nothing on the server holds the plaintext.
    let db = std::fs::read(server_dir.path().join("meta.sqlite"))?;
    assert!(
        !String::from_utf8_lossy(&db).contains("hunter2"),
        "the plaintext reached the server"
    );
    Ok(())
}

#[test]
fn the_value_never_appears_in_argv() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    // There is no flag that takes a value: the only way in is stdin.
    let help = converge(ws.path(), home.path(), &["secret", "set", "--help"]);
    let text = String::from_utf8_lossy(&help.stdout).to_lowercase();
    assert!(
        !text.contains("--value") && !text.contains("--from"),
        "a value-bearing flag exists, so values will end up in shell history:\n{text}"
    );
    Ok(())
}

#[test]
fn rotating_a_key_does_not_strand_existing_secrets() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "before-rotation"],
        Some("old-value\n"),
    );
    assert!(
        converge(ws.path(), home.path(), &["key", "rotate"])
            .status
            .success()
    );

    // Sealed to the old key, still readable: rotation keeps the old key
    // on the machine precisely so this works (batch 19.1).
    let got = converge(
        ws.path(),
        home.path(),
        &["secret", "get", "before-rotation"],
    );
    assert!(
        got.status.success(),
        "a rotation stranded an existing secret: {}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "old-value");

    // A secret written after the rotation is sealed to both keys.
    let stored = json_data(&converge_with_stdin(
        ws.path(),
        home.path(),
        &["--json", "secret", "set", "after-rotation"],
        Some("new-value\n"),
    ));
    assert_eq!(
        stored["recipients"].as_array().map(Vec::len),
        Some(2),
        "a new secret should be sealed to every key the caller holds"
    );
    Ok(())
}

#[test]
fn updating_and_deleting_behave_like_the_rest_of_the_cli() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "token"],
        Some("v1"),
    );
    let updated = json_data(&converge_with_stdin(
        ws.path(),
        home.path(),
        &["--json", "secret", "set", "token"],
        Some("v2"),
    ));
    assert_eq!(updated["version"], 2, "a second write is a new version");
    let got = converge(ws.path(), home.path(), &["secret", "get", "token"]);
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "v2");

    assert!(
        converge(ws.path(), home.path(), &["secret", "rm", "token"])
            .status
            .success()
    );
    let gone = converge(ws.path(), home.path(), &["secret", "get", "token"]);
    assert!(
        !gone.status.success(),
        "a deleted secret was still readable"
    );
    Ok(())
}

#[test]
fn a_machine_without_the_key_gets_told_what_to_do() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "thing"],
        Some("value"),
    );

    // Same person, new machine: logged in, no key yet.
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    assert!(
        converge(
            ws.path(),
            home.path(),
            &[
                "login", "--url", &base_url, "--token", "token-a", "--repo", "repo", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );

    let out = converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "another"],
        Some("value"),
    );
    let message = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(!out.status.success());
    assert!(
        message.contains("key init"),
        "the error should name the fix, got: {message}"
    );
    Ok(())
}

/// Batch 19.5: a secret reaches a child process without ever being
/// written into the working tree.
#[test]
fn run_injects_named_secrets_into_one_child_and_nowhere_else() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "db-password"],
        Some("hunter2"),
    );

    // The derived variable name is the conventional shape.
    let out = converge(
        ws.path(),
        home.path(),
        &[
            "run",
            "--secret",
            "db-password",
            "--",
            "sh",
            "-c",
            "printf %s \"$DB_PASSWORD\"",
        ],
    );
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hunter2");

    // An explicit mapping when the names differ.
    let out = converge(
        ws.path(),
        home.path(),
        &[
            "run",
            "--secret",
            "PGPASSWORD=db-password",
            "--",
            "sh",
            "-c",
            "printf %s \"$PGPASSWORD\"",
        ],
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hunter2");

    // Nothing was written to the workspace on the way through.
    let tree: Vec<String> = std::fs::read_dir(ws.path())?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !tree.iter().any(|name| name.contains("env")),
        "run left something behind: {tree:?}"
    );

    // The child's exit code is the command's answer, not ours.
    let failed = converge(
        ws.path(),
        home.path(),
        &["run", "--secret", "db-password", "--", "sh", "-c", "exit 3"],
    );
    assert_eq!(failed.status.code(), Some(3));
    Ok(())
}

/// The escape hatch closes the door behind itself.
#[test]
fn write_env_warns_and_self_ignores() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;
    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "api-key"],
        Some("s3cr3t value"),
    );

    let out = converge(ws.path(), home.path(), &["secret", "write-env", ".env"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let said = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(said.contains("plaintext"), "the warning is missing: {said}");
    assert!(
        said.contains("converge run"),
        "it should point at the better option: {said}"
    );

    // The file holds a usable value, quoted so a space survives.
    let written = std::fs::read_to_string(ws.path().join(".env"))?;
    assert_eq!(written.trim(), "API_KEY='s3cr3t value'");

    // And it is ignored, so no snap can ever capture it.
    let ignore = std::fs::read_to_string(ws.path().join(".convergeignore"))?;
    assert!(
        ignore.lines().any(|line| line.trim() == ".env"),
        "the dotenv was not added to .convergeignore: {ignore}"
    );
    let status = json_data(&converge(ws.path(), home.path(), &["--json", "status"]));
    let pending: Vec<&str> = status["pending"]["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["path"].as_str())
        .collect();
    assert!(
        !pending.contains(&".env"),
        "the dotenv is capturable despite the ignore rule: {pending:?}"
    );
    // The ignore file itself is project configuration and *should* show
    // up — a teammate restoring the snap needs it.
    assert!(
        pending.contains(&".convergeignore"),
        "expected the ignore file to be capturable: {pending:?}"
    );
    Ok(())
}

/// Reads are on the record (doc 19 §10c).
#[test]
fn reading_a_secret_leaves_an_audit_event() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;
    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "audited"],
        Some("v"),
    );
    converge(ws.path(), home.path(), &["secret", "get", "audited"]);

    let events = json_data(&converge(ws.path(), home.path(), &["--json", "events"]));
    let kinds: Vec<&str> = events
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(
        kinds.contains(&"secret.read"),
        "a read left no trace: {kinds:?}"
    );
    assert!(
        kinds.contains(&"secret.changed"),
        "a write left no trace: {kinds:?}"
    );
    assert!(
        !events.to_string().contains("\"v\""),
        "an event carried the value"
    );
    Ok(())
}
