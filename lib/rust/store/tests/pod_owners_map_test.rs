use super::*;

const START_TS: i64 = 5;
const END_TS: i64 = 10;

#[fixture]
fn owners_map() -> PodOwnersMap {
    Default::default()
}

#[rstest]
fn test_store_new_pod_lifecycle(mut owners_map: PodOwnersMap) {
    owners_map.store_new_pod_lifecycle("podA", "deployment1", 1234, PodLifecycleData::Running(5));
    owners_map.store_new_pod_lifecycle("podB", "deployment1", 1234, PodLifecycleData::Running(7));
    owners_map.store_new_pod_lifecycle("podC", "deployment1", 5678, PodLifecycleData::Running(9));
    owners_map.store_new_pod_lifecycle("podD", "deployment2", 5678, PodLifecycleData::Running(13));
    assert_eq!(
        owners_map.lifecycle_data_for("deployment1", &1234).unwrap(),
        vec![PodLifecycleData::Running(5), PodLifecycleData::Running(7)]
    );
    assert_eq!(owners_map.lifecycle_data_for("deployment1", &5678).unwrap(), vec![PodLifecycleData::Running(9)]);
    assert_eq!(owners_map.lifecycle_data_for("deployment2", &5678).unwrap(), vec![PodLifecycleData::Running(13)]);

    assert_eq!(*owners_map.pod_owner_meta("podA").unwrap(), ("deployment1".to_string(), 1234, 0));
    assert_eq!(*owners_map.pod_owner_meta("podB").unwrap(), ("deployment1".to_string(), 1234, 1));
    assert_eq!(*owners_map.pod_owner_meta("podC").unwrap(), ("deployment1".to_string(), 5678, 0));
    assert_eq!(*owners_map.pod_owner_meta("podD").unwrap(), ("deployment2".to_string(), 5678, 0));
}

#[rstest]
fn test_filter_owners_map() {
    let index = HashMap::from([("test/deployment1".into(), 9876), ("test/deployment2".into(), 5432)]);
    let owners_map = PodOwnersMap::new_from_parts(
        HashMap::from([
            ("test/deployment1".into(), PodLifecyclesMap::from([(1234, vec![PodLifecycleData::Finished(1, 2)])])),
            (
                "test/deployment2".into(),
                PodLifecyclesMap::from([(5678, vec![PodLifecycleData::Running(6), PodLifecycleData::Running(11)])]),
            ),
            ("test/deployment3".into(), PodLifecyclesMap::from([(9999, vec![PodLifecycleData::Finished(1, 2)])])),
        ]),
        HashMap::new(),
    );

    let res = owners_map.filter(START_TS, END_TS, &index);
    assert_eq!(
        res,
        HashMap::from([(
            "test/deployment2".into(),
            PodLifecyclesMap::from([(5678, vec![PodLifecycleData::Running(6)])]),
        )])
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
            PodLifecycleData::Finished(1, 8),
            PodLifecycleData::Finished(5, 10),
            // These don't
            PodLifecycleData::Running(10),
            PodLifecycleData::Running(11),
            PodLifecycleData::Finished(1, 2),
        ],
    )]);
    let expected_map = PodLifecyclesMap::from([(1234, lifecycles_map[&1234][0..4].into())]);
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
