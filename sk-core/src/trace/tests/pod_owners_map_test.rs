use std::collections::HashMap;

use super::*;
use crate::constants::*;
use crate::k8s::{
    KubeResourceId,
    PodLifecycleData,
};
use crate::trace::TraceIndex;
use crate::trace::pod_owners_map::{
    PodLifecyclesMap,
    PodOwnersMap,
    filter_lifecycles_map,
};

const START_TS: i64 = 5;
const END_TS: i64 = 10;

#[fixture]
fn owners_map() -> PodOwnersMap {
    Default::default()
}

#[fixture]
fn depl1_id() -> KubeResourceId {
    KubeResourceId::new(DEPLOYMENT_GVK.clone(), "test/deployment1".into())
}

#[fixture]
fn depl2_id() -> KubeResourceId {
    KubeResourceId::new(DEPLOYMENT_GVK.clone(), "test/deployment2".into())
}

#[rstest]
fn test_store_new_pod_lifecycle(mut owners_map: PodOwnersMap, depl1_id: KubeResourceId, depl2_id: KubeResourceId) {
    owners_map.store_new_pod_lifecycle("podA", &depl1_id, 1234, &PodLifecycleData::Running(5));
    owners_map.store_new_pod_lifecycle("podB", &depl1_id, 1234, &PodLifecycleData::Running(7));
    owners_map.store_new_pod_lifecycle("podC", &depl1_id, 5678, &PodLifecycleData::Running(9));
    owners_map.store_new_pod_lifecycle("podD", &depl2_id, 5678, &PodLifecycleData::Running(13));
    assert_eq!(
        owners_map.lifecycle_data_for(&depl1_id, 1234).unwrap(),
        &vec![PodLifecycleData::Running(5), PodLifecycleData::Running(7)]
    );
    assert_eq!(owners_map.lifecycle_data_for(&depl1_id, 5678).unwrap(), &vec![PodLifecycleData::Running(9)]);
    assert_eq!(owners_map.lifecycle_data_for(&depl2_id, 5678).unwrap(), &vec![PodLifecycleData::Running(13)]);

    assert_eq!(*owners_map.pod_owner_meta("podA").unwrap(), (depl1_id.clone(), 1234, 0));
    assert_eq!(*owners_map.pod_owner_meta("podB").unwrap(), (depl1_id.clone(), 1234, 1));
    assert_eq!(*owners_map.pod_owner_meta("podC").unwrap(), (depl1_id.clone(), 5678, 0));
    assert_eq!(*owners_map.pod_owner_meta("podD").unwrap(), (depl2_id.clone(), 5678, 0));
}

#[rstest]
fn test_filter_owners_map(depl1_id: KubeResourceId, depl2_id: KubeResourceId) {
    let mut index = TraceIndex::new();
    index.insert(&depl1_id, 9876);
    index.insert(&depl2_id, 5432);
    let owners_map = PodOwnersMap::new_from_parts(
        HashMap::from([
            (depl1_id.clone(), PodLifecyclesMap::from([(1234, vec![PodLifecycleData::Finished(1, 2)])])),
            (
                depl2_id.clone(),
                PodLifecyclesMap::from([(5678, vec![PodLifecycleData::Running(6), PodLifecycleData::Running(11)])]),
            ),
            (
                KubeResourceId::new(DEPLOYMENT_GVK.clone(), "test/deployment3".into()),
                PodLifecyclesMap::from([(9999, vec![PodLifecycleData::Finished(1, 2)])]),
            ),
        ]),
        HashMap::new(),
    );

    let res = owners_map.filter(START_TS, END_TS, &index);
    assert_eq!(
        res,
        HashMap::from([(depl2_id, PodLifecyclesMap::from([(5678, vec![PodLifecycleData::Running(6)])]),)])
    );
}

#[rstest]
fn test_filter_lifecycles_map() {
    let lifecycles_map = PodLifecyclesMap::from([(
        1234,
        vec![
            // These overlap
            PodLifecycleData::Running(6),
            PodLifecycleData::Finished(7, 9),
            PodLifecycleData::Finished(1, 8), // This one will get truncated
            PodLifecycleData::Finished(5, 10),
            // These don't
            PodLifecycleData::Running(10),
            PodLifecycleData::Running(11),
            PodLifecycleData::Finished(1, 2),
        ],
    )]);
    let expected_map = PodLifecyclesMap::from([(
        1234,
        lifecycles_map[&1234][0..4]
            .iter()
            .cloned()
            .map(|l| l.bound_start_ts(5))
            .collect(),
    )]);
    let res = filter_lifecycles_map(START_TS, END_TS, &lifecycles_map).unwrap();
    assert_eq!(res, expected_map);
}

#[rstest]
fn test_filter_lifecycles_map_empty() {
    let lifecycles_map = PodLifecyclesMap::from([(
        1234,
        vec![
            // These don't overlap
            PodLifecycleData::Running(11),
            PodLifecycleData::Finished(1, 2),
        ],
    )]);
    let res = filter_lifecycles_map(START_TS, END_TS, &lifecycles_map);
    assert_eq!(res, None);
}
