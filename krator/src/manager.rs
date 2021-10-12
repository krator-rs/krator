//! Defines types for registering controllers with runtime.
use crate::{operator::Operator, store::Store};
// use std::sync::Arc;
#[cfg(feature = "admission-webhook")]
use warp::Filter;
pub mod tasks;
use tasks::{controller_tasks, OperatorTask};

pub mod controller;
use controller::{Controller, ControllerBuilder};
mod watch;

/// Coordinates one or more controllers and the main entrypoint for starting
/// the application.
///
/// # Warning
///
/// This API does not support admissions webhooks yet, please
/// use [OperatorRuntime](crate::runtime::OperatorRuntime).
pub struct Manager {
    kubeconfig: kube::Config,
    controllers: Vec<Controller>,
    controller_tasks: Vec<OperatorTask>,
    store: Store,
    #[cfg(feature = "admission-webhook")]
    filter: warp::filters::BoxedFilter<(warp::reply::WithStatus<warp::reply::Json>,)>,
}

#[cfg(feature = "admission-webhook")]
fn not_found() -> warp::reply::WithStatus<warp::reply::Json> {
    warp::reply::with_status(warp::reply::json(&()), warp::http::StatusCode::NOT_FOUND)
}

impl Manager {
    /// Create a new controller manager.
    pub fn new(kubeconfig: &kube::Config) -> Self {
        #[cfg(feature = "admission-webhook")]
        let filter = { warp::any().map(not_found).boxed() };

        Manager {
            controllers: vec![],
            controller_tasks: vec![],
            kubeconfig: kubeconfig.clone(),
            store: Store::new(),
            #[cfg(feature = "admission-webhook")]
            filter,
        }
    }

    /// Register a controller with the manager.
    pub fn register_controller<C: Operator>(&mut self, builder: ControllerBuilder<C>) {
        #[cfg(feature = "admission-webhook")]
        for endpoint in builder.webhooks.values() {
            // Create temporary variable w/ throwaway filter of correct type.
            let mut temp = warp::any().map(not_found).boxed();

            // Swap self.filter into temporary.
            std::mem::swap(&mut temp, &mut self.filter);

            // Compose new filter from new endpoint and temporary (now holding original self.filter).
            let mut new_filter = endpoint.clone().or(temp).unify().boxed();

            // Swap new filter back into self.filter.
            std::mem::swap(&mut new_filter, &mut self.filter);

            // Throwaway filter stored in new_filter implicitly dropped.
        }

        let (controller, tasks) =
            controller_tasks(self.kubeconfig.clone(), builder, self.store.clone());

        self.controllers.push(controller);
        self.controller_tasks.extend(tasks);
    }

    /// Start the manager, blocking forever.
    pub async fn start(self) {
        use futures::FutureExt;
        use std::convert::TryFrom;
        use tasks::launch_watcher;

        let mut tasks = self.controller_tasks;
        let client = kube::Client::try_from(self.kubeconfig)
            .expect("Unable to create kube::Client from kubeconfig.");

        // TODO: Deduplicate Watchers
        for controller in self.controllers {
            tasks.push(launch_watcher(client.clone(), controller.manages).boxed());
            for handle in controller.owns {
                tasks.push(launch_watcher(client.clone(), handle).boxed());
            }
            for handle in controller.watches {
                tasks.push(launch_watcher(client.clone(), handle).boxed());
            }
        }

        #[cfg(feature = "admission-webhook")]
        {
            let task = warp::serve(self.filter)
                // .tls()
                // .cert(tls.cert)
                // .key(tls.private_key)
                .run(([0, 0, 0, 0], 8443));
            tasks.push(task.boxed());
        }

        futures::future::join_all(tasks).await;
    }
}
