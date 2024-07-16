mod dyn_obj_watcher;
mod pod_watcher;

pub use self::dyn_obj_watcher::{
    DynObjWatcher,
    KubeObjectStream,
};
pub use self::pod_watcher::{
    PodStream,
    PodWatcher,
};

#[cfg(test)]
mod tests;
