use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkerState {
    Installing,
    Installed,
    Activating,
    Activated,
    Redundant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Registration {
    pub scope: Url,
    pub script_url: Url,
    pub state: WorkerState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FetchResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServiceWorkerRegistry {
    registrations: BTreeMap<String, Registration>,
    responses: BTreeMap<String, FetchResponse>,
}

impl ServiceWorkerRegistry {
    pub fn register(&mut self, scope: Url, script_url: Url) -> Registration {
        let registration = Registration {
            scope: scope.clone(),
            script_url,
            state: WorkerState::Activated,
        };
        self.registrations
            .insert(scope.as_str().into(), registration.clone());
        registration
    }
    pub fn get(&self, scope: &Url) -> Option<&Registration> {
        self.registrations.get(scope.as_str())
    }

    pub fn transition(&mut self, scope: &Url, next: WorkerState) -> Option<Registration> {
        let registration = self.registrations.get_mut(scope.as_str())?;
        let valid = matches!(
            (registration.state, next),
            (WorkerState::Installing, WorkerState::Installed)
                | (WorkerState::Installing, WorkerState::Redundant)
                | (WorkerState::Installed, WorkerState::Activating)
                | (WorkerState::Installed, WorkerState::Redundant)
                | (WorkerState::Activating, WorkerState::Activated)
                | (WorkerState::Activating, WorkerState::Redundant)
                | (WorkerState::Activated, WorkerState::Redundant)
        );
        if !valid {
            return None;
        }
        registration.state = next;
        Some(registration.clone())
    }

    pub fn unregister(&mut self, scope: &Url) -> bool {
        let removed = self.registrations.remove(scope.as_str()).is_some();
        if removed {
            self.responses.retain(|url, _| {
                Url::parse(url)
                    .map(|candidate| !scope_contains(scope, &candidate))
                    .unwrap_or(true)
            });
        }
        removed
    }
    pub fn intercept(&mut self, url: &Url, response: FetchResponse) {
        self.responses.insert(url.as_str().into(), response);
    }
    pub fn fetch(&self, url: &Url) -> Option<&FetchResponse> {
        let controlled = self
            .registrations
            .values()
            .filter(|registration| {
                registration.state == WorkerState::Activated
                    && scope_contains(&registration.scope, url)
            })
            .max_by_key(|registration| registration.scope.as_str().len());
        controlled.and_then(|_| self.responses.get(url.as_str()))
    }
}

fn scope_contains(scope: &Url, url: &Url) -> bool {
    let scope_path = scope.path();
    let path_matches = if scope_path.ends_with('/') {
        url.path().starts_with(scope_path)
    } else {
        url.path() == scope_path || url.path().starts_with(&format!("{scope_path}/"))
    };
    scope.scheme() == url.scheme()
        && scope.host_str() == url.host_str()
        && scope.port_or_known_default() == url.port_or_known_default()
        && path_matches
}
