use super::super::remote_action_parse::parse_promote_args;
use super::*;

impl App {
    pub(in crate::tui_shell::app) fn cmd_promote(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Promote",
                "bundle id> ",
                TextInputAction::PromoteBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_promote_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.open_text_input_modal(
                "Promote",
                "bundle id> ",
                TextInputAction::PromoteBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        };

        let to_gate = match parsed.to_gate {
            Some(g) => g,
            None => {
                let graph = match client.get_gate_graph() {
                    Ok(g) => g,
                    Err(err) => {
                        self.push_error(format!("get gate graph: {:#}", err));
                        return;
                    }
                };

                let bundle = match client.get_bundle(&bundle_id) {
                    Ok(b) => b,
                    Err(err) => {
                        self.push_error(format!("get bundle: {:#}", err));
                        return;
                    }
                };
                let mut next = graph
                    .gates
                    .iter()
                    .filter(|g| g.upstream.iter().any(|u| u == &bundle.gate))
                    .map(|g| g.id.clone())
                    .collect::<Vec<_>>();
                next.sort();
                if next.len() == 1 {
                    next[0].clone()
                } else {
                    if next.is_empty() {
                        self.push_error("no downstream gates for bundle gate".to_string());
                        return;
                    }
                    self.start_promote_wizard(bundle_id.clone(), next, None);
                    return;
                }
            }
        };

        match client.promote_bundle(&bundle_id, &to_gate) {
            Ok(_) => self.push_output(vec![format!("promoted {} -> {}", bundle_id, to_gate)]),
            Err(err) => self.push_error(format!("promote: {:#}", err)),
        }
    }
}
