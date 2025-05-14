use std::env;

use assertables::*;
use tracing_test::traced_test;

use super::*;
use crate::objects::{
    build_driver_job,
    build_local_trace_volume,
    TRACE_VOLUME_NAME,
};

#[rstest(tokio::test)]
async fn test_build_driver_job_with_extra_args(mut test_sim: Simulation) {
    // this_is_fine.jpg
    unsafe {
        env::set_var(POD_SVC_ACCOUNT_ENV_VAR, TEST_SERVICE_ACCOUNT);
    }

    test_sim.spec.driver.args = Some(vec!["--foo".into(), "bar".into(), "--baz".into()]);
    let (_, client) = make_fake_apiserver();
    let ctx = SimulationContext::new(client, Default::default());
    let job = build_driver_job(&ctx, &test_sim, None, "secret", TEST_NAMESPACE).unwrap();

    let job_spec = job.spec.unwrap().template.spec.unwrap();
    let args = job_spec.containers.get(0).unwrap().args.as_ref().unwrap();
    let expected: Vec<&str> = vec![
        "--cert-path",
        "/usr/local/etc/ssl/tls.crt",
        "--key-path",
        "/usr/local/etc/ssl/tls.key",
        "--trace-path",
        "/foo/bar",
        "--virtual-ns-prefix",
        "virtual",
        "--sim-name",
        "",
        "--controller-ns",
        "test-namespace",
        "--foo",
        "bar",
        "--baz",
    ];
    assert_iter_eq!(args, expected);
}

#[rstest]
fn test_build_local_trace_volume_not_local(mut test_sim: Simulation) {
    test_sim.spec.driver.trace_path = "s3://foo/bar".into();
    let res = build_local_trace_volume(&test_sim).unwrap();
    assert_none!(res);
}

#[rstest]
fn test_build_local_trace_volume_skip_volume_mount(mut test_sim: Simulation) {
    test_sim
        .annotations_mut()
        .insert(SKIP_LOCAL_VOLUME_MOUNT_ANNOTATION_KEY.into(), "true".into());
    let res = build_local_trace_volume(&test_sim).unwrap();
    assert_none!(res);
}

#[rstest]
#[traced_test]
fn test_build_local_trace_volume_skip_volume_mount_not_local(mut test_sim: Simulation) {
    test_sim
        .annotations_mut()
        .insert(SKIP_LOCAL_VOLUME_MOUNT_ANNOTATION_KEY.into(), "true".into());
    test_sim.spec.driver.trace_path = "s3://foo/bar".into();
    let res = build_local_trace_volume(&test_sim).unwrap();
    assert_none!(res);
    assert!(logs_contain("ignoring annotation"));
}

#[rstest]
fn test_build_local_trace_volume(test_sim: Simulation) {
    let res = build_local_trace_volume(&test_sim).unwrap();
    let (mount, volume, path) = res.unwrap();

    assert_eq!(mount.name, TRACE_VOLUME_NAME);
    assert_eq!(mount.mount_path, "/foo/bar");
    assert_eq!(volume.name, TRACE_VOLUME_NAME);
    let host_path = volume.host_path.unwrap();
    assert_eq!(host_path.path, "/foo/bar");
    assert_eq!(host_path.type_, Some("File".into()));
    assert_eq!(path, "/foo/bar");
}
