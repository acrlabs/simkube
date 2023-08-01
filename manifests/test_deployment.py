import fireconfig as fire
from cdk8s import Chart
from constructs import Construct


class TestDeployment(Chart):
    def __init__(self, scope: Construct, namespace: str, id: str):
        super().__init__(scope, id)

        app_key = "app"
        app_value = "nginx"

        container = fire.ContainerBuilder(
            name="nginx",
            image="nginx:latest",
        ).with_resources(requests={"cpu": "1"})

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={app_key: app_value})
            .with_containers(container)
            .with_toleration("simkube.io/virtual-node", "true")
            .with_node_selector("type", "virtual")
        )
        depl.build(self)
