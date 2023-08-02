package cloudprov

import (
	"context"
	"errors"
	"fmt"
	"sync"

	"github.com/samber/lo"
	log "github.com/sirupsen/logrus"
	"google.golang.org/protobuf/types/known/anypb"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/autoscaler/cluster-autoscaler/cloudprovider/externalgrpc/protos"
	"k8s.io/client-go/kubernetes"

	"simkube/pkg/util"
)

const (
	maxNodeGroupSize = 10
	providerName     = "sk-cloudprov"
	podDeletionCost  = "-9999"
)

var errorUnknownNodeGroup = errors.New("unknown node group")

// In _theory_, nothing is changing the node group size aside from
// cluster autoscaler, so we can "reasonably" expect that these values
// are correct and have not been modified externally
type cachedNodeGroup struct {
	data       *protos.NodeGroup
	instances  []*protos.Instance
	targetSize int32
}

type SimkubeCloudProvider struct {
	protos.UnimplementedCloudProviderServer

	mutex sync.Mutex

	k8sClient          kubernetes.Interface
	scalingClient      scalerI
	deploymentSelector string

	nodeGroups map[string]*cachedNodeGroup
	logger     *log.Entry
}

func NewCloudProvider(deploymentSelector string) (*SimkubeCloudProvider, error) {
	k8sClient, err := util.NewKubernetesClient()
	if err != nil {
		return nil, fmt.Errorf("could not initialize Kubernetes client: %w", err)
	}

	return &SimkubeCloudProvider{
		k8sClient:          k8sClient,
		scalingClient:      &scaler{k8sClient},
		deploymentSelector: deploymentSelector,

		logger: log.WithFields(log.Fields{"provider": providerName}),
	}, nil
}

func (self *SimkubeCloudProvider) NodeGroups(
	context.Context,
	*protos.NodeGroupsRequest, // NodeGroupsRequest is empty
) (*protos.NodeGroupsResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	self.logger.Debug("NodeGroups called")

	ngs := lo.MapToSlice(
		self.nodeGroups,
		func(_ string, ng *cachedNodeGroup) *protos.NodeGroup { return ng.data },
	)
	return &protos.NodeGroupsResponse{NodeGroups: ngs}, nil
}

func (self *SimkubeCloudProvider) NodeGroupForNode(
	ctx context.Context,
	req *protos.NodeGroupForNodeRequest,
) (*protos.NodeGroupForNodeResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	self.logger.Debugf("NodeGroupForNode called with %s", req.Node.Name)

	if nodeGroupName, ok := req.Node.Labels[util.NodeGroupNameLabel]; ok {
		if nodeGroupNamespace, ok := req.Node.Labels[util.NodeGroupNamespaceLabel]; ok {
			fullName := util.NamespacedName(nodeGroupNamespace, nodeGroupName)
			if nodeGroup, ok := self.nodeGroups[fullName]; ok {
				self.logger.Infof("found node group %s for node %s", nodeGroup.data.Id, req.Node.Name)
				return &protos.NodeGroupForNodeResponse{NodeGroup: nodeGroup.data}, nil
			}
		}
	}

	self.logger.Warnf("No node group found for %s", req.Node.Name)
	return &protos.NodeGroupForNodeResponse{NodeGroup: nil}, nil
}

func (self *SimkubeCloudProvider) NodeGroupNodes(
	ctx context.Context,
	req *protos.NodeGroupNodesRequest,
) (*protos.NodeGroupNodesResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	logger := self.logger.WithFields(log.Fields{"nodeGroup": req.Id})
	logger.Debugf("NodeGroupNodes called")

	ng, ok := self.nodeGroups[req.Id]
	if !ok {
		logger.Error("could not find node group")
		return nil, errorUnknownNodeGroup
	}

	logger.Infof("nodes for node group: %v", ng.instances)
	return &protos.NodeGroupNodesResponse{Instances: ng.instances}, nil
}

func (self *SimkubeCloudProvider) NodeGroupTargetSize(
	ctx context.Context,
	req *protos.NodeGroupTargetSizeRequest,
) (*protos.NodeGroupTargetSizeResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	logger := self.logger.WithFields(log.Fields{"nodeGroup": req.Id})
	logger.Debug("NodeGroupTargetSize called")

	ng, ok := self.nodeGroups[req.Id]
	if !ok {
		logger.Error("could not find node group")
		return nil, errorUnknownNodeGroup
	}

	logger.Infof("target size for node group: %d", ng.targetSize)
	return &protos.NodeGroupTargetSizeResponse{TargetSize: ng.targetSize}, nil
}

