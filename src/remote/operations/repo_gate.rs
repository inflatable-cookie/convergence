use super::*;

impl RemoteClient {
    pub fn create_repo(&self, repo_id: &str) -> Result<Repo> {
        let resp = self
            .client
            .post(self.url("/repos"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&CreateRepoRequest {
                id: repo_id.to_string(),
            })
            .send()
            .context("create repo request")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote endpoint not found (is converge-server running?)");
        }

        let resp = self.ensure_ok(resp, "create repo")?;
        let repo: Repo = resp.json().context("parse create repo response")?;
        Ok(repo)
    }

    pub fn list_publications(&self) -> Result<Vec<Publication>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/publications", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list publications")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let pubs: Vec<Publication> = self
            .ensure_ok(resp, "list publications")?
            .json()
            .context("parse publications")?;
        Ok(pubs)
    }

    pub fn get_gate_graph(&self) -> Result<GateGraph> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/gate-graph", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get gate graph")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let graph: GateGraph = self
            .ensure_ok(resp, "get gate graph")?
            .json()
            .context("parse gate graph")?;
        Ok(graph)
    }

    pub fn put_gate_graph(&self, graph: &GateGraph) -> Result<GateGraph> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .put(self.url(&format!("/repos/{}/gate-graph", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(graph)
            .send()
            .context("put gate graph")?;

        if resp.status() == reqwest::StatusCode::BAD_REQUEST {
            let v: GateGraphValidationError =
                resp.json().context("parse gate graph validation error")?;
            if v.issues.is_empty() {
                anyhow::bail!(v.error);
            }

            let mut lines: Vec<String> = Vec::new();
            lines.push(v.error);
            for i in v.issues.iter().take(8) {
                let mut bits = Vec::new();
                bits.push(i.code.clone());
                if let Some(g) = &i.gate {
                    bits.push(format!("gate={}", g));
                }
                if let Some(u) = &i.upstream {
                    bits.push(format!("upstream={}", u));
                }
                lines.push(format!("- {}: {}", bits.join(" "), i.message));
            }
            if v.issues.len() > 8 {
                lines.push(format!("... and {} more", v.issues.len() - 8));
            }
            anyhow::bail!(lines.join("\n"));
        }
        let graph: GateGraph = self
            .ensure_ok(resp, "put gate graph")?
            .json()
            .context("parse gate graph")?;
        Ok(graph)
    }
}
