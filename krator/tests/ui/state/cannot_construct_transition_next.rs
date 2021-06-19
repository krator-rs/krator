// Test that State<T> can only transition to State<T>
// edition:2018
extern crate async_trait;
extern crate krator;
extern crate k8s_openapi;

use krator::{Transition, state::StateHolder, ObjectState};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Status;
use k8s_openapi::api::core::v1::Pod;
use krator::state::test::Stub;

struct PodState;
struct ProviderState;

#[async_trait::async_trait]
impl ObjectState for PodState {
    type Manifest = Pod;
    type Status = Status;
    type SharedState = ProviderState;
    async fn async_drop(self, _provider_state: &mut ProviderState) { }
}

fn main() {
    // This fails because `state` is a private field. Use Transition::next classmethod instead.
    let _transition = Transition::<PodState>::Next(StateHolder {
        state: Box::new(Stub),
    });
}
