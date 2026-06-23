use uuid::Uuid;

use crate::{AppState, current_request_tenant_id};

impl AppState {
    pub(crate) fn configured_tenant_id(&self) -> Uuid {
        let Self { tenant_id, .. } = self;
        *tenant_id
    }

    pub(crate) fn current_tenant_id(&self) -> Uuid {
        current_request_tenant_id(self.configured_tenant_id())
    }
}
