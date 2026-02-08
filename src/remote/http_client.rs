use super::*;

pub(super) fn with_retries<T>(label: &str, mut f: impl FnMut() -> Result<T>) -> Result<T> {
    const ATTEMPTS: usize = 3;
    let mut last: Option<anyhow::Error> = None;
    for i in 0..ATTEMPTS {
        match f() {
            Ok(v) => return Ok(v),
            Err(err) => {
                last = Some(err);
                if i + 1 < ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_millis(200 * (1 << i)));
                }
            }
        }
    }
    Err(last
        .unwrap_or_else(|| anyhow::anyhow!("unknown error"))
        .context(label.to_string()))
}

impl RemoteClient {
    pub(super) fn ensure_ok(
        &self,
        resp: reqwest::blocking::Response,
        label: &str,
    ) -> Result<reqwest::blocking::Response> {
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!(
                "unauthorized (token invalid/expired; run `converge login --url ... --token ... --repo ...`)"
            );
        }
        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "forbidden (insufficient permissions; check repo membership/role or admin)"
            );
        }
        resp.error_for_status()
            .with_context(|| format!("{} status", label))
    }

    pub(super) fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}{}", self.remote.base_url, path)
    }
}
