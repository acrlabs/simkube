import os

import fireconfig as fire
from constructs import Construct
from fireconfig.types import Capability
from fireconfig.types import DownwardAPIField

NODE_YML_PATH = "node.yml"
NODE_YML = """---
apiVersion: v1
kind: Node
status:
  allocatable:
    cpu: "16"
    memory: "32Gi"
  capacity:
    cpu: "16"
    memory: "32Gi"
"""
CONFIGMAP_NAME = "node-skeleton"


class SKVnode(fire.AppPackage):
    def __init__(self):
        volumes = fire.VolumesBuilder().with_config_map(CONFIGMAP_NAME, "/config", {NODE_YML_PATH: NODE_YML})
        env = (fire.EnvBuilder()
            .with_field_ref("POD_NAME", DownwardAPIField.NAME)
            .with_field_ref("POD_NAMESPACE", DownwardAPIField.NAMESPACE)
        )

        try:
            with open(os.getenv('BUILD_DIR') + f'/{self.id}-image') as f:
                image = f.read()
        except FileNotFoundError:
            image = 'PLACEHOLDER'

        container = fire.ContainerBuilder(
            name=self.id,
            image=image,
            args=["/sk-vnode", "--node-skeleton", volumes.get_path_to_config_map(CONFIGMAP_NAME, NODE_YML_PATH)],
        ).with_env(env).with_volumes(volumes).with_security_context(Capability.DEBUG)

        self._depl = (fire.DeploymentBuilder(app_label=self.id)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_node_selector("type", "kind-worker")
        )

    def compile(self, chart: Construct):
        self._depl.build(chart)

    @property
    def id(self) -> str:
        return "sk-vnode"
