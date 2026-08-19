use std::collections::{
    BTreeMap,
    HashMap,
    HashSet,
};
use std::iter::repeat;

use assertables::*;
use sk_core::trace::PodSimData;

use super::*;
use crate::owners_index::{
    OwnersIndex,
    OwnersIndexEntry,
    bucket_pod_sim_data_by_mtime,
};

const START_TS: i64 = 5;
const END_TS: i64 = 10;

#[fixture]
fn owners_index() -> OwnersIndex {
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

fn sim_datas_from_lifecycles(lifecycles: Vec<PodLifecycleData>) -> Vec<PodSimData> {
    lifecycles.into_iter().map(|l| PodSimData::new(l)).collect()
}

#[rstest]
fn test_store_new_pod_lifecycle(mut owners_index: OwnersIndex, depl1_id: KubeResourceId, depl2_id: KubeResourceId) {
    let lifecycles1 = vec![PodLifecycleData::Running(5), PodLifecycleData::Running(7), PodLifecycleData::Running(9)];
    let lifecycles2 = vec![PodLifecycleData::Running(13)];

    for (i, (l, depl)) in lifecycles1
        .iter()
        .zip(repeat(depl1_id.clone()))
        .chain(lifecycles2.iter().zip(repeat(depl2_id.clone())))
        .enumerate()
    {
        owners_index
            .store_new_pod_lifecycle(&format!("pod{i}"), &depl, l.clone())
            .unwrap();
    }

    let expected1 = OwnersIndexEntry::new_from_parts(None, vec![], sim_datas_from_lifecycles(lifecycles1));
    let expected2 = OwnersIndexEntry::new_from_parts(None, vec![], sim_datas_from_lifecycles(lifecycles2));

    assert_some_eq_x!(owners_index.lifecycle_data_for(&depl1_id), &expected1);
    assert_some_eq_x!(owners_index.lifecycle_data_for(&depl2_id), &expected2);

    assert_some_eq_x!(owners_index.pod_index_entry("pod0"), &(depl1_id.clone(), 0));
    assert_some_eq_x!(owners_index.pod_index_entry("pod1"), &(depl1_id.clone(), 1));
    assert_some_eq_x!(owners_index.pod_index_entry("pod2"), &(depl1_id.clone(), 2));
    assert_some_eq_x!(owners_index.pod_index_entry("pod3"), &(depl2_id.clone(), 0));
}

#[rstest]
fn test_aggregate_pod_sim_data(depl1_id: KubeResourceId, depl2_id: KubeResourceId) {
    let mut all_objects = HashSet::new();
    all_objects.insert(depl1_id.clone());
    all_objects.insert(depl2_id.clone());

    let lifecycles1 = vec![
        PodLifecycleData::Finished(1, 2),
        PodLifecycleData::Finished(4, 7),
        PodLifecycleData::Finished(8, 12),
    ];
    let lifecycles2 = vec![PodLifecycleData::Running(6), PodLifecycleData::Running(11)];
    let lifecycles3 = vec![PodLifecycleData::Finished(2, 3)];
    let owners_index = OwnersIndex::new_from_parts(
        HashMap::from([
            (
                depl1_id.clone(),
                OwnersIndexEntry::new_from_parts(None, vec![1, 3], sim_datas_from_lifecycles(lifecycles1)),
            ),
            (
                depl2_id.clone(),
                OwnersIndexEntry::new_from_parts(None, vec![4], sim_datas_from_lifecycles(lifecycles2)),
            ),
            (
                KubeResourceId::new(DEPLOYMENT_GVK.clone(), "test/deployment3".into()),
                OwnersIndexEntry::new_from_parts(None, vec![2], sim_datas_from_lifecycles(lifecycles3)),
            ),
        ]),
        HashMap::new(),
    );

    let res = owners_index.aggregate_pod_sim_data(START_TS, END_TS, &all_objects);
    assert_eq!(
        res,
        HashMap::from([
            (
                depl1_id.clone(),
                BTreeMap::from([(
                    3,
                    vec![
                        // this one will get truncated
                        PodSimData::new(PodLifecycleData::Finished(5, 7)),
                        PodSimData::new(PodLifecycleData::Finished(8, 12))
                    ],
                )]),
            ),
            (depl2_id.clone(), BTreeMap::from([(4, vec![PodSimData::new(PodLifecycleData::Running(6))])]),)
        ]),
    );
}

#[rstest]
fn test_filter_lifecycles_map() {
    let entry = OwnersIndexEntry::new_from_parts(
        None,
        vec![1],
        sim_datas_from_lifecycles(vec![
            // These overlap
            PodLifecycleData::Running(6),
            PodLifecycleData::Finished(7, 9),
            PodLifecycleData::Finished(1, 8), // This one will get truncated
            PodLifecycleData::Finished(5, 10),
            // These don't
            PodLifecycleData::Running(10),
            PodLifecycleData::Running(11),
            PodLifecycleData::Finished(1, 2),
        ]),
    );
    let expected_map = BTreeMap::from([(
        1,
        entry.sim_data()[0..4]
            .iter()
            .cloned()
            .map(|l| PodSimData::new(l.lifecycle.bound_start_ts(5)))
            .collect(),
    )]);
    let res = bucket_pod_sim_data_by_mtime(START_TS, END_TS, &entry);
    assert_some_eq_x!(&res, &expected_map);
}

#[rstest]
fn test_filter_lifecycles_map_empty() {
    let entry = OwnersIndexEntry::new_from_parts(
        None,
        vec![1],
        sim_datas_from_lifecycles(vec![
            // These don't overlap
            PodLifecycleData::Running(11),
            PodLifecycleData::Finished(1, 2),
        ]),
    );
    let res = bucket_pod_sim_data_by_mtime(START_TS, END_TS, &entry);
    assert_none!(res);
}
