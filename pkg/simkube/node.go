package simkube

import (
	"fmt"
	"os"

	"github.com/samber/lo"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"sigs.k8s.io/yaml"
)

const (
	// Taken from "Well-known Labels, Annotations, and Taints"
	// https://kubernetes.io/docs/reference/labels-annotations-taints/
	nodeTypeLabel           = "type"
	kubernetesArchLabel     = "kubernetes.io/arch"
	kubernetesOSLabel       = "kubernetes.io/os"
	kubernetesHostnameLabel = "kubernetes.io/hostname"
	nodeInstanceTypeLabel   = "node.kubernetes.io/instance-type"
	topologyRegionLabel     = "topology.kubernetes.io/region"
	topologyZoneLabel       = "topology.kubernetes.io/zone"

	nodeRoleAgentLabel   = "node-role.kubernetes.io/agent"
	nodeRoleVirtualLabel = "node-role.kubernetes.io/virtual"

	nodeType              = "virtual"
	defaultArch           = "amd64"
	defaultOS             = "linux"
	defaultInstanceType   = "m6i.large"
	defaultTopologyRegion = "us-east-1"
	defaultTopologyZone   = "us-east-1a"
	defaultKubeVersion    = "v1.27.1"

	podNameEnv = "POD_NAME"
)

func parseSkeletonNode(nodeSkeletonFile string) (*corev1.Node, error) {
	var skel corev1.Node
	nodeBytes, err := os.ReadFile(nodeSkeletonFile)
	if err != nil {
		return nil, fmt.Errorf("could not open %s: %w", nodeSkeletonFile, err)
	}

	if err = yaml.UnmarshalStrict(nodeBytes, &skel); err != nil {
		return nil, fmt.Errorf("could not parse %s: %w", nodeSkeletonFile, err)
	}

	return &skel, nil
}

func applyStandardNodeLabelsAnnotations(node *corev1.Node) {
	defaultLabels := map[string]string{
		nodeTypeLabel:           nodeType,
		kubernetesArchLabel:     defaultArch,
		kubernetesOSLabel:       defaultOS,
		kubernetesHostnameLabel: node.ObjectMeta.Name,
		nodeInstanceTypeLabel:   defaultInstanceType,
		topologyRegionLabel:     defaultTopologyRegion,
		topologyZoneLabel:       defaultTopologyZone,
		nodeRoleAgentLabel:      "",
		nodeRoleVirtualLabel:    "",
	}
	if node.ObjectMeta.Labels == nil {
		node.ObjectMeta.Labels = make(map[string]string)
	}

	node.ObjectMeta.Labels = lo.Assign(defaultLabels, node.ObjectMeta.Labels)
}

func setNodeConditions(node *corev1.Node) {
	node.Status.Conditions = []corev1.NodeCondition{
		{
			Type:               "Ready",
			Status:             corev1.ConditionTrue,
			LastHeartbeatTime:  metav1.Now(),
			LastTransitionTime: metav1.Now(),
			Reason:             "KubeletReady",
			Message:            "kubelet is ready.",
		},
		{
			Type:               "OutOfDisk",
			Status:             corev1.ConditionFalse,
			LastHeartbeatTime:  metav1.Now(),
			LastTransitionTime: metav1.Now(),
			Reason:             "KubeletHasSufficientDisk",
			Message:            "kubelet has sufficient disk space available",
		},
		{
			Type:               "MemoryPressure",
			Status:             corev1.ConditionFalse,
			LastHeartbeatTime:  metav1.Now(),
			LastTransitionTime: metav1.Now(),
			Reason:             "KubeletHasSufficientMemory",
			Message:            "kubelet has sufficient memory available",
		},
		{
			Type:               "DiskPressure",
			Status:             corev1.ConditionFalse,
			LastHeartbeatTime:  metav1.Now(),
			LastTransitionTime: metav1.Now(),
			Reason:             "KubeletHasNoDiskPressure",
			Message:            "kubelet has no disk pressure",
		},
	}
}

func makeNode(nodeSkeletonFile, kubeVersion string) (*corev1.Node, error) {
	node, err := parseSkeletonNode(nodeSkeletonFile)
	if err != nil {
		return nil, err
	}

	nodeName := os.Getenv(podNameEnv)
	if nodeName == "" {
		return nil, fmt.Errorf("could not determine pod name")
	}

	node.ObjectMeta.Name = nodeName
	setNodeConditions(node)
	applyStandardNodeLabelsAnnotations(node)

	if kubeVersion == "" {
		node.Status.NodeInfo.KubeletVersion = defaultKubeVersion
	} else {
		node.Status.NodeInfo.KubeletVersion = kubeVersion
	}

	return node, nil
}
