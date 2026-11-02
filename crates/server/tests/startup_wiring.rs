//! Entrypoint coverage for the wiring that dispatcher behavior tests cannot see.
//! Production once duplicated startup and never started the durable dispatcher.

#[test]
fn standalone_uses_shared_deployment_initialization() {
    let main = include_str!("../src/main.rs");
    assert!(main.contains("startup::initialize_deployment(shutdown_token.clone()).await?"));
    assert!(!main.contains("DeploymentImpl::new("));
    assert!(!main.contains("cleanup_orphan_executions()"));
}

#[test]
fn dispatcher_subscribes_before_recovery_and_reconciles_after() {
    let startup = include_str!("../src/startup.rs");
    let subscribe = startup
        .find("crate::issue_comment_delivery::spawn(&deployment)")
        .unwrap();
    let cleanup = startup.find(".cleanup_orphan_executions()").unwrap();
    let resume = startup.find(".resume_interrupted_coding_agents(").unwrap();
    let reconciled = startup.find(".mark_startup_reconciled()").unwrap();
    assert!(subscribe < cleanup);
    assert!(cleanup < resume);
    assert!(resume < reconciled);
}
