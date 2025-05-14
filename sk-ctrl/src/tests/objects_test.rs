use std::env;

use assertables::*;

use super::*;
use crate::objects::build_driver_job;

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
    let expected_args: Vec<&str> = vec![
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
    assert_iter_eq!(expected_args, args);
}