func (self *SimkubeCloudProvider) NodeGroupIncreaseSize(
	ctx context.Context,
	req *protos.NodeGroupIncreaseSizeRequest,
) (*protos.NodeGroupIncreaseSizeResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	logger := self.logger.WithFields(log.Fields{"nodeGroup": req.Id})
	logger.Infof("NodeGroupIncreaseSize called with delta: %d", req.Delta)

	ng, ok := self.nodeGroups[req.Id]
	if !ok {
		logger.Error("could not find node group")
		return nil, errorUnknownNodeGroup
	}

	logger.Infof("increasing size: %d -> %d", ng.targetSize, ng.targetSize+req.Delta)
	namespace, name := util.SplitNamespacedName(req.Id)
	if err := self.scalingClient.ScaleTo(ctx, namespace, name, ng.targetSize+req.Delta); err != nil {
		err = fmt.Errorf("could not scale node group: %w", err)
		logger.Error(err)
		return nil, err
	}

	logger.Infof("increased target size for node group to %d", ng.targetSize)
	return &protos.NodeGroupIncreaseSizeResponse{}, nil
}

func (self *SimkubeCloudProvider) NodeGroupDeleteNodes(
	ctx context.Context,
	req *protos.NodeGroupDeleteNodesRequest,
) (*protos.NodeGroupDeleteNodesResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	nodeNames := lo.Map(req.Nodes, func(n *protos.ExternalGrpcNode, _ int) string { return n.Name })

	logger := self.logger.WithFields(log.Fields{"nodeGroup": req.Id})
	logger.Infof("NodeGroupDeleteNodes called for nodes %v", nodeNames)

	ng, ok := self.nodeGroups[req.Id]
	if !ok {
		logger.Error("could not find node group")
		return nil, errorUnknownNodeGroup
	}

	delta := int32(len(req.Nodes))
	namespace, name := util.SplitNamespacedName(req.Id)
	for _, nodeName := range nodeNames {
		podName := util.NamespacedName(namespace, nodeName)
		pod, err := self.k8sClient.CoreV1().Pods(namespace).Get(ctx, nodeName, metav1.GetOptions{})
		if err != nil {
			err = fmt.Errorf("could not get pod %s: %w", podName, err)
			logger.Error(err)
			return nil, err
		}
		if pod.ObjectMeta.Annotations == nil {
			pod.ObjectMeta.Annotations = map[string]string{}
		}
		pod.ObjectMeta.Annotations[corev1.PodDeletionCost] = podDeletionCost
		if _, err := self.k8sClient.CoreV1().Pods(namespace).Update(ctx, pod, metav1.UpdateOptions{}); err != nil {
			err = fmt.Errorf("could not update pod %s: %w", podName, err)
			logger.Error(err)
			return nil, err
		}
	}
	if err := self.scalingClient.ScaleTo(ctx, namespace, name, ng.targetSize-delta); err != nil {
		err = fmt.Errorf("could not scale node group: %w", err)
		logger.Error(err)
		return nil, err
	}

	logger.Infof("Successfully deleted nodes; new target size: %d", ng.targetSize)
	return &protos.NodeGroupDeleteNodesResponse{}, nil
}

func (self *SimkubeCloudProvider) NodeGroupDecreaseTargetSize(
	ctx context.Context,
	req *protos.NodeGroupDecreaseTargetSizeRequest,
) (*protos.NodeGroupDecreaseTargetSizeResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	logger := self.logger.WithFields(log.Fields{"nodeGroup": req.Id})
	logger.Infof("NodeGroupDecreaseTargetSize called with delta: %d", req.Delta)

	ng, ok := self.nodeGroups[req.Id]
	if !ok {
		logger.Error("could not find node group")
		return nil, errorUnknownNodeGroup
	}

	namespace, name := util.SplitNamespacedName(req.Id)
	if err := self.scalingClient.ScaleTo(ctx, namespace, name, ng.targetSize-req.Delta); err != nil {
		err = fmt.Errorf("could not scale node group: %w", err)
		logger.Error(err)
		return nil, err
	}

	logger.Infof("Successfully reduced target size to %d", ng.targetSize)
	return &protos.NodeGroupDecreaseTargetSizeResponse{}, nil
}

