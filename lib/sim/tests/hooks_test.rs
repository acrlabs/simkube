use super::*;

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_execute_hooks(test_sim: Simulation) {
    // Should print "foo"
    let res = hooks::execute(&test_sim, hooks::Type::PreStart).await;
    assert!(res.is_ok());

    // No PreStop hook defined
    let res = hooks::execute(&test_sim, hooks::Type::PostStop).await;
    assert!(res.is_ok());

    // PreRun hook calls bad command
    let res = hooks::execute(&test_sim, hooks::Type::PreRun).await;
    assert!(res.is_err());
}
