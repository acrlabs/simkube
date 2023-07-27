package cloudprov

import (
	"context"
	"fmt"

	log "github.com/sirupsen/logrus"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/autoscaler/cluster-autoscaler/cloudprovider/externalgrpc/protos"
	"k8s.io/client-go/kubernetes"

	"simkube/pkg/util"
)

const (
	maxNodeGroupSize = 10
)

type SimkubeCloudProvider struct {
	protos.UnimplementedCloudProviderServer

	k8sClient          kubernetes.Interface
	deploymentSelector string
	logger             *log.Entry
}

func NewCloudProvider(deploymentSelector string) (*SimkubeCloudProvider, error) {
	k8sClient, err := util.NewKubernetesClient()
	if err != nil {
		return nil, fmt.Errorf("could not initialize Kubernetes client: %w", err)
	}

	return &SimkubeCloudProvider{
		k8sClient:          k8sClient,
		deploymentSelector: deploymentSelector,
		logger:             log.WithFields(log.Fields{"provider": "sk-cloudprov"}),
	}, nil
}

func (self *SimkubeCloudProvider) NodeGroups(
	ctx context.Context,
	_ *protos.NodeGroupsRequest, // NodeGroupsRequest is empty
) (*protos.NodeGroupsResponse, error) {
	self.logger.Info("NodeGroups called")
	deployments, err := self.k8sClient.AppsV1().Deployments("").List(ctx, metav1.ListOptions{
		LabelSelector: self.deploymentSelector,
	})
	if err != nil {
		return nil, fmt.Errorf("could not fetch node groups: %w", err)
	}

	nodeGroups := make([]*protos.NodeGroup, len(deployments.Items))
	for i, d := range deployments.Items {
		nodeGroups[i] = &protos.NodeGroup{
			Id:      util.NamespacedNameFromObjectMeta(d.ObjectMeta),
			MinSize: 0,
			MaxSize: maxNodeGroupSize,
		}
	}

	self.logger.Infof("found the following node groups: %v", nodeGroups)
	return &protos.NodeGroupsResponse{NodeGroups: nodeGroups}, nil
}

func (self *SimkubeCloudProvider) NodeGroupForNode(
	ctx context.Context,
	req *protos.NodeGroupForNodeRequest,
) (*protos.NodeGroupForNodeResponse, error) {
	self.logger.Info("NodeGroupForNode called")
	return nil, nil
}

func (self *SimkubeCloudProvider) NodeGroupTargetSize(
	ctx context.Context,
	req *protos.NodeGroupTargetSizeRequest,
) (*protos.NodeGroupTargetSizeResponse, error) {
	self.logger.Info("NodeGroupTargetSize called")
	return nil, nil
}

func (self *SimkubeCloudProvider) NodeGroupIncreaseSize(
	ctx context.Context,
	req *protos.NodeGroupIncreaseSizeRequest,
) (*protos.NodeGroupIncreaseSizeResponse, error) {
	self.logger.Info("NodeGroupIncreaseSize called")
	return nil, nil
}

func (self *SimkubeCloudProvider) NodeGroupDeleteNodes(
	ctx context.Context,
	req *protos.NodeGroupDeleteNodesRequest,
) (*protos.NodeGroupDeleteNodesResponse, error) {
	self.logger.Info("NodeGroupDeleteNodes called")
	return nil, nil
}

func (self *SimkubeCloudProvider) NodeGroupDecreaseTargetSize(
	ctx context.Context,
	req *protos.NodeGroupDecreaseTargetSizeRequest,
) (*protos.NodeGroupDecreaseTargetSizeResponse, error) {
	self.logger.Info("NodeGroupDecreaseTargetSize called")
	return nil, nil
}

func (self *SimkubeCloudProvider) NodeGroupNodes(
	ctx context.Context,
	req *protos.NodeGroupNodesRequest,
) (*protos.NodeGroupNodesResponse, error) {
	self.logger.Info("NodeGroupNodes called")
	return nil, nil
}
