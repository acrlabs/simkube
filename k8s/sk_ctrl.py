import os

import fireconfig as fire
from constructs import Construct
from fireconfig.types import Capability
from fireconfig.types import DownwardAPIField


class SKController(fire.AppPackage):
    def __init__(self):
        env = (fire.EnvBuilder({"RUST_BACKTRACE": "1"})
            .with_field_ref("POD_SVC_ACCOUNT", DownwardAPIField.SERVICE_ACCOUNT_NAME)
        )

        try:
            with open(os.getenv('BUILD_DIR') + f'/{self.id}-image') as f:
                image = f.read()
        except FileNotFoundError:
            image = 'PLACEHOLDER'

        try:
            with open(os.getenv('BUILD_DIR') + '/sk-driver-image') as f:
                driver_image = f.read()
        except FileNotFoundError:
            driver_image = 'PLACEHOLDER'

        container = fire.ContainerBuilder(
            name=self.id,
            image=image,
            args=[
                "/sk-ctrl",
                "--driver-image", driver_image,
                "--use-cert-manager",
                "--cert-manager-issuer", "selfsigned",
            ],
        ).with_security_context(Capability.DEBUG).with_env(env)

        self._depl = (fire.DeploymentBuilder(app_label=self.id)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_node_selector("type", "kind-worker")
        )

    def compile(self, chart: Construct):
        self._depl.build(chart)

    @property
    def id(self) -> str:
        return "sk-ctrl"
