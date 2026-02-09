use clap::Args;

#[derive(Args)]
pub(crate) struct LoginArgs {
    #[arg(long)]
    pub(crate) url: String,
    #[arg(long)]
    pub(crate) token: String,
    #[arg(long)]
    pub(crate) repo: String,
    #[arg(long, default_value = "main")]
    pub(crate) scope: String,
    #[arg(long, default_value = "dev-intake")]
    pub(crate) gate: String,
}

#[derive(Args)]
pub(crate) struct WhoamiArgs {
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}
