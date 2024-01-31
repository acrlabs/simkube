import fireconfig as fire
from constructs import Construct
from fireconfig.types import TaintEffect


class TestDeployment(fire.AppPackage):
    def __init__(self):
        container = fire.ContainerBuilder(
            name="nginx",
            image="nginx:latest",
        ).with_resources(requests={"cpu": "1"})

        self._depl = (fire.DeploymentBuilder(app_label=self.id)
            .with_containers(container)
            .with_toleration("kwok-provider", "true", TaintEffect.NoSchedule)
            .with_node_selector("type", "virtual")
        )

    def compile(self, chart: Construct):
        self._depl.build(chart)

    @property
    def id(self) -> str:
        return "test"
