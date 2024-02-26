use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

use kube::{
    api::{Api, Resource, ResourceExt},
    Client,
};
use kube_runtime::watcher;
use kube_runtime::watcher::Event;

use crate::manifest::Manifest;
use crate::object::ObjectKey;
use crate::object::ObjectState;
use crate::operator::Operator;
use crate::state::{run_to_completion, SharedState};
use crate::store::Store;
use crate::util::PrettyEvent;

#[derive(Debug)]
enum ObjectEvent<R> {
    Applied(R),
    Deleted {
        name: String,
        namespace: Option<String>,
    },
}

impl<R: Resource> From<&ObjectEvent<R>> for PrettyEvent {
    fn from(event: &ObjectEvent<R>) -> Self {
        match event {
            ObjectEvent::Applied(object) => PrettyEvent::Applied {
                name: object.name_any(),
                namespace: object.namespace(),
            },
            ObjectEvent::Deleted { name, namespace } => PrettyEvent::Deleted {
                name: name.to_string(),
                namespace: namespace.clone(),
            },
        }
    }
}

/// Accepts a type implementing the `Operator` trait and watches
/// for resources of the associated `Manifest` type, running the
/// associated state machine for each. Optionally filter by
/// `kube_runtime::watcher::Config`.
pub struct OperatorRuntime<O: Operator> {
    client: Client,
    handlers: HashMap<ObjectKey, Sender<ObjectEvent<O::Manifest>>>,
    operator: Arc<O>,
    watcher_config: watcher::Config,
    signal: Option<Arc<AtomicBool>>,
    store: Store,
}

impl<O: Operator> OperatorRuntime<O> {
    /// Create new runtime with optional watcher::Config.
    pub fn new(kubeconfig: &kube::Config, operator: O, watcher_config: Option<watcher::Config>) -> Self {
        let client = Client::try_from(kubeconfig.clone())
            .expect("Unable to create kube::Client from kubeconfig.");
        let watcher_config = watcher_config.unwrap_or_default();
        OperatorRuntime {
            client,
            handlers: HashMap::new(),
            operator: Arc::new(operator),
            watcher_config,
            signal: None,
            store: Store::new(),
        }
    }

    #[cfg(not(feature = "admission-webhook"))]
    pub(crate) fn new_with_store(
        kubeconfig: &kube::Config,
        operator: O,
        watcher_config: Option<watcher::Config>,
        store: Store,
    ) -> Self {
        let client = Client::try_from(kubeconfig.clone())
            .expect("Unable to create kube::Client from kubeconfig.");
        let watcher_config = watcher_config.unwrap_or_default();
        OperatorRuntime {
            client,
            handlers: HashMap::new(),
            operator: Arc::new(operator),
            watcher_config,
            signal: None,
            store,
        }
    }

