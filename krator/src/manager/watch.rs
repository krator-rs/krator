use kube::{
    api::{DynamicObject, GroupVersionKind},
    Resource,
};
use kube_runtime::watcher::{Config, Event};

/// Captures configuration needed to configure a watcher.
#[derive(Clone, Debug)]
pub struct Watch {
    /// The (group, version, kind) tuple of the resource to be watched.
    pub gvk: GroupVersionKind,
    /// Optionally restrict watching to namespace.
    pub namespace: Option<String>,
    /// Restrict to objects with `watcher::Config` (default watches everything).
    pub config: Config,
}

impl Watch {
    pub fn new<
        R: Resource<DynamicType = (), Scope = kube::core::NamespaceResourceScope> + serde::de::DeserializeOwned + Clone + Send + 'static,
    >(
        namespace: Option<String>,
        config: Config,
    ) -> Self {
        let gvk = GroupVersionKind::gvk(&R::group(&()), &R::version(&()), &R::kind(&()));
        Watch {
            gvk,
            namespace,
            config,
        }
    }

    pub fn handle(
        self,
        buffer: usize,
    ) -> (
        WatchHandle,
        tokio::sync::mpsc::Receiver<Event<DynamicObject>>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);
        let handle = WatchHandle { watch: self, tx };
        (handle, rx)
    }
}

#[derive(Clone)]
pub struct WatchHandle {
    pub watch: Watch,
    pub tx: tokio::sync::mpsc::Sender<Event<DynamicObject>>,
}
