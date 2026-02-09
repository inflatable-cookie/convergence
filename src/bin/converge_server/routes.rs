//! Authenticated HTTP route registration for the converge server.

use super::handlers_system::require_bearer;
use super::*;

#[path = "routes/register.rs"]
mod register;

pub(super) fn authed_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    register::register_routes(Router::new())
        .layer(middleware::from_fn_with_state(state, require_bearer))
}
