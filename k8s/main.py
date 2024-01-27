#!/usr/bin/env python
import os

import fireconfig as fire
from sk_cloudprov import SKCloudProv
from sk_ctrl import SKController
from sk_tracer import SKTracer
from sk_vnode import SKVnode
from test_deployment import TestDeployment

DAG_FILENAME = "dag.mermaid"
DIFF_FILENAME = "k8s.df"


if __name__ == "__main__":
    dag_path = f"{os.getenv('BUILD_DIR')}/{DAG_FILENAME}"
    diff_path = f"{os.getenv('BUILD_DIR')}/{DIFF_FILENAME}"
    graph, diff = fire.compile({
        "kube-system": [SKCloudProv()],
        "simkube": [
            SKVnode(),
            SKTracer(),
            SKController(),
            TestDeployment(),
        ],
    }, dag_path)

    with open(dag_path, "w") as f:
        f.write(graph)

    with open(diff_path, "w") as f:
        f.write(diff)
