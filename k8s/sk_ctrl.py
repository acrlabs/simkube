import fireconfig as fire
from constructs import Construct
from fireconfig.types import Capability
from fireconfig.types import DownwardAPIField

# TODO - sync these from lib/constants.rs
POD_SVC_ACCOUNT_ENV_VAR = "POD_SVC_ACCOUNT"
CTRL_NS_ENV_VAR = "CTRL_NAMESPACE"


class SkCtrl(fire.AppPackage):
    def __init__(self, image: str, debug: bool):
        env = (
            fire.EnvBuilder({"RUST_BACKTRACE": "1"})
            .with_field_ref(POD_SVC_ACCOUNT_ENV_VAR, DownwardAPIField.SERVICE_ACCOUNT_NAME)
            .with_field_ref(CTRL_NS_ENV_VAR, DownwardAPIField.NAMESPACE)
        )

        container = fire.ContainerBuilder(
            name=self.id(),
            image=image,
            args=[
                "/sk-ctrl",
                "--driver-secrets",
                "simkube",
                "--use-cert-manager",
                "--cert-manager-issuer",
                "selfsigned",
            ],
        ).with_env(env)
        if debug:
            container = container.with_security_context(Capability.DEBUG)

        self._depl = (
            fire.DeploymentBuilder(app_label=self.id())
            .with_service_account_and_role_binding("cluster-admin", True)
            .with_containers(container)
            .with_node_selector("type", "kind-worker")
        )

    def compile(self, chart: Construct):
        self._depl.build(chart)  # type: ignore
