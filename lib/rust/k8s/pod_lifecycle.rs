use std::cmp::{
    max,
    Ordering,
};

use kube::ResourceExt;

use super::*;
use crate::time::Clockable;
use crate::util::min_some;

impl PodLifecycleData {
    fn new(start_ts: Option<i64>, end_ts: Option<i64>) -> PodLifecycleData {
        match (start_ts, end_ts) {
            (None, _) => PodLifecycleData::Empty,
            (Some(ts), None) => PodLifecycleData::Running(ts),
            (Some(start), Some(end)) => PodLifecycleData::Finished(start, end),
        }
    }

    pub fn new_for(pod: &corev1::Pod) -> anyhow::Result<PodLifecycleData> {
        let (mut earliest_start_ts, mut latest_end_ts) = (None, None);
        let mut terminated_container_count = 0;

        let pod_status = pod.status()?;
        if let Some(cstats) = pod_status.init_container_statuses.as_ref() {
            for state in cstats.iter().filter_map(|s| s.state.as_ref()) {
                earliest_start_ts = min_some(state.start_ts()?, earliest_start_ts);
                latest_end_ts = max(latest_end_ts, state.end_ts()?);
            }
        }

        if let Some(cstats) = pod_status.container_statuses.as_ref() {
            for state in cstats.iter().filter_map(|s| s.state.as_ref()) {
                earliest_start_ts = min_some(state.start_ts()?, earliest_start_ts);
                let end_ts = state.end_ts()?;
                if end_ts.is_some() {
                    terminated_container_count += 1;
                }
                latest_end_ts = max(latest_end_ts, end_ts);
            }
        }

        if terminated_container_count != pod.spec()?.containers.len() {
            latest_end_ts = None;
        }
        Ok(PodLifecycleData::new(earliest_start_ts, latest_end_ts))
    }

    pub fn start_ts(&self) -> Option<i64> {
        match self {
            PodLifecycleData::Empty => None,
            PodLifecycleData::Running(ts) => Some(*ts),
            PodLifecycleData::Finished(ts, _) => Some(*ts),
        }
    }

    pub fn guess_finished_lifecycle(
        pod: &corev1::Pod,
        current_lifecycle_data: &PodLifecycleData,
        clock: &(dyn Clockable + Send),
    ) -> anyhow::Result<PodLifecycleData> {
        let new_lifecycle_data = PodLifecycleData::new_for(pod).unwrap_or(PodLifecycleData::Empty);
        let now = clock.now();

        match new_lifecycle_data {
            PodLifecycleData::Finished(..) => Ok(new_lifecycle_data),
            PodLifecycleData::Running(start_ts) => Ok(PodLifecycleData::Finished(start_ts, now)),
            PodLifecycleData::Empty => {
                let start_ts = if let Some(ts) = current_lifecycle_data.start_ts() {
                    ts
                } else if let Some(t) = pod.creation_timestamp() {
                    t.0.timestamp()
                } else {
                    bail!("could not determine final pod lifecycle for {}", pod.namespaced_name());
                };
                Ok(PodLifecycleData::Finished(start_ts, now))
            },
        }
    }

    pub fn empty(&self) -> bool {
        self == PodLifecycleData::Empty
    }

    pub fn running(&self) -> bool {
        matches!(self, PodLifecycleData::Running(_))
    }

    pub fn finished(&self) -> bool {
        matches!(self, PodLifecycleData::Finished(..))
    }
}

impl PartialOrd for PodLifecycleData {
    fn partial_cmp(&self, other: &PodLifecycleData) -> Option<Ordering> {
        match self {
            PodLifecycleData::Empty => {
                if !other.empty() {
                    Some(Ordering::Less)
                } else {
                    Some(Ordering::Equal)
                }
            },
            PodLifecycleData::Running(ts) => match other {
                PodLifecycleData::Empty => Some(Ordering::Greater),
                PodLifecycleData::Running(other_ts) => {
                    if ts == other_ts {
                        Some(Ordering::Equal)
                    } else {
                        None
                    }
                },
                PodLifecycleData::Finished(..) => Some(Ordering::Less),
            },
            PodLifecycleData::Finished(sts, ets) => match other {
                PodLifecycleData::Empty => Some(Ordering::Greater),
                PodLifecycleData::Running(other_ts) => {
                    if sts == other_ts {
                        Some(Ordering::Greater)
                    } else {
                        None
                    }
                },
                PodLifecycleData::Finished(other_sts, other_ets) => {
                    if sts == other_sts && ets == other_ets {
                        Some(Ordering::Equal)
                    } else {
                        None
                    }
                },
            },
        }
    }
}


