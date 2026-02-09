use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

impl ObjectId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
