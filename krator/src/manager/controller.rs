use super::watch::{Watch, WatchHandle};
#[cfg(feature = "admission-webhook")]
use crate::admission::create_boxed_endpoint;
#[cfg(feature = "admission-webhook")]
use crate::admission::AdmissionResult;
use crate::operator::Watchable;
#[cfg(feature = "admission-webhook")]
use crate::ObjectState;
use crate::Operator;
use kube::api::ListParams;
#[cfg(feature = "admission-webhook")]
use kube::Resource;
#[cfg(feature = "admission-webhook")]
use std::collections::BTreeMap;
use std::sync::Arc;
#[cfg(feature = "admission-webhook")]
use tokio::sync::RwLock;

/// Builder pattern for registering a controller or operator.
pub struct ControllerBuilder<C: Operator> {
    /// The controller or operator singleton.
    pub(crate) controller: Arc<C>,
    ///  List of watch configurations for objects that will simply be cached
    ///  locally.
    pub(crate) watches: Vec<Watch>,
    /// List of watch configurations for objects that will trigger
    /// notifications (based on OwnerReferences).
    pub(crate) owns: Vec<Watch>,
    /// Restrict our controller to act on a specific namespace.
    namespace: Option<String>,
    /// Restrict our controller to act on objects that match specific list
    /// params.
    list_params: ListParams,
    /// The buffer length for Tokio channels used to communicate between
    /// watcher tasks and runtime tasks.
    buffer: usize,
    /// Registered webhooks.
    #[cfg(feature = "admission-webhook")]
    pub(crate) webhooks:
        BTreeMap<String, warp::filters::BoxedFilter<(warp::reply::WithStatus<warp::reply::Json>,)>>,
}

// pub trait AsyncPtrFuture<O: Operator>: std::future::Future<Output=AdmissionResult<O::Manifest>> + Send + 'static {}

// pub type AsyncFnPtrReturn<O: Operator> = std::pin::Pin<Box<
//         dyn AsyncPtrFuture<O>
//     >
// >;

// pub trait AsyncFn<O: Operator>: Fn(O::Manifest, &<O::ObjectState as ObjectState>::SharedState) -> AsyncFnPtrReturn<O> {}

// pub  type AsyncFnPtr<O: Operator> = Box<
//     dyn AsyncFn<O>
// >;

impl<O: Operator> ControllerBuilder<O> {
    /// Create builder from operator singleton.
    pub fn new(operator: O) -> Self {
        ControllerBuilder {
            controller: Arc::new(operator),
            watches: vec![],
            owns: vec![],
            namespace: None,
            list_params: Default::default(),
            buffer: 32,
            #[cfg(feature = "admission-webhook")]
            webhooks: BTreeMap::new(),
        }
    }

    /// Change the length of buffer used for internal communication channels.
    pub fn with_buffer(mut self, buffer: usize) -> Self {
        self.buffer = buffer;
        self
    }

    pub(crate) fn buffer(&self) -> usize {
        self.buffer
    }

    /// Create watcher definition for the configured managed resource.
    pub(crate) fn manages(&self) -> Watch {
        Watch::new::<O::Manifest>(self.namespace.clone(), self.list_params.clone())
    }

    /// Restrict controller to manage a specific namespace.
    pub fn namespaced(mut self, namespace: &str) -> Self {
        self.namespace = Some(namespace.to_string());
        self
    }

    /// Restrict controller to manage only objects matching specific list
    /// params.
    pub fn with_params(mut self, list_params: ListParams) -> Self {
        self.list_params = list_params;
        self
    }

    /// Watch all objects of given kind R. Cluster scoped and no list param
    /// restrictions.
    pub fn watches<R>(mut self) -> Self
    where
        R: Watchable,
    {
        self.watches.push(Watch::new::<R>(None, Default::default()));
        self
    }

    /// Watch objects of given kind R. Cluster scoped, but limited to objects
    /// matching supplied list params.
    pub fn watches_with_params<R>(mut self, list_params: ListParams) -> Self
    where
        R: Watchable,
    {
        self.watches.push(Watch::new::<R>(None, list_params));
        self
    }

    /// Watch all objects of given kind R in supplied namespace, with no list
    /// param restrictions.
    pub fn watches_namespaced<R>(mut self, namespace: &str) -> Self
    where
        R: Watchable,
    {
        self.watches.push(Watch::new::<R>(
            Some(namespace.to_string()),
            Default::default(),
        ));
        self
    }

