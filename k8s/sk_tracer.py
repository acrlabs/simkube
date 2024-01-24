import os

import fireconfig as fire
from constructs import Construct

SERVER_PORT = 7777
TRACER_CONFIG_PATH = "tracer-config.yml"
TRACER_CONFIG_YML = """---
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePath: /spec/template
"""
CONFIGMAP_NAME = "tracer-config"


class SKTracer(fire.AppPackage):
    def __init__(self):
        env = fire.EnvBuilder({"RUST_BACKTRACE": "1"})
        volumes = (fire.VolumesBuilder()
            .with_config_map(CONFIGMAP_NAME, "/config", {TRACER_CONFIG_PATH: TRACER_CONFIG_YML})
        )

        try:
            with open(os.getenv('BUILD_DIR') + f'/{self.id}-image') as f:
                image = f.read()
        except FileNotFoundError:
            image = 'PLACEHOLDER'

        container = fire.ContainerBuilder(
            name=self.id,
            image=image,
            args=[
                "/sk-tracer",
                "--server-port", f"{SERVER_PORT}",
                "-c", volumes.get_path_to_config_map(CONFIGMAP_NAME, TRACER_CONFIG_PATH),
            ],
        ).with_ports(SERVER_PORT).with_volumes(volumes).with_env(env)

        self._depl = (fire.DeploymentBuilder(app_label=self.id)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_service()
            .with_node_selector("type", "kind-worker")
        )

    def compile(self, chart: Construct):
        self._depl.build(chart)

    @property
    def id(self) -> str:
        return "sk-tracer"
