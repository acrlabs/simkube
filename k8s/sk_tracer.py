import os

import fireconfig as fire
from constructs import Construct
from fireconfig.types import Capability

SERVER_PORT = 7777
TRACER_CONFIG_PATH = "tracer-config.yml"

tracer_config = open("../config/tracer-config.yml", "r", encoding="utf-8")
TRACER_CONFIG_YML = tracer_config.read()
tracer_config.close()
CONFIGMAP_NAME = "tracer-config"


class SkTracer(fire.AppPackage):
    def __init__(self, image: str, debug: bool):
        env = fire.EnvBuilder({"RUST_BACKTRACE": "1"})
        volumes = fire.VolumesBuilder().with_config_map(
            CONFIGMAP_NAME, "/config", {TRACER_CONFIG_PATH: TRACER_CONFIG_YML}
        )

        container = (
            fire.ContainerBuilder(
                name=self.id(),
                image=image,
                args=[
                    "/sk-tracer",
                    "--server-port",
                    f"{SERVER_PORT}",
                    "-c",
                    volumes.get_path_to_config_map(CONFIGMAP_NAME, TRACER_CONFIG_PATH),
                ],
            )
            .with_ports(SERVER_PORT)
            .with_volumes(volumes)
            .with_env(env)
        )
        if debug:
            container = container.with_security_context(Capability.DEBUG)

        self._depl = (
            fire.DeploymentBuilder(app_label=self.id())
            .with_service_account_and_role_binding("view", True)
            .with_containers(container)
            .with_service()
        )

        if os.getenv("KUSTOMIZE") is None:
            self._depl = self._depl.with_node_selector("type", "kind-worker")

    def compile(self, chart: Construct):
        self._depl.build(chart)  # type: ignore
