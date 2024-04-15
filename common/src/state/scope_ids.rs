use dioxus::prelude::ScopeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ScopeIds {
    pub chatbar: Option<usize>,
    // Would be nice in future if there is a way to access a shared state without subscribing
    // This can then be removed
    pub file_transfer: Option<usize>,
    pub file_transfer_icon: Option<usize>,
    pub pending_message_component: Option<usize>,
}

impl ScopeIds {
    pub fn scope_id_from_usize(scope_usize: usize) -> ScopeId {
        ScopeId(scope_usize)
    }
}
