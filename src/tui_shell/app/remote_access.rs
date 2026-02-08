use super::*;

impl App {
    pub(in crate::tui_shell) fn remote_config(&mut self) -> Option<RemoteConfig> {
        let ws = self.require_workspace()?;
        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };
        cfg.remote
    }

    pub(in crate::tui_shell) fn remote_client(&mut self) -> Option<RemoteClient> {
        let ws = self.require_workspace()?;

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };
        let Some(remote) = cfg.remote else {
            self.push_error("no remote configured".to_string());
            return None;
        };

        let token = match ws.store.get_remote_token(&remote) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.push_error(
                    "no remote token configured (run `login --url ... --token ... --repo ...`)"
                        .to_string(),
                );
                return None;
            }
            Err(err) => {
                self.push_error(format!("read remote token: {:#}", err));
                return None;
            }
        };

        match RemoteClient::new(remote, token) {
            Ok(c) => Some(c),
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                None
            }
        }
    }
}
