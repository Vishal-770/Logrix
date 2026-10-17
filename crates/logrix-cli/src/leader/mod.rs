mod k8s;

#[cfg(test)]
mod tests;

pub use k8s::KubernetesLeaseElector;