    /// Watch objects of given kind R in supplied namespace, and limited to
    /// objects matching supplied list params.
    pub fn watches_namespaced_with_params<R>(
        mut self,
        namespace: &str,
        list_params: ListParams,
    ) -> Self
    where
        R: Watchable,
    {
        self.watches
            .push(Watch::new::<R>(Some(namespace.to_string()), list_params));
        self
    }

    /// Watch and subscribe to notifications based on OwnerReferences all
    /// objects of kind R. Cluster scoped and no list param restrictions.
    pub fn owns<R>(mut self) -> Self
    where
        R: Watchable,
    {
        self.owns.push(Watch::new::<R>(None, Default::default()));
        self
    }

    /// Watch and subscribe to notifications based on OwnerReferences
    /// objects of kind R. Cluster scoped, but limited to objects matching
    /// supplied list params.
    pub fn owns_with_params<R>(mut self, list_params: ListParams) -> Self
    where
        R: Watchable,
    {
        self.owns.push(Watch::new::<R>(None, list_params));
        self
    }

    /// Watch and subscribe to notifications based on OwnerReferences
    /// objects of kind R in supplied namespace, with no list param
    /// restrictions.
    pub fn owns_namespaced<R>(mut self, namespace: &str) -> Self
    where
        R: Watchable,
    {
        self.owns.push(Watch::new::<R>(
            Some(namespace.to_string()),
            Default::default(),
        ));
        self
    }

    /// Watch and subscribe to notifications based on OwnerReferences
    /// objects of kind R in supplied namespace, and limited to objects
    /// matching supplied list params.
    pub fn owns_namespaced_with_params<R>(
        mut self,
        namespace: &str,
        list_params: ListParams,
    ) -> Self
    where
        R: Watchable,
    {
        self.owns
            .push(Watch::new::<R>(Some(namespace.to_string()), list_params));
        self
    }

    /// Registers a webhook at the path "/$GROUP/$VERSION/$KIND".
    /// Multiple webhooks can be registered, but must be at different paths.
    #[cfg(feature = "admission-webhook")]
    pub fn with_webhook<F, R>(mut self, f: F) -> Self
    where
        R: GenericFuture<O>,
        F: GenericAsyncFn<O, R>,
    {
        let path = format!(
            "/{}/{}/{}",
            O::Manifest::group(&()),
            O::Manifest::version(&()),
            O::Manifest::kind(&())
        );
        let filter = create_boxed_endpoint(Arc::clone(&self.controller), path.to_string(), f);
        self.webhooks.insert(path.to_string(), filter);
        self
    }

    /// Registers a webhook at the supplied path.
    #[cfg(feature = "admission-webhook")]
    pub fn with_webhook_at_path<F, R>(mut self, path: &str, f: F) -> Self
    where
        R: GenericFuture<O>,
        F: GenericAsyncFn<O, R>,
    {
        let filter = create_boxed_endpoint(Arc::clone(&self.controller), path.to_string(), f);
        self.webhooks.insert(path.to_string(), filter);
        self
    }
}

#[cfg(feature = "admission-webhook")]
pub trait GenericFuture<O: Operator>:
    'static + std::future::Future<Output = AdmissionResult<O::Manifest>> + Send
{
}

#[cfg(feature = "admission-webhook")]
impl<
        O: Operator,
        T: 'static + std::future::Future<Output = AdmissionResult<O::Manifest>> + Send,
    > GenericFuture<O> for T
{
}

#[cfg(feature = "admission-webhook")]
pub trait GenericAsyncFn<O: Operator, R>:
    'static
    + Clone
    + Send
    + Sync
    + Fn(O::Manifest, Arc<RwLock<<O::ObjectState as ObjectState>::SharedState>>) -> R
{
}

#[cfg(feature = "admission-webhook")]
impl<
        O: Operator,
        R,
        T: 'static
            + Clone
            + Send
            + Sync
            + Fn(O::Manifest, Arc<RwLock<<O::ObjectState as ObjectState>::SharedState>>) -> R,
    > GenericAsyncFn<O, R> for T
{
}

#[derive(Clone)]
pub struct Controller {
    pub manages: WatchHandle,
    pub owns: Vec<WatchHandle>,
    pub watches: Vec<WatchHandle>,
}
