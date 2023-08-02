package cloudprov

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/mock"
	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/autoscaler/cluster-autoscaler/cloudprovider/externalgrpc/protos"
	"k8s.io/client-go/kubernetes/fake"

	"simkube/pkg/util"
	testutil "simkube/test/util"
)

const (
	testDeploymentLabelKey   = "app"
	testDeploymentLabelValue = "fake"
	testNodeGroupNamespace   = "testing"
	testNodeGroupName        = "simkube-node-group"
	testNodeName             = "simkube-node-group-1234"
)

//nolint:gochecknoglobals
var (
	testNodeGroupFullName = util.NamespacedName(testNodeGroupNamespace, testNodeGroupName)
	testNodeGroup         = &protos.NodeGroup{Id: testNodeGroupFullName, MinSize: 0, MaxSize: 13}
	testNodeProviderID    = util.ProviderID(testNodeName)
)

type mockScaler struct {
	mock.Mock
}

func (self *mockScaler) ScaleTo(ctx context.Context, namespace, name string, target int32) error {
	retvals := self.Called(ctx, namespace, name, target)
	return retvals.Error(0)
}

func fakeCloudProvider(scalingClient *mockScaler) *SimkubeCloudProvider {
	k8sClient := fake.NewSimpleClientset()
	replicas := int32(1)

	if _, err := k8sClient.AppsV1().Deployments(testNodeGroupNamespace).Create(
		context.TODO(),
		&appsv1.Deployment{
			ObjectMeta: metav1.ObjectMeta{
				Namespace: testNodeGroupNamespace,
				Name:      testNodeGroupName,
				Labels:    map[string]string{testDeploymentLabelKey: testDeploymentLabelValue},
			},
			Spec: appsv1.DeploymentSpec{
				Selector: &metav1.LabelSelector{MatchLabels: map[string]string{"app": "fakeNode"}},
				Replicas: &replicas,
			},
		},
		metav1.CreateOptions{},
	); err != nil {
		panic(err)
	}

	if _, err := k8sClient.CoreV1().Nodes().Create(
		context.TODO(),
		&corev1.Node{
			ObjectMeta: metav1.ObjectMeta{
				Name: testNodeName,
				Labels: map[string]string{
					util.NodeGroupNamespaceLabel: testNodeGroupNamespace,
					util.NodeGroupNameLabel:      testNodeGroupName,
				},
			},
			Spec: corev1.NodeSpec{
				ProviderID: testNodeProviderID,
			},
			Status: corev1.NodeStatus{
				Phase: corev1.NodeRunning,
			},
		},
		metav1.CreateOptions{},
	); err != nil {
		panic(err)
	}

	if _, err := k8sClient.CoreV1().Nodes().Create(
		context.TODO(),
		&corev1.Node{ObjectMeta: metav1.ObjectMeta{Name: "some-other-node"}},
		metav1.CreateOptions{},
	); err != nil {
		panic(err)
	}

	instances := []*protos.Instance{{
		Id: testNodeProviderID,
		Status: &protos.InstanceStatus{
			InstanceState: protos.InstanceStatus_instanceRunning,
		},
	}}

	return &SimkubeCloudProvider{
		k8sClient:          k8sClient,
		scalingClient:      scalingClient,
		deploymentSelector: "app=fake",
		nodeGroups: map[string]*cachedNodeGroup{
			testNodeGroupFullName: {
				data:       testNodeGroup,
				instances:  instances,
				targetSize: int32(len(instances)),
			},
		},
		logger: testutil.GetFakeLogger(),
	}
}

func TestNodeGroups(t *testing.T) {
	skprov := fakeCloudProvider(nil)

	resp, err := skprov.NodeGroups(context.TODO(), &protos.NodeGroupsRequest{})

	assert.Nil(t, err)
	assert.Len(t, resp.NodeGroups, 1)
	assert.Equal(t, testNodeGroup, resp.NodeGroups[0])
}