    /// Dispatch event to the matching resource's task.
    /// If no task is found, `self.start_object` is called to start a task for
    /// the new object.
    #[tracing::instrument(
      level="trace",
      skip(self, event),
      fields(event = ?PrettyEvent::from(&event))
    )]
    async fn dispatch(&mut self, event: ObjectEvent<O::Manifest>) -> anyhow::Result<()> {
        match event {
            ObjectEvent::Applied(object) => {
                let key: ObjectKey = (&object).into();
                // We are explicitly not using the entry api here to insert to avoid the need for a
                // mutex
                match self.handlers.get_mut(&key) {
                    Some(sender) => {
                        trace!("Found existing event handler for object.");
                        match sender.send(ObjectEvent::Applied(object)).await {
                            Ok(_) => trace!("Successfully sent event to handler for object."),
                            Err(error) => error!(
                                name=key.name().to_string(),
                                namespace=?key.namespace(),
                                ?error,
                                "Error while sending event. Will retry on next event.",
                            ),
                        }
                    }
                    None => {
                        debug!(
                            name=key.name(),
                            namespace=?key.namespace(),
                            "Creating event handler for object.",
                        );
                        self.handlers.insert(
                            key.clone(),
                            // TODO Do we want to capture join handles? Worker wasnt using them.
                            // TODO How do we drop this sender / handler?
                            self.start_object(object).await?,
                        );
                    }
                }
                Ok(())
            }
            ObjectEvent::Deleted { name, namespace } => {
                let key = ObjectKey::new(namespace.clone(), name.clone());
                if let Some(sender) = self.handlers.remove(&key) {
                    debug!(
                        "Removed event handler for object {} in namespace {:?}.",
                        key.name(),
                        key.namespace()
                    );
                    sender
                        .send(ObjectEvent::Deleted { name, namespace })
                        .await?;
                }
                Ok(())
            }
        }
    }

    /// Start task for a single API object.
    // Calls `run_object_task` with first event. Monitors for object deletion
    // on subsequent events.
    async fn start_object(
        &self,
        manifest: O::Manifest,
    ) -> anyhow::Result<Sender<ObjectEvent<O::Manifest>>> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<ObjectEvent<O::Manifest>>(128);

        let deleted = Arc::new(RwLock::new(false));
        let deleted_event = Arc::new(RwLock::new(false));

        let object_state = self.operator.initialize_object_state(&manifest).await?;

        let (manifest_tx, manifest_rx) = Manifest::new(manifest, self.store.clone());
        let reflector_deleted = Arc::clone(&deleted);
        let reflector_deleted_event = Arc::clone(&deleted_event);

        // Two tasks are spawned for each resource. The first updates shared state (manifest and
        // deleted flag) while the second awaits on the actual state machine, interrupts it on
        // deletion, and handles cleanup.

        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                // Watch errors are handled before an event ever gets here, so it should always have
                // an object
                match event {
                    ObjectEvent::Applied(manifest) => {
                        trace!(
                            name=%manifest.name_any(),
                            namespace=?manifest.namespace(),
                            "Resource applied.",
                        );
                        let meta = manifest.meta();
                        if meta.deletion_timestamp.is_some() {
                            {
                                let mut event = reflector_deleted.write().await;
                                *event = true;
                            }
                        }
                        match manifest_tx.send(manifest) {
                            Ok(()) => (),
                            Err(_) => {
                                debug!("Manifest receiver hung up, exiting.");
                                return;
                            }
                        }
                    }
                    ObjectEvent::Deleted { name, namespace } => {
                        // I'm not sure if this matters, we get notified of pod deletion with a
                        // Modified event, and I think we only get this after *we* delete the pod.
                        // There is the case where someone force deletes, but we want to go through
                        // our normal terminate and deregister flow anyway.
                        debug!(
                            %name,
                            ?namespace,
                            "Resource deleted.",
                        );
                        {
                            let mut event = reflector_deleted.write().await;
                            *event = true;
                        }
                        {
                            let mut event = reflector_deleted_event.write().await;
                            *event = true;
                        }
                        break;
                    }
                }
            }
        });

        tokio::spawn(run_object_task::<O>(
            self.client.clone(),
            manifest_rx,
            self.operator.shared_state().await,
            object_state,
            deleted,
            deleted_event,
            Arc::clone(&self.operator),
        ));

        Ok(sender)
    }

    /// Resyncs the queue given the list of objects. Objects that exist in
    /// the queue but no longer exist in the list will be deleted
    #[tracing::instrument(
      level="trace",
      skip(self, objects),
      fields(count=objects.len())
    )]
    async fn resync(&mut self, objects: Vec<O::Manifest>) -> anyhow::Result<()> {
        // First reconcile any deleted items we might have missed (if it exists
        // in our map, but not in the list)
        let current_objects: HashSet<ObjectKey> = objects.iter().map(|obj| obj.into()).collect();
        let objects_in_state: HashSet<ObjectKey> = self.handlers.keys().cloned().collect();
        for key in objects_in_state.difference(&current_objects) {
            trace!(
                name=key.name(),
                namespace=?key.namespace(),
                "object_deleted"
            );
            self.dispatch(ObjectEvent::Deleted {
                name: key.name().to_string(),
                namespace: key.namespace().cloned(),
            })
            .await?;
        }

        // Now that we've sent off deletes, queue an apply event for all pods
        for object in objects.into_iter() {
            trace!(
                name=%object.name_any(),
                namespace=?object.namespace(),
                "object_applied"
            );
            self.dispatch(ObjectEvent::Applied(object)).await?
        }
        Ok(())
    }

    #[tracing::instrument(
        level="trace",
        skip(self, event),
        fields(event=?PrettyEvent::from(&event))
    )]
    pub(crate) async fn handle_event(&mut self, event: Event<O::Manifest>) {
        if let Some(ref signal) = self.signal {
            if matches!(event, kube_runtime::watcher::Event::Applied(_))
                && signal.load(Ordering::Relaxed)
            {
                warn!("Controller is shutting down (got signal). Dropping Add event.");
                return;
            }
        }
        match event {
            Event::Restarted(objects) => {
                info!("Got a watch restart. Resyncing queue...");
                // If we got a restart, we need to requeue an applied event for all objects
                match self.resync(objects).await {
                    Ok(()) => info!("Finished resync of objects."),
                    Err(error) => warn!(?error, "Error resyncing objects."),
                };
            }
            Event::Applied(object) => {
                match self.dispatch(ObjectEvent::Applied(object)).await {
                    Ok(()) => debug!("Dispatched event for processing."),
                    Err(error) => warn!(?error, "Error dispatching object event."),
                };
            }
            Event::Deleted(object) => {
                let key: ObjectKey = (&object).into();
                let event = ObjectEvent::<O::Manifest>::Deleted {
                    name: key.name().to_string(),
                    namespace: key.namespace().cloned(),
                };
                match self.dispatch(event).await {
                    Ok(()) => debug!("Dispatched event for processing."),
                    Err(error) => warn!(?error, "Error dispatching object event."),
                };
            }
        }
    }

    /// Listens for updates to objects and forwards them to queue.
    pub async fn main_loop(&mut self) {
        let api = Api::<O::Manifest>::all(self.client.clone());
        let mut informer = watcher(api, self.watcher_config.clone()).boxed();
        loop {
            match informer.try_next().await {
                Ok(Some(event)) => self.handle_event(event).await,
                Ok(None) => break,
                Err(error) => warn!(?error, "Error streaming object events."),
            }
        }
    }

    /// Start Operator (blocks forever).
    #[cfg(not(feature = "admission-webhook"))]
    pub async fn start(&mut self) {
        self.main_loop().await;
    }

    /// Start Operator (blocks forever).
    #[cfg(feature = "admission-webhook")]
    pub async fn start(&mut self) {
        let hook = crate::admission::endpoint(Arc::clone(&self.operator));
        let main = self.main_loop();
        tokio::select!(
            _ = main => warn!("Main loop exited"),
            _ = hook => warn!("Admission hook exited."),
        )
    }
}

