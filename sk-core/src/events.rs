use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use k8s_openapi::api::batch::v1 as batchv1;
use kube::Resource;
use kube::runtime::events;
use kube::runtime::events::{
    Event,
    EventType,
};
#[cfg(feature = "mock")]
use mockall::automock;

use crate::prelude::*;

#[cfg_attr(feature = "mock", automock)]
#[async_trait]
trait EventSendable {
    async fn send_event(&self, event: Event, object_ref: &corev1::ObjectReference) -> EmptyResult;
}

#[derive(Clone)]
struct EventSender {
    recorder: events::Recorder,
}

#[async_trait]
impl EventSendable for EventSender {
    async fn send_event(&self, event: Event, object_ref: &corev1::ObjectReference) -> EmptyResult {
        Ok(self.recorder.publish(&event, object_ref).await?)
    }
}

#[derive(Clone)]
pub struct SkEventRecorder {
    sender: Arc<dyn EventSendable + Send + Sync>,
    sim_object_ref: Option<corev1::ObjectReference>,
}


impl SkEventRecorder {
    pub fn new(client: kube::Client, controller: String) -> SkEventRecorder {
        let reporter = events::Reporter { controller, instance: None };
        let recorder = events::Recorder::new(client, reporter);
        SkEventRecorder {
            sender: Arc::new(EventSender { recorder }),
            sim_object_ref: None,
        }
    }

    pub fn with_sim(&self, sim: &Simulation) -> Self {
        let mut rec = self.clone();
        rec.sim_object_ref = Some(sim.object_ref(&()).clone());
        rec
    }

    pub async fn send_driver_created_event(&self, job: &batchv1::Job) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "SuccessfulDriverCreate".into(),
            note: Some(format!("Created driver job: {}", job.namespaced_name())),
            action: "SimulationPrerequisitesCreated".into(),
            secondary: None,
        })
        .await
    }

    pub async fn send_driver_pod_failed_event(&self) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "SimulationDriverFailed".into(),
            note: Some("Simulation driver pod failed".into()),
            action: "SimulationDriverRunning".into(),
            secondary: None,
        })
        .await
    }

    // exit_code will be None if the hook was terminated by a signal
    pub async fn send_hook_failed_event(
        &self,
        hook_type: &str,
        hook_cmd_str: &str,
        exit_code: Option<i32>,
    ) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Warning,
            reason: format!("{hook_type}HookFailed"),
            note: Some(format!(
                "Simulation hook `{hook_cmd_str}` failed with exit code: {}",
                exit_code.map(|c| format!("{c}")).unwrap_or("<unknown>".into())
            )),
            action: format!("{hook_type}HookExecuted"),
            secondary: None,
        })
        .await
    }

    pub async fn send_hooks_succeeded_event(&self, hook_type: &str) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: format!("{hook_type}HooksSucceeded"),
            note: Some("All hooks succeeded".into()),
            action: format!("{hook_type}HookExecuted"),
            secondary: None,
        })
        .await
    }

    pub async fn send_sim_blocked_event(&self) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "Blocked".into(),
            note: Some("Simulation is blocked from running".into()),
            action: "OtherSimulationRunning".into(),
            secondary: None,
        })
        .await
    }

    pub async fn send_sim_cleanup_failed_event(&self) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Warning,
            reason: "CleanupFailed".into(),
            note: Some("Simulation cleanup failed; there may be dangling resources".into()),
            action: "SimulationCleanupInitiated".into(),
            secondary: None,
        })
        .await
    }

    pub async fn send_sim_started_event(&self, sim_name: &str) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "SimulationStarted".into(),
            note: Some(format!("Started running simulation: {}", sim_name)),
            action: "NewSimulationResourceCreated".into(),
            secondary: None,
        })
        .await
    }

    pub async fn send_waiting_for_metrics_event(&self) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "WaitingForMetrics".into(),
            note: Some("Monitoring pod(s) are not Ready".into()),
            action: "PrometheusResourceCreated".into(),
            secondary: None,
        })
        .await
    }

    pub async fn send_waiting_for_secret_event(&self) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "WaitingForDriverSecret".into(),
            note: Some("The driver certificate secret is not Created".into()),
            action: "CertificateRequestCreated".into(),
            secondary: None,
        })
        .await
    }

    pub async fn send_waiting_for_webhook_cert_event(&self) -> EmptyResult {
        self.send_event(Event {
            type_: EventType::Normal,
            reason: "WaitingForWebhookCertBundle".into(),
            note: Some("The driver webhook certificate bundle is not injected".into()),
            action: "CertificateRequestCreated".into(),
            secondary: None,
        })
        .await
    }

    async fn send_event(&self, event: Event) -> EmptyResult {
        let Some(object_ref) = &self.sim_object_ref else {
            bail!("Simulation object reference empty; cannot send event");
        };
        self.sender.send_event(event, object_ref).await
    }
}

#[cfg(feature = "mock")]
impl SkEventRecorder {
    pub fn mock() -> SkEventRecorder {
        let mut sender = MockEventSendable::new();
        sender.expect_send_event().returning(|_, _| Ok(()));
        SkEventRecorder {
            sender: Arc::new(sender),
            sim_object_ref: Some(corev1::ObjectReference::default()),
        }
    }
}
