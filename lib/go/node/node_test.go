package node

import (
	"testing"

	"github.com/stretchr/testify/assert"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/resource"
	"k8s.io/client-go/kubernetes/fake"

	"simkube/lib/go/testutils"
)

const (
	testSkelFile = "../testutils/manifests/skeleton-node.yml"

	expectedName = "testNode"
	expectedArch = "arm64"
	expectedOS   = "linux"

	expectedConditionCount = 4
)

//nolint:gochecknoglobals
var (
	expectedCpuCapacity    = resource.MustParse("2")
	expectedCpuAllocatable = resource.MustParse("1")
	expectedMem            = resource.MustParse("5Gi")
	expectedDisk           = resource.MustParse("1024Gi")
)

func TestCreateNodeObject(t *testing.T) {
	nlm := &LifecycleManager{expectedName, fake.NewSimpleClientset(), testutils.GetFakeLogger()}
	n, err := nlm.CreateNodeObject(testSkelFile)

	assert.Nil(t, err)
	assert.Equal(t, expectedName, n.ObjectMeta.Name)

	sv, err := nlm.k8sClient.Discovery().ServerVersion()
	if err != nil {
		panic(err)
	}
	assert.Equal(t, sv.String(), n.Status.NodeInfo.KubeletVersion)

	// arch should be user-overridden, OS should not
	assert.Equal(t, expectedArch, n.ObjectMeta.Labels[kubernetesArchLabel])
	assert.Equal(t, expectedOS, n.ObjectMeta.Labels[kubernetesOSLabel])

	// explicitly override CPU cap/allocatable
	assert.Equal(t, expectedCpuCapacity, n.Status.Capacity[corev1.ResourceCPU])
	assert.Equal(t, expectedCpuAllocatable, n.Status.Allocatable[corev1.ResourceCPU])

	// explicitly override mem cap, assert allocatable is equal
	assert.Equal(t, expectedMem, n.Status.Capacity[corev1.ResourceMemory])
	assert.Equal(t, expectedMem, n.Status.Allocatable[corev1.ResourceMemory])

	// use defaults for storage
	assert.Equal(t, expectedDisk, n.Status.Capacity[corev1.ResourceEphemeralStorage])
	assert.Equal(t, expectedDisk, n.Status.Allocatable[corev1.ResourceEphemeralStorage])

	assert.Len(t, n.Status.Conditions, expectedConditionCount)
}
