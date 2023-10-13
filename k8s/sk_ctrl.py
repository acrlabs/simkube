import os

import fireconfig as fire
from cdk8s import Chart
from constructs import Construct
from fireconfig.types import Capability

ID = "sk-ctrl"


class SKController(Chart):
    def __init__(self, scope: Construct, namespace: str):
        super().__init__(scope, ID)

        app_key = "app"

        with open(os.getenv('BUILD_DIR') + f'/{ID}-image') as f:
            image = f.read()
        with open(os.getenv('BUILD_DIR') + '/sk-driver-image') as f:
            driver_image = f.read()
        container = fire.ContainerBuilder(
            name=ID,
            image=image,
            args=[
                "/sk-ctrl",
                "--driver-image", driver_image,
                # TODO can we auto-detect this in fireconfig?
                "--sim-svc-account", "sk-ctrl-service-account-c8688aad",
                "--use-cert-manager",
                "--cert-manager-issuer", "selfsigned",
            ],
        ).with_security_context(Capability.DEBUG)

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={app_key: ID})
            .with_label(app_key, ID)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_node_selector("type", "kind-worker")
        )
        depl.build(self)
