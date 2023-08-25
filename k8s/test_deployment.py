import fireconfig as fire
from cdk8s import Chart
from constructs import Construct

ID = "test"


class TestDeployment(Chart):
    def __init__(self, scope: Construct, namespace: str):
        super().__init__(scope, ID)

        app_key = "app"

        container = fire.ContainerBuilder(
            name="nginx",
            image="nginx:latest",
        ).with_resources(requests={"cpu": "1"})

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={app_key: ID})
            .with_containers(container)
            .with_toleration("simkube.io/virtual-node", "true")
            .with_node_selector("type", "virtual")
        )
        depl.build(self)