impl PartialEq<Option<&PodLifecycleData>> for PodLifecycleData {
    fn eq(&self, other: &Option<&PodLifecycleData>) -> bool {
        match self {
            PodLifecycleData::Empty => other.is_none() || other.as_ref().is_some_and(|plt| plt.empty()),
            _ => other.as_ref().is_some_and(|plt| plt == self),
        }
    }
}

impl PartialOrd<Option<&PodLifecycleData>> for PodLifecycleData {
    fn partial_cmp(&self, other: &Option<&PodLifecycleData>) -> Option<Ordering> {
        match self {
            PodLifecycleData::Empty => other.as_ref().map_or(Some(Ordering::Equal), |o| self.partial_cmp(o)),
            _ => other.as_ref().map_or(Some(Ordering::Greater), |o| self.partial_cmp(o)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partial_eq() {
        assert_eq!(PodLifecycleData::Empty, None);
        assert_eq!(PodLifecycleData::Empty, Some(&PodLifecycleData::Empty));
        assert_eq!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Running(1)));
        assert_eq!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Finished(1, 2)));

        assert_ne!(PodLifecycleData::Empty, Some(&PodLifecycleData::Running(1)));
        assert_ne!(PodLifecycleData::Empty, Some(&PodLifecycleData::Finished(1, 2)));
        assert_ne!(PodLifecycleData::Running(1), None);
        assert_ne!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Empty));
        assert_ne!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Running(2)));
        assert_ne!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Finished(1, 2)));
        assert_ne!(PodLifecycleData::Finished(1, 2), None);
        assert_ne!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Empty));
        assert_ne!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Running(2)));
        assert_ne!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Finished(1, 3)));
    }

    #[test]
    fn test_partial_ord() {
        for cmp in [
            PodLifecycleData::Empty.partial_cmp(&None),
            PodLifecycleData::Empty.partial_cmp(&Some(&PodLifecycleData::Empty)),
            PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Running(1))),
            PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Finished(1, 2))),
        ] {
            assert_eq!(cmp, Some(Ordering::Equal));
        }

        for cmp in [
            PodLifecycleData::Empty.partial_cmp(&Some(&PodLifecycleData::Running(1))),
            PodLifecycleData::Empty.partial_cmp(&Some(&PodLifecycleData::Finished(1, 2))),
            PodLifecycleData::Running(1).partial_cmp(&None),
            PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Empty)),
            PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Running(2))),
            PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Finished(1, 2))),
            PodLifecycleData::Finished(1, 2).partial_cmp(&None),
            PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Empty)),
            PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Running(2))),
            PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Finished(1, 3))),
        ] {
            assert_ne!(cmp, Some(Ordering::Equal));
        }

        assert!(PodLifecycleData::Empty < Some(&PodLifecycleData::Running(1)));
        assert!(PodLifecycleData::Empty < Some(&PodLifecycleData::Finished(1, 2)));
        assert!(PodLifecycleData::Running(1) < Some(&PodLifecycleData::Finished(1, 2)));

        assert!(PodLifecycleData::Running(1) > None);
        assert!(PodLifecycleData::Running(1) > Some(&PodLifecycleData::Empty));
        assert!(PodLifecycleData::Finished(1, 2) > None);
        assert!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Empty));
        assert!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Running(1)));

        assert!(!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Running(0))));
        assert!(!(PodLifecycleData::Finished(1, 2) < Some(&PodLifecycleData::Running(0))));
        assert!(!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Finished(1, 3))));
        assert!(!(PodLifecycleData::Finished(1, 2) < Some(&PodLifecycleData::Finished(1, 3))));
        assert!(!(PodLifecycleData::Running(1) < Some(&PodLifecycleData::Running(2))));
        assert!(!(PodLifecycleData::Running(1) > Some(&PodLifecycleData::Running(2))));
    }
}
