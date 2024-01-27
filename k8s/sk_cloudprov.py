import os

import fireconfig as fire
from constructs import Construct
from fireconfig.types import Capability
from fireconfig.types import TaintEffect

CLOUDPROV_ID = "sk-cloudprov"
AUTOSCALER_ID = "cluster-autoscaler"
CA_CONFIG_PATH = "config.yml"
APP_KEY = "app"
GRPC_PORT = 8086
CA_CONFIG_YML = """---
address: {}:{}
"""


def _make_cloud_provider():
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

    return (fire.DeploymentBuilder(app_label=CLOUDPROV_ID)
        .with_containers(container)
        .with_service()
        .with_service_account_and_role_binding("cluster-admin", True)
        .with_node_selector("type", "kind-worker")
    )


def _make_cluster_autoscaler(cloud_prov_addr):
    volumes = fire.VolumesBuilder().with_config_map(
        "cluster-autoscaler-config",
        "/config",
        {CA_CONFIG_PATH: CA_CONFIG_YML.format(cloud_prov_addr, GRPC_PORT)},
    )
    container = fire.ContainerBuilder(
        name=AUTOSCALER_ID,
        image="localhost:5000/cluster-autoscaler:latest",
        args=[
            "/cluster-autoscaler",
            "--cloud-provider", "externalgrpc",
            "--cloud-config", volumes.get_path_to_config_map("cluster-autoscaler-config", CA_CONFIG_PATH),
            "--scale-down-delay-after-add", "1m",
            "--scale-down-unneeded-time", "1m",
            "--v", "4",
        ],
    ).with_volumes(volumes).with_security_context(Capability.DEBUG)

    return (fire.DeploymentBuilder(app_label=AUTOSCALER_ID, tag="cluster-autoscaler")
        .with_containers(container)
        .with_node_selector("type", "kind-control-plane")
        .with_toleration("node-role.kubernetes.io/control-plane", "", TaintEffect.NoSchedule)
        .with_service_account_and_role_binding("cluster-admin", True)
    )


class SKCloudProv(fire.AppPackage):
    def __init__(self):
        self._cloud_prov = _make_cloud_provider()
        self._cluster_autoscaler = _make_cluster_autoscaler(self._cloud_prov.service_name)

    def compile(self, chart: Construct):
        self._cloud_prov.build(chart)
        self._cluster_autoscaler.build(chart)

    @property
    def id(self) -> str:
        return "sk-cloudprov"
