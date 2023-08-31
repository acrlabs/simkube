import os

import fireconfig as fire
from cdk8s import Chart
from constructs import Construct
from fireconfig import k8s
from fireconfig.types import Capability
from fireconfig.types import DownwardAPIField

ID = "sk-vnode"
NODE_YML = """---
apiVersion: v1
kind: Node
status:
  allocatable:
    cpu: "1"
    memory: "1Gi"
  capacity:
    cpu: "1"
    memory: "1Gi"
"""


class SKVnode(Chart):
    def __init__(self, scope: Construct, namespace: str):
        super().__init__(scope, ID)

        app_key = "app"

        cm = k8s.KubeConfigMap(
            self, "configmap",
            metadata={"namespace": namespace},
            data={"node.yml": NODE_YML}
        )

        volumes = fire.VolumesBuilder().with_config_map("node-skeleton", "/config", cm)
        env = (fire.EnvBuilder()
            .with_field_ref("POD_NAME", DownwardAPIField.NAME)
            .with_field_ref("POD_NAMESPACE", DownwardAPIField.NAMESPACE)
        )

        with open(os.getenv('BUILD_DIR') + f'/{ID}-image') as f:
            image = f.read()
        container = fire.ContainerBuilder(
            name=ID,
            image=image,
            args=["/sk-vnode", "--node-skeleton", volumes.get_path_to("node-skeleton")],
        ).with_env(env).with_volumes(volumes).with_security_context(Capability.DEBUG)

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={app_key: ID})
            .with_label(app_key, ID)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_node_selector("type", "kind-worker")
            .with_dependencies(cm)
        )
        depl.build(self)