async fn wait_event(event: Arc<RwLock<bool>>) {
    loop {
        {
            let event = event.read().await;
            if *event {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn run_object_task<O: Operator>(
    client: Client,
    manifest: Manifest<O::Manifest>,
    shared: SharedState<<O::ObjectState as ObjectState>::SharedState>,
    mut object_state: O::ObjectState,
    deleted: Arc<RwLock<bool>>,
    deleted_event: Arc<RwLock<bool>>,
    operator: Arc<O>,
) {
    debug!("Running registration hook.");
    let state: O::InitialState = Default::default();
    let (namespace, name) = {
        let m = manifest.latest();
        match operator.registration_hook(manifest.clone()).await {
            Ok(()) => debug!("Running hook complete."),
            Err(e) => {
                error!(
                    "Operator registration hook for object {} in namespace {:?} failed: {:?}",
                    m.name_any(),
                    m.namespace(),
                    e
                );
                return;
            }
        }
        (m.namespace(), m.name_any())
    };

    tokio::select! {
        _ = run_to_completion(&client, state, shared.clone(), &mut object_state, manifest.clone()) => (),
        _ = wait_event(Arc::clone(&deleted)) => {
            let state: O::DeletedState = Default::default();
            debug!("Object {} in namespace {:?} terminated. Jumping to state {:?}.", name, &namespace, state);
            run_to_completion(&client, state, shared.clone(), &mut object_state, manifest.clone()).await;
        }
    }

    debug!(
        "Resource {} in namespace {:?} waiting for deregistration.",
        name, namespace
    );
    wait_event(Arc::clone(&deleted)).await;
    {
        let mut state_writer = shared.write().await;
        object_state.async_drop(&mut state_writer).await;
    }

    match operator.deregistration_hook(manifest.clone()).await {
        Ok(()) => (),
        Err(e) => warn!(
            "Operator deregistration hook for object {} in namespace {:?} failed: {:?}",
            name, namespace, e
        ),
    }

    let api_client: Api<O::Manifest> = match namespace {
        Some(ref namespace) => kube::Api::namespaced(client, namespace),
        None => kube::Api::all(client),
    };

    let dp = kube::api::DeleteParams {
        grace_period_seconds: Some(0),
        ..Default::default()
    };

    match api_client.delete(&name, &dp).await {
        Ok(_) => {
            debug!(
                ?namespace,
                %name,
                "Object deregistered"
            );
        }
        Err(e) => match e {
            // Ignore not found, already deleted. This could happen if resource was force deleted.
            kube::error::Error::Api(kube::error::ErrorResponse { code: 404, .. }) => {
                debug!(?namespace, %name, "Object already deleted")
            }
            error => {
                warn!(
                    ?namespace,
                    %name,
                    ?error,
                    "Unable to deregister object with Kubernetes API"
                );
            }
        },
    }

    wait_event(deleted_event).await;
    debug!(?namespace, %name, "Object deleted");
}
