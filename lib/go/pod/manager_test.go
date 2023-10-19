package pod

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"k8s.io/client-go/kubernetes/fake"

	"simkube/lib/go/testutils"
)

func TestPodManagerRun(t *testing.T) {
	plm := &LifecycleManager{
		nodeName:   "test-node",
		k8sClient:  fake.NewSimpleClientset(),
		podHandler: testutils.NewPodHandler(),
		logger:     testutils.GetFakeLogger(),
	}

	ctx, cancel := context.WithCancelCause(context.TODO())
	plm.Run(ctx, cancel)

	assert.Nil(t, context.Cause(ctx))
}
