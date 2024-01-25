import os

import fireconfig as fire
from cdk8s import Chart
from constructs import Construct
from fireconfig import k8s
from fireconfig.types import Capability
from fireconfig.types import TaintEffect

CLOUDPROV_ID = "sk-cloudprov"
AUTOSCALER_ID = "cluster-autoscaler"
APP_KEY = "app"
GRPC_PORT = 8086
CA_CONFIG_YML = """---
address: {}
"""


class SKCloudProv(Chart):
    def __init__(self, scope: Construct, namespace: str):
        super().__init__(scope, CLOUDPROV_ID, disable_resource_name_hashes=True)

        try:
            with open(os.getenv('BUILD_DIR') + f'/{CLOUDPROV_ID}-image') as f:
                image = f.read()
        except FileNotFoundError:
            image = 'PLACEHOLDER'

        container = fire.ContainerBuilder(
            name=CLOUDPROV_ID,
            image=image,
            args=["/sk-cloudprov"],
        ).with_ports(GRPC_PORT).with_security_context(Capability.DEBUG)

        self._depl = (fire.DeploymentBuilder(namespace=namespace, selector={APP_KEY: CLOUDPROV_ID})
            .with_containers(container)
            .with_service()
            .with_service_account_and_role_binding("cluster-admin", True)
            .with_node_selector("type", "kind-worker")
        )

        self._depl.build(self)

    def get_grpc_address(self) -> str:
        return f'{self._depl.get_service_address()}:{GRPC_PORT}'


class ClusterAutoscaler(Chart):
    def __init__(self, scope: Construct, cloud_prov_address: str):
        super().__init__(scope, AUTOSCALER_ID, disable_resource_name_hashes=True)

        namespace = "kube-system"

        cm = k8s.KubeConfigMap(
            self, "configmap",
            metadata={"namespace": namespace},
            data={"cluster-autoscaler-config.yml": CA_CONFIG_YML.format(cloud_prov_address)}
        )

        volumes = fire.VolumesBuilder().with_config_map("cluster-autoscaler-config", "/config", cm)
        container = fire.ContainerBuilder(
            name=AUTOSCALER_ID,
            image="localhost:5000/cluster-autoscaler:latest",
            args=[
                "/cluster-autoscaler",
                "--cloud-provider", "externalgrpc",
                "--cloud-config", volumes.get_path_to("cluster-autoscaler-config"),
                "--scale-down-delay-after-add", "1m",
                "--scale-down-unneeded-time", "1m",
                "--v", "4",
            ],
        ).with_volumes(volumes).with_security_context(Capability.DEBUG)

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={APP_KEY: AUTOSCALER_ID})
            .with_containers(container)
            .with_node_selector("type", "kind-control-plane")
            .with_toleration("node-role.kubernetes.io/control-plane", "", TaintEffect.NoSchedule)
            .with_service_account_and_role_binding("cluster-admin", True)
        )
        depl.build(self)
