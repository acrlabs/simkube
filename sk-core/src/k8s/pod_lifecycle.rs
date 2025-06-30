use std::cmp::{
    Ord,
    Ordering,
    max,
    min,
};

use tracing::*;

use super::*;
use crate::prelude::*;

// A PodLifecycleData object is how we track the length of time a pod was running in a cluster.  It
// has three states, Empty, Running, and Finished.  For each state, we track the timestamps that
// are relevant for that state, e.g., Running only has a start time, and Finished has both a start
// and end time.
//
// We compute this by tracking the earliest container start time and the latest container end time
// among all the containers in the pod (we don't want to use the pod's creation timestamp field,
// for example, because this will include time when the pod was pending and not running;
// additionally, the various pod phase statuses don't actually have a "first container started"
// status -- "Running" means that all of the containers are created, and "Pending" means that "one
// or more of the containers is not running".  So instead, we track it by hand.
//
// There's some slightly ugly code here, mostly because of annoyances in the k8s API spec.  We want
// to look at all containers, including init containers, but the initContainer field is optional,
// whereas the main container field is not.  So we have to treat these paths slightly differently.
//
// A pod can only be marked "finished" if all of the containers in the pod have terminated, OR if
// the pod has been deleted externally -- in the happy path, even if the pod is deleted externally,
// we'd still get a status update saying that the containers have terminated, but I'm not sure this
// is guaranteed to be received, or received in the correct order.  So we have two different ways
// of trying to determine this information: the `new_for` function will only return `Finished` if
// all the containers have been definitively terminated, but the `guess_finished_lifecycle` will
// just fill in the finished timestamp with `Utc::now()`.

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
            for (container, state) in cstats.iter().filter_map(|s| Some((&s.name, s.state.as_ref()?))) {
                let (start_ts, end_ts) = get_start_end_ts(pod, container, state);
                earliest_start_ts = min_some(start_ts, earliest_start_ts);
                latest_end_ts = max(latest_end_ts, end_ts);
            }
        }

        if let Some(cstats) = pod_status.container_statuses.as_ref() {
            for (container, state) in cstats.iter().filter_map(|s| Some((&s.name, s.state.as_ref()?))) {
                let (start_ts, end_ts) = get_start_end_ts(pod, container, state);
                earliest_start_ts = min_some(start_ts, earliest_start_ts);

                if end_ts.is_some() {
                    terminated_container_count += 1;
                }
                latest_end_ts = max(latest_end_ts, end_ts);
            }
        }

        // all init containers must have terminated before any of the main containers
        // start, so we don't need to additionally check the init containers here.
        //
        // TODO: I am not sure if or how this logic needs to change with the stabilization
        // of the sidecar primitive as a "non-terminating init container"
        if terminated_container_count != pod.spec()?.containers.len() {
            latest_end_ts = None;
        }
        Ok(PodLifecycleData::new(earliest_start_ts, latest_end_ts))
    }

    pub fn end_ts(&self) -> Option<i64> {
        match self {
            &PodLifecycleData::Finished(_, ts) => Some(ts),
            _ => None,
        }
    }

    pub fn start_ts(&self) -> Option<i64> {
        match *self {
            PodLifecycleData::Running(ts) => Some(ts),
            PodLifecycleData::Finished(ts, _) => Some(ts),
            _ => None,
        }
    }

    pub fn overlaps(&self, start_ts: i64, end_ts: i64) -> bool {
        // If at least one of the pod's lifecycle events appears between the given time window, OR
        // if the pod is still running at the end of the given time window, it counts as
        // overlapping the time window.
        match *self {
            PodLifecycleData::Running(ts) => ts < end_ts,
            PodLifecycleData::Finished(s, e) => (start_ts <= s && s < end_ts) || (start_ts <= e && e < end_ts),
            _ => false,
        }
    }

    pub fn guess_finished_lifecycle(
        pod: &corev1::Pod,
        current_lifecycle_data: &PodLifecycleData,
        now: i64,
    ) -> anyhow::Result<PodLifecycleData> {
        let new_lifecycle_data = PodLifecycleData::new_for(pod).unwrap_or(PodLifecycleData::Empty);

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

// We implement PartialOrd and PartialEq for PodLifecycleData; this is maybe a little bit magic,
// but it makes the code at the calling site much cleaner.  The motivation here is thus: if we've
// already received some lifecycle data, we don't want to override the data with differing data.
// An example could be, if a pod is in CrashLoopBackoff, every time we get a status update, the
// container is going to have a different start time recorded, but for the purposes of simulation,
// we want to record the _earliest_ start time we saw for the pod.
//
// With this in mind, we implemnt a partial order over PodLifecycleData, as follows:
//   - Empty < X, \forall X
//   - Running(start) < Finished(start, end), \forall Running, Finished, start, end
//   - Running(start1) <> Finished(start2, end), \forall start1 != start2
//   - Finished(start1, end1) <> Finished(start2, end2) \forall (start1 != start2 || end1 != end2)
//
// This allows us to concisely check for _valid_ updates to pod lifecycle data with an expression
// like if pld1 > pld2 { do update };  if pld1 and pld2 aren't comparable, no update will occur.
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

fn get_start_end_ts(pod: &corev1::Pod, container: &str, state: &corev1::ContainerState) -> (Option<i64>, Option<i64>) {
    let start_ts = state.start_ts().unwrap_or_else(|err| {
        warn!("could not find start_ts for container {container} in {}: {err:?}", pod.namespaced_name());
        None
    });
    let end_ts = state.end_ts().unwrap_or_else(|err| {
        warn!("could not find end_ts for container {container} in {}: {err:?}", pod.namespaced_name());
        None
    });

    (start_ts, end_ts)
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

// The default comparison between Option types returns `None` if either option is `None`, i.e.,
// `None < X \forall X`.  This is not the correct behaviour if you want to compute the minimum
// of a list of options, if it exists, and only return None if all the options are None.  That
// is what min_some does.
//
// Note the asymmetry here: we don't need a corresponding max_some because 'greater-than' works
// "correctly" for uninhabited objects.
pub fn min_some<T: Ord>(o1: Option<T>, o2: Option<T>) -> Option<T> {
    if o1.is_none() {
        o2
    } else if o2.is_none() {
        o1
    } else {
        min(o1, o2)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use sk_testutils::*;

    use super::*;

    #[rstest]
    #[case::both_none(None, None, None)]
    #[case::left_some(Some(1), None, Some(1))]
    #[case::right_some(None, Some(1), Some(1))]
    #[case::both_some(Some(2), Some(1), Some(1))]
    fn test_min_some(#[case] o1: Option<i32>, #[case] o2: Option<i32>, #[case] expected: Option<i32>) {
        assert_eq!(min_some(o1, o2), expected);
    }
}
