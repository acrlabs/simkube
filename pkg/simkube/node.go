package simkube

import (
	"context"
	"fmt"
	"os"

	"github.com/samber/lo"
	log "github.com/sirupsen/logrus"
	"github.com/virtual-kubelet/virtual-kubelet/node"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/client-go/kubernetes"
	"sigs.k8s.io/yaml"

	"simkube/pkg/util"
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

	nodeGroupEnvKey = "POD_OWNER"
	namespaceEnvKey = "POD_NAMESPACE"

	virtualNodeTaintKey   = "simkube.io/virtual-node"
	virtualNodeTaintValue = "true"

	nodeType              = "virtual"
	defaultArch           = "amd64"
	defaultOS             = "linux"
	defaultInstanceType   = "m6i.large"
	defaultTopologyRegion = "us-east-1"
	defaultTopologyZone   = "us-east-1a"
	defaultKubeVersion    = "v1.27.1"
)

type NodeLifecycleManagerI interface {
	CreateNodeObject(string) (*corev1.Node, error)
	Run(context.Context, context.CancelCauseFunc, *corev1.Node)
	DeleteNode(context.CancelFunc) error
}

type NodeLifecycleManager struct {
	nodeName  string
	k8sClient kubernetes.Interface
	logger    *log.Entry
}

func (self *NodeLifecycleManager) CreateNodeObject(nodeSkeletonFile string) (*corev1.Node, error) {
	node, err := parseSkeletonNode(nodeSkeletonFile)
	if err != nil {
		return nil, err
	}

	setNodeNameAndID(self.nodeName, node)
	setNodeStatus(node)
	applyStandardNodeLabelsAndTaints(node)
	configureNodeResources(node)

	if kubeVersion, err := getKubeVersion(self.k8sClient); err != nil {
		self.logger.WithError(err).Warn("could not determine Kubernetes version, using default")
		node.Status.NodeInfo.KubeletVersion = defaultKubeVersion
	} else {
		node.Status.NodeInfo.KubeletVersion = kubeVersion
	}

	return node, nil
}

func (self *NodeLifecycleManager) Run(ctx context.Context, cancel context.CancelCauseFunc, n *corev1.Node) {
	self.logger.Info("Starting node manager...")

	leaseClient := self.k8sClient.CoordinationV1().Leases(corev1.NamespaceNodeLease)
	nodeCtrl, err := node.NewNodeController(
		node.NaiveNodeProvider{},
		n,
		self.k8sClient.CoreV1().Nodes(),
		node.WithNodeEnableLeaseV1(leaseClient, 0),
	)
	if err != nil {
		cancel(fmt.Errorf("could not create node controller: %w", err))
		return
	}

	go func() {
		if err := nodeCtrl.Run(ctx); err != nil {
			cancel(fmt.Errorf("could not run node controller: %w", err))
		}
	}()
	self.logger.Info("Node manager running!")
}

func (self *NodeLifecycleManager) DeleteNode(stop context.CancelFunc) error {
	stop()
	if err := self.k8sClient.CoreV1().Nodes().Delete(
		context.Background(),
		self.nodeName,
		metav1.DeleteOptions{},
	); err != nil {
		return fmt.Errorf("delete node failed: %w", err)
	}

	return nil
}

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

func setNodeNameAndID(nodeName string, node *corev1.Node) {
	node.ObjectMeta.Name = nodeName
	node.Spec.ProviderID = util.ProviderID(nodeName)
}

func setNodeStatus(node *corev1.Node) {
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
	node.Status.Phase = corev1.NodeRunning
}

func applyStandardNodeLabelsAndTaints(node *corev1.Node) {
	defaultLabels := map[string]string{
		nodeTypeLabel:                nodeType,
		kubernetesArchLabel:          defaultArch,
		kubernetesOSLabel:            defaultOS,
		kubernetesHostnameLabel:      node.ObjectMeta.Name,
		nodeInstanceTypeLabel:        defaultInstanceType,
		topologyRegionLabel:          defaultTopologyRegion,
		topologyZoneLabel:            defaultTopologyZone,
		nodeRoleAgentLabel:           "",
		nodeRoleVirtualLabel:         "",
		util.NodeGroupNamespaceLabel: os.Getenv(namespaceEnvKey),
		util.NodeGroupNameLabel:      os.Getenv(nodeGroupEnvKey),
	}
	node.ObjectMeta.Labels = lo.Assign(defaultLabels, node.ObjectMeta.Labels)

	defaultTaints := []corev1.Taint{
		{
			Key:    virtualNodeTaintKey,
			Value:  virtualNodeTaintValue,
			Effect: corev1.TaintEffectNoExecute,
		},
	}
	if node.Spec.Taints != nil {
		node.Spec.Taints = append(node.Spec.Taints, defaultTaints...)
	} else {
		node.Spec.Taints = defaultTaints
	}
}

func configureNodeResources(node *corev1.Node) {
	defaultCapacity := map[corev1.ResourceName]resource.Quantity{
		corev1.ResourceCPU:              resource.MustParse("1"),
		corev1.ResourceMemory:           resource.MustParse("1Gi"),
		corev1.ResourceEphemeralStorage: resource.MustParse("1024Gi"),
		corev1.ResourcePods:             resource.MustParse("110"),
	}

	node.Status.Capacity = lo.Assign(defaultCapacity, node.Status.Capacity)
	node.Status.Allocatable = lo.Assign(node.Status.Capacity, node.Status.Allocatable)
}

func getKubeVersion(k8sClient kubernetes.Interface) (string, error) {
	kubeServerInfo, err := k8sClient.Discovery().ServerVersion()
	if err != nil {
		return "", fmt.Errorf("failed getting version: %w", err)
	} else {
		return kubeServerInfo.String(), nil
	}
}