func TestNodeGroupForNode(t *testing.T) {
	skprov := fakeCloudProvider(nil)

	cases := map[string]struct {
		namespace string
		name      string
		expected  *protos.NodeGroup
	}{
		"missing": {
			namespace: "foo",
			name:      "bar",
		},
		"present": {
			namespace: testNodeGroupNamespace,
			name:      testNodeGroupName,
			expected:  testNodeGroup,
		},
	}

	for name, tc := range cases {
		t.Run(name, func(t *testing.T) {
			node := &protos.ExternalGrpcNode{
				ProviderID: testNodeProviderID,
				Name:       testNodeName,
				Labels: map[string]string{
					util.NodeGroupNamespaceLabel: tc.namespace,
					util.NodeGroupNameLabel:      tc.name,
				},
			}

			resp, err := skprov.NodeGroupForNode(context.TODO(), &protos.NodeGroupForNodeRequest{Node: node})

			assert.Nil(t, err) // the function doesn't error even if there's no node group
			assert.Equal(t, tc.expected, resp.NodeGroup)
		})
	}
}

func TestNodeGroupNodesMissing(t *testing.T) {
	skprov := fakeCloudProvider(nil)

	resp, err := skprov.NodeGroupNodes(context.TODO(), &protos.NodeGroupNodesRequest{Id: "foo/bar"})

	assert.ErrorIs(t, err, errorUnknownNodeGroup)
	assert.Nil(t, resp)
}

func TestNodeGroupNodes(t *testing.T) {
	skprov := fakeCloudProvider(nil)

	resp, err := skprov.NodeGroupNodes(context.TODO(), &protos.NodeGroupNodesRequest{Id: testNodeGroupFullName})

	assert.Nil(t, err)
	assert.Equal(t, skprov.nodeGroups[testNodeGroupFullName].instances, resp.Instances)
}

func TestNodeGroupTargetSizeMissing(t *testing.T) {
	skprov := fakeCloudProvider(nil)

	resp, err := skprov.NodeGroupTargetSize(context.TODO(), &protos.NodeGroupTargetSizeRequest{Id: "foo/bar"})

	assert.ErrorIs(t, err, errorUnknownNodeGroup)
	assert.Nil(t, resp)
}

func TestNodeGroupTargetSize(t *testing.T) {
	skprov := fakeCloudProvider(nil)

	resp, err := skprov.NodeGroupTargetSize(context.TODO(), &protos.NodeGroupTargetSizeRequest{Id: testNodeGroupFullName})

	assert.Nil(t, err)
	assert.Equal(t, int32(len(skprov.nodeGroups[testNodeGroupFullName].instances)), resp.TargetSize)
}

func TestNodeGroupIncreaseSize(t *testing.T) {
	scalingClient := &mockScaler{}
	scalingClient.On("ScaleTo", context.TODO(), testNodeGroupNamespace, testNodeGroupName, int32(43)).Return(nil).Once()
	skprov := fakeCloudProvider(scalingClient)

	_, err := skprov.NodeGroupIncreaseSize(
		context.TODO(),
		&protos.NodeGroupIncreaseSizeRequest{Id: testNodeGroupFullName, Delta: 42},
	)

	assert.Nil(t, err)
	scalingClient.AssertExpectations(t)
}

func TestRefresh(t *testing.T) {
	skprov := fakeCloudProvider(nil)
	skprov.nodeGroups = map[string]*cachedNodeGroup{}

	_, err := skprov.Refresh(context.TODO(), &protos.RefreshRequest{})

	assert.Nil(t, err)
	assert.Contains(t, skprov.nodeGroups, testNodeGroupFullName)

	ng := skprov.nodeGroups[testNodeGroupFullName]
	assert.Equal(t, testNodeGroupFullName, ng.data.Id)
	assert.Len(t, ng.instances, int(ng.targetSize))
	assert.Equal(t, testNodeProviderID, ng.instances[0].Id)
	assert.Equal(t, protos.InstanceStatus_instanceRunning, ng.instances[0].Status.InstanceState)
}
