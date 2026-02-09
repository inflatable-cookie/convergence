use super::super::login_bootstrap_validate::validate_login_inputs;
use super::*;

impl crate::tui_shell::App {
    pub(in crate::tui_shell) fn continue_login_wizard(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        if self.login_wizard.is_none() {
            self.push_error("login wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::LoginUrl => {
                if let Some(w) = self.login_wizard.as_mut() {
                    w.url = Some(value);
                }
                self.open_text_input_modal(
                    "Login",
                    "token> ",
                    TextInputAction::LoginToken,
                    None,
                    vec![
                        "Access token (will be stored locally).".to_string(),
                        "Tip: paste it, then Enter.".to_string(),
                    ],
                );
            }
            TextInputAction::LoginToken => {
                if let Some(w) = self.login_wizard.as_mut() {
                    w.token = Some(value);
                }
                let repo_initial = self.login_wizard.as_ref().and_then(|w| w.repo.clone());
                self.open_text_input_modal(
                    "Login",
                    "repo> ",
                    TextInputAction::LoginRepo,
                    repo_initial,
                    vec!["Repo id".to_string()],
                );
            }
            TextInputAction::LoginRepo => {
                if let Some(w) = self.login_wizard.as_mut() {
                    w.repo = Some(value);
                }
                let scope_initial = self.login_wizard.as_ref().map(|w| w.scope.clone());
                self.open_text_input_modal(
                    "Login",
                    "scope> ",
                    TextInputAction::LoginScope,
                    scope_initial,
                    vec!["Scope id".to_string()],
                );
            }
            TextInputAction::LoginScope => {
                if let Some(w) = self.login_wizard.as_mut()
                    && !value.is_empty()
                {
                    w.scope = value;
                }
                let gate_initial = self.login_wizard.as_ref().map(|w| w.gate.clone());
                self.open_text_input_modal(
                    "Login",
                    "gate> ",
                    TextInputAction::LoginGate,
                    gate_initial,
                    vec!["Gate id".to_string()],
                );
            }
            TextInputAction::LoginGate => {
                if let Some(w) = self.login_wizard.as_mut()
                    && !value.is_empty()
                {
                    w.gate = value;
                }

                let (base_url, token, repo_id, scope, gate) = match self.login_wizard.as_ref() {
                    Some(w) => {
                        let base_url = w.url.clone().unwrap_or_default();
                        let token = w.token.clone().unwrap_or_default();
                        let repo_id = w.repo.clone().unwrap_or_default();
                        let scope = w.scope.clone();
                        let gate = w.gate.clone();
                        (base_url, token, repo_id, scope, gate)
                    }
                    None => {
                        self.push_error("login wizard not active".to_string());
                        return;
                    }
                };

                if let Err(err) = validate_login_inputs(&base_url, &token, &repo_id) {
                    self.push_error(err);
                    self.login_wizard = None;
                    return;
                }

                self.login_wizard = None;
                self.apply_login_config(base_url, token, repo_id, scope, gate);
            }

            _ => self.push_error("unexpected login wizard input".to_string()),
        }
    }
}
