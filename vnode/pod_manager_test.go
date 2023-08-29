package vnode

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"k8s.io/client-go/kubernetes/fake"

	testutil "simkube/lib/go/test/util"
	"simkube/vnode/mocks"
)

func TestPodManagerRun(t *testing.T) {
	plm := &PodLifecycleManager{
		nodeName:   "test-node",
		k8sClient:  fake.NewSimpleClientset(),
		podHandler: mocks.NewPodHandler(),
		logger:     testutil.GetFakeLogger(),
	}

	ctx, cancel := context.WithCancelCause(context.TODO())
	plm.Run(ctx, cancel)

	assert.Nil(t, context.Cause(ctx))
}
