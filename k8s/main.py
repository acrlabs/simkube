#!/usr/bin/env python
import argparse
import os
import typing as T

import fireconfig as fire

from sk_ctrl import SkCtrl
from sk_tracer import SkTracer

DAG_FILENAME = "dag.mermaid"
DIFF_FILENAME = "k8s.df"

KUSTOMIZATION_YML_BASE = """---
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - sk-namespace.yml
"""
KUSTOMIZATION_YML_PROD = """---
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - ../base
  - sk-tracer-rbac.yml
  - sk-tracer.yml
"""
KUSTOMIZATION_YML_SIM = """---
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - ../base
  - simkube.io_simulations.yml
  - sk-ctrl.yml
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
        return [f"{QUAY_IO_PREFIX}/{app.id()}:v{os.getenv('APP_VERSION')}" for app in to_build]

    images = []
    for app in to_build:
        try:
            with open(build_dir + f"/{app.id()}-image", encoding="utf-8") as f:
                image = f.read()
        except FileNotFoundError:
            image = "PLACEHOLDER"
        images.append(image)

    return images


def write_kustomize_files(build_dir: str):
    # This is all super brittle and could fail if, well, really anything changes, but I want to replace
    # this whole system at some point anyways so I'm just gonna deal with fixing it then
    os.makedirs(f"{build_dir}/base", exist_ok=True)
    os.makedirs(f"{build_dir}/prod", exist_ok=True)
    os.makedirs(f"{build_dir}/sim", exist_ok=True)

    kustomization_path_base = f"{build_dir}/base/kustomization.yml"
    kustomization_path_prod = f"{build_dir}/prod/kustomization.yml"
    kustomization_path_sim = f"{build_dir}/sim/kustomization.yml"
    with open(kustomization_path_base, "w", encoding="utf-8") as f:
        f.write(KUSTOMIZATION_YML_BASE)
    with open(kustomization_path_prod, "w", encoding="utf-8") as f:
        f.write(KUSTOMIZATION_YML_PROD)
    with open(kustomization_path_sim, "w", encoding="utf-8") as f:
        f.write(KUSTOMIZATION_YML_SIM)

    os.rename(f"{build_dir}/0000-global.k8s.yaml", f"{build_dir}/base/sk-namespace.yml")
    os.rename(f"{build_dir}/0001-sk-tracer.k8s.yaml", f"{build_dir}/prod/sk-tracer.yml")
    os.rename(f"{build_dir}/sk-tracer-rbac.yml", f"{build_dir}/prod/sk-tracer-rbac.yml")
    os.rename(f"{build_dir}/0002-sk-ctrl.k8s.yaml", f"{build_dir}/sim/sk-ctrl.yml")
    os.rename(f"{build_dir}/simkube.io_simulations.yml", f"{build_dir}/sim/simkube.io_simulations.yml")


def main():
    args = setup_args()
    debug = not args.kustomize
    if args.kustomize:
        os.environ["KUSTOMIZE"] = "1"

    build_dir = os.getenv("BUILD_DIR")
    dag_path = None if args.kustomize else f"{build_dir}/{DAG_FILENAME}"
    diff_path = f"{build_dir}/{DIFF_FILENAME}"

    apps = [SkTracer, SkCtrl]
    images = get_images(apps, args.kustomize, build_dir)

    graph, diff = fire.compile(
        {"simkube": [app(image, debug) for app, image in zip(apps, images)]},
        dag_path,
    )

    if args.kustomize:
        write_kustomize_files(build_dir)
    else:
        with open(dag_path, "w", encoding="utf-8") as f:
            f.write(graph)

        with open(diff_path, "w", encoding="utf-8") as f:
            f.write(diff)


if __name__ == "__main__":
    main()