func (self *SimkubeCloudProvider) Refresh(
	ctx context.Context,
	req *protos.RefreshRequest,
) (*protos.RefreshResponse, error) {
	self.mutex.Lock()
	defer self.mutex.Unlock()

	self.logger.Info("Refreshing node group cache")

	deployments, err := self.k8sClient.AppsV1().Deployments("").List(ctx, metav1.ListOptions{
		LabelSelector: self.deploymentSelector,
	})
	if err != nil {
		err = fmt.Errorf("could not fetch node groups: %w", err)
		self.logger.Error(err)
		return nil, err
	}

	self.nodeGroups = make(map[string]*cachedNodeGroup, len(deployments.Items))
	for _, d := range deployments.Items {
		name := util.NamespacedNameFromObjectMeta(d.ObjectMeta)

		nodes, err := self.k8sClient.CoreV1().Nodes().List(
			ctx,
			metav1.ListOptions{LabelSelector: fmt.Sprintf(
				"%s=%s,%s=%s",
				util.NodeGroupNamespaceLabel,
				d.ObjectMeta.Namespace,
				util.NodeGroupNameLabel,
				d.ObjectMeta.Name,
			)},
		)
		if err != nil {
			err = fmt.Errorf("could not get nodes for node group: %w", err)
			self.logger.Error(err)
			return nil, err
		}

		instances := make([]*protos.Instance, len(nodes.Items))
		for i, n := range nodes.Items {
			instances[i] = &protos.Instance{
				Id:     n.Spec.ProviderID,
				Status: nodeStatusToInstanceStatus(n.Status),
			}
		}

		self.nodeGroups[name] = &cachedNodeGroup{
			data: &protos.NodeGroup{
				Id:      name,
				MinSize: 0,
				MaxSize: maxNodeGroupSize,
			},
			instances:  instances,
			targetSize: *d.Spec.Replicas,
		}
	}

	self.logger.Infof("found the following node groups: %v", self.nodeGroups)
	return &protos.RefreshResponse{}, nil
}

func (self *SimkubeCloudProvider) Cleanup(context.Context, *protos.CleanupRequest) (*protos.CleanupResponse, error) {
	self.logger.Info("Cleanup called")

	return &protos.CleanupResponse{}, nil
}

func (self *SimkubeCloudProvider) GPULabel(context.Context, *protos.GPULabelRequest) (*protos.GPULabelResponse, error) {
	self.logger.Debug("GPULabel called")

	return &protos.GPULabelResponse{Label: "simkube.io/notimplemented"}, nil
}

func (self *SimkubeCloudProvider) GetAvailableGPUTypes(
	context.Context,
	*protos.GetAvailableGPUTypesRequest,
) (*protos.GetAvailableGPUTypesResponse, error) {
	self.logger.Debug("GetAvailableGPUTypes called")

	return &protos.GetAvailableGPUTypesResponse{GpuTypes: map[string]*anypb.Any{}}, nil
}

func (self *SimkubeCloudProvider) NodeGroupGetOptions(
	_ context.Context,
	req *protos.NodeGroupAutoscalingOptionsRequest,
) (*protos.NodeGroupAutoscalingOptionsResponse, error) {
	logger := self.logger.WithFields(log.Fields{"nodeGroup": req.Id})
	logger.Debug("NodeGroupGetOptions called")

	return &protos.NodeGroupAutoscalingOptionsResponse{NodeGroupAutoscalingOptions: req.Defaults}, nil
}

func nodeStatusToInstanceStatus(s corev1.NodeStatus) *protos.InstanceStatus {
	var is protos.InstanceStatus_InstanceState
	switch s.Phase {
	case corev1.NodePending:
		is = protos.InstanceStatus_instanceCreating
	case corev1.NodeRunning:
		is = protos.InstanceStatus_instanceRunning
	case corev1.NodeTerminated:
		is = protos.InstanceStatus_instanceDeleting
	}

	return &protos.InstanceStatus{
		InstanceState: is,
		ErrorInfo:     nil,
	}
}
