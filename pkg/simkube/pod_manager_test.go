package simkube

import (
	"context"
	"testing"

	log "github.com/sirupsen/logrus"
	"github.com/sirupsen/logrus/hooks/test"
	"github.com/stretchr/testify/assert"
	"k8s.io/client-go/kubernetes/fake"

	"simkube/test/mocks"
)

func TestPodManagerRun(t *testing.T) {
	l, _ := test.NewNullLogger()
	logger := l.WithFields(log.Fields{"test": "true"})
	plm := &PodLifecycleManager{
		nodeName:   "test-node",
		k8sClient:  fake.NewSimpleClientset(),
		podHandler: mocks.NewPodHandler(),
		logger:     logger,
	}

	ctx, cancel := context.WithCancelCause(context.TODO())
	plm.Run(ctx, cancel)

	assert.Nil(t, context.Cause(ctx))
}
