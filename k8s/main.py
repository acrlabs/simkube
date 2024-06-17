#!/usr/bin/env python
import os
import argparse
import typing as T

import fireconfig as fire
from sk_ctrl import SkCtrl
from sk_tracer import SkTracer

DAG_FILENAME = "dag.mermaid"
DIFF_FILENAME = "k8s.df"
KUSTOMIZATION_YML = """
---
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - simkube.io_simulations.yml
  - 0000-global.k8s.yaml
  - 0001-sk-tracer.k8s.yaml
  - 0002-sk-ctrl.k8s.yaml
"""
QUAY_IO_PREFIX = "quay.io/appliedcomputing"


def setup_args() -> argparse.Namespace:
    root_parser = argparse.ArgumentParser(prog="k8sgen")
    root_parser.add_argument(
        "--kustomize",
        action="store_true",
    )
    return root_parser.parse_args()


def get_images(to_build: T.List, kustomize: bool, build_dir: str) -> T.List[str]:
    if kustomize:
        return [
            f"{QUAY_IO_PREFIX}/{app.id()}:v{os.getenv('APP_VERSION')}"
            for app in to_build
        ]

    images = []
    for app in to_build:
        try:
            with open(build_dir + f"/{app.id()}-image") as f:
                image = f.read()
        except FileNotFoundError:
            image = "PLACEHOLDER"
        images.append(image)

    return images


def main():
    args = setup_args()
    debug = not args.kustomize

    build_dir = os.getenv("BUILD_DIR")
    dag_path = None if args.kustomize else f"{build_dir}/{DAG_FILENAME}"
    diff_path = f"{build_dir}/{DIFF_FILENAME}"
    kustomization_path = f"{build_dir}/kustomization.yml"

    apps = [SkTracer, SkCtrl]
    images = get_images(apps, args.kustomize, build_dir)

    graph, diff = fire.compile(
        {"simkube": [app(image, debug) for app, image in zip(apps, images)]},
        dag_path,
    )

    if args.kustomize:
        with open(kustomization_path, "w") as f:
            f.write(KUSTOMIZATION_YML)
    else:
        with open(dag_path, "w") as f:
            f.write(graph)

        with open(diff_path, "w") as f:
            f.write(diff)


if __name__ == "__main__":
    main()
