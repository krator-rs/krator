// Test that TransitionTo is required for a transition to compile. 
// edition:2018
extern crate async_trait;
extern crate anyhow;
extern crate krator;
extern crate k8s_openapi;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Status;
use k8s_openapi::api::core::v1::Pod;
use krator::state::test::Stub;
use krator::{TransitionTo, ObjectState, State, SharedState, Manifest, Transition};

#[derive(Debug)]
struct TestState;

// impl TransitionTo<TestState> for TestState {}

struct PodState;
struct ProviderState;

#[async_trait::async_trait]
impl ObjectState for PodState {
    type Manifest = Pod;
    type Status = Status;
    type SharedState = ProviderState;
    async fn async_drop(self, _provider_state: &mut ProviderState) { }
}

#[async_trait::async_trait]
impl State<PodState> for TestState {
    async fn next(
        self: Box<Self>,
        _provider_state: SharedState<ProviderState>,
        _state: &mut PodState,
        _pod: Manifest<Pod>,
    ) -> Transition<PodState> {
        // This fails because TestState is not TransitionTo<TestState>
        Transition::next(self, TestState)
    }

    async fn status(
        &self,
        _state: &mut PodState,
        _pod: &Pod,
    ) -> anyhow::Result<Status> {
        Ok(Default::default())
    }
}

fn main() {}
