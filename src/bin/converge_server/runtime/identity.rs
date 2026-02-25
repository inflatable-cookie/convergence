use std::collections::HashMap;

use anyhow::{Context, Result};

use super::Args;
use crate::types::{AccessToken, User};

use crate::identity_store::{
    bootstrap_identity, load_identity_from_disk, persist_identity_to_disk,
};

pub(super) fn load_or_bootstrap_identity(
    args: &Args,
) -> Result<(HashMap<String, User>, HashMap<String, AccessToken>)> {
    let (mut users, mut tokens) =
        load_identity_from_disk(&args.data_dir).context("load identity")?;

    if users.is_empty() || tokens.is_empty() {
        if args.bootstrap_token.is_some() {
            if !(users.is_empty() && tokens.is_empty()) {
                anyhow::bail!(
                    "identity store inconsistent (users/tokens missing); remove {} to re-bootstrap",
                    args.data_dir.display()
                );
            }
        } else {
            let (u, t) = bootstrap_identity(&args.dev_user, &args.dev_token);
            users.insert(u.id.clone(), u);
            tokens.insert(t.id.clone(), t);
            persist_identity_to_disk(&args.data_dir, &users, &tokens)
                .context("persist identity")?;
        }
    }

    Ok((users, tokens))
}

#[cfg(test)]
#[path = "../../../tests/bin/converge_server/runtime/identity_tests.rs"]
mod tests;
