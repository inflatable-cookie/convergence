use super::*;

pub(crate) async fn whoami(Extension(subject): Extension<Subject>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "user": subject.user,
        "user_id": subject.user_id,
        "admin": subject.admin,
    }))
}
