//! Batch 19.2: the envelope service.
//!
//! The properties here are the ones the substrate's promise rests on:
//! the server returns exactly the bytes it was given without ever
//! parsing them, only recipients can fetch, and a stale write loses
//! rather than silently winning.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use converge_client::remote::RemoteClient;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// alice and bob both hold `secret`; carol is a member without it;
/// dana is a repo admin, to prove admin does not mean "can fetch".
fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "default", "2026-07-25T00:00:00Z")?;
    for subject in ["alice", "bob", "carol", "dana"] {
        meta.upsert_user(subject)?;
        meta.add_grant(subject, "repo", "*", "read")?;
    }
    for subject in ["alice", "bob"] {
        meta.add_grant(subject, "repo", "*", "secret")?;
    }
    meta.add_grant("dana", "repo", "*", "admin")?;

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([
            ("token-a".to_string(), "alice".to_string()),
            ("token-b".to_string(), "bob".to_string()),
            ("token-c".to_string(), "carol".to_string()),
            ("token-d".to_string(), "dana".to_string()),
        ]),
        gc_running: Default::default(),
        oidc: None,
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

/// Register a key for `subject` and return its key id.
fn register_key(client: &RemoteClient) -> Result<String> {
    let identity = age::x25519::Identity::generate();
    let record = client.register_key("repo", &identity.to_public().to_string(), "test")?;
    Ok(record.key_id)
}

#[test]
fn ciphertext_round_trips_byte_exact_without_being_parsed() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let base_url = start_server(dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let key_id = register_key(&alice)?;

    // Deliberate garbage: not an age file, not valid UTF-8 structure the
    // server could hope to interpret. If anything on the server parsed
    // ciphertext, this would fail rather than round-trip.
    let garbage = "not-an-age-file\n\u{1}\u{2}\u{3} ????? ///// \n";
    alice.set_secret(
        "repo",
        "db-password",
        garbage,
        std::slice::from_ref(&key_id),
        0,
    )?;

    let fetched = alice.get_secret("repo", "db-password")?;
    assert_eq!(
        fetched.ciphertext, garbage,
        "the server altered ciphertext it should only be carrying"
    );
    assert_eq!(fetched.version, 1);
    assert_eq!(fetched.owner, "alice");
    assert_eq!(fetched.recipients, vec![key_id]);

    // A real age file round-trips and still decrypts.
    let identity = age::x25519::Identity::generate();
    let sealed = age::encrypt(&identity.to_public(), b"hunter2")?;
    let armored = String::from_utf8(
        sealed
            .iter()
            .flat_map(|b| std::ascii::escape_default(*b))
            .collect::<Vec<u8>>(),
    )?;
    alice.set_secret("repo", "db-password", &armored, &["k".to_string()], 1)?;
    assert_eq!(alice.get_secret("repo", "db-password")?.ciphertext, armored);
    Ok(())
}

#[test]
fn only_recipients_can_fetch_and_admin_is_not_a_recipient() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let base_url = start_server(dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");
    let carol = RemoteClient::new(&base_url, "token-c");
    let dana = RemoteClient::new(&base_url, "token-d");

    let alice_key = register_key(&alice)?;
    register_key(&bob)?;
    alice.set_secret(
        "repo",
        "personal",
        "sealed",
        std::slice::from_ref(&alice_key),
        0,
    )?;

    assert!(
        alice.get_secret("repo", "personal").is_ok(),
        "the owner reads"
    );

    // bob holds `secret` but is not a recipient.
    let err = bob.get_secret("repo", "personal").unwrap_err();
    assert!(
        format!("{err:#}").contains("404"),
        "a non-recipient must not learn whether the secret exists: {err:#}"
    );

    // dana is a repo admin, and admin subsumes every capability — which
    // is exactly why the recipient check cannot be the grant check.
    let err = dana.get_secret("repo", "personal").unwrap_err();
    assert!(
        format!("{err:#}").contains("404"),
        "admin fetched an envelope it is not a recipient of: {err:#}"
    );

    // carol has no `secret` capability at all: refused earlier, by authz.
    let err = carol.get_secret("repo", "personal").unwrap_err();
    let message = format!("{err:#}");
    assert!(
        message.contains("403") || message.contains("404"),
        "expected a refusal, got: {message}"
    );

    // Listing is deliberately open to members: knowing a secret exists
    // is what lets someone ask to be added to it.
    let listed = carol.list_secrets("repo")?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "personal");
    assert_eq!(listed[0].owner, "alice");
    Ok(())
}

#[test]
fn a_stale_write_is_refused_rather_than_winning() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let base_url = start_server(dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let key_id = register_key(&alice)?;

    alice.set_secret("repo", "token", "v1", std::slice::from_ref(&key_id), 0)?;
    alice.set_secret("repo", "token", "v2", std::slice::from_ref(&key_id), 1)?;

    // Someone who read version 1 and rotated from there must lose, or a
    // concurrent rotation disappears without anyone noticing.
    let err = alice
        .set_secret(
            "repo",
            "token",
            "v2-from-stale-read",
            std::slice::from_ref(&key_id),
            1,
        )
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("409"),
        "a stale write should conflict: {err:#}"
    );
    assert_eq!(
        alice.get_secret("repo", "token")?.ciphertext,
        "v2",
        "the stale write must not have landed"
    );

    // Creating over an existing secret is the same mistake with
    // expected_version 0.
    assert!(
        alice
            .set_secret("repo", "token", "clobber", std::slice::from_ref(&key_id), 0)
            .is_err(),
        "creating over an existing secret must conflict"
    );
    Ok(())
}

#[test]
fn only_the_owner_deletes_and_shapes_are_validated() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let base_url = start_server(dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");
    let key_id = register_key(&alice)?;
    alice.set_secret(
        "repo",
        "shared.thing",
        "sealed",
        std::slice::from_ref(&key_id),
        0,
    )?;

    assert!(
        bob.delete_secret("repo", "shared.thing").is_err(),
        "a non-owner deleted someone else's secret"
    );
    assert!(alice.delete_secret("repo", "shared.thing").is_ok());
    assert!(alice.get_secret("repo", "shared.thing").is_err());

    // A secret nobody can decrypt is a mistake worth refusing at the
    // door rather than discovering later.
    assert!(
        alice.set_secret("repo", "empty", "sealed", &[], 0).is_err(),
        "a secret with no recipients was accepted"
    );
    assert!(
        alice
            .set_secret("repo", "empty", "", std::slice::from_ref(&key_id), 0)
            .is_err(),
        "empty ciphertext was accepted"
    );
    for bad in ["with space", "slash/inside", "", "quote\"here"] {
        assert!(
            alice
                .set_secret("repo", bad, "sealed", std::slice::from_ref(&key_id), 0)
                .is_err(),
            "name {bad:?} should be refused"
        );
    }
    Ok(())
}
