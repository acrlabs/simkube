package vnode

import (
	"context"
	"sync"
	"syscall"
	"testing"

	"github.com/stretchr/testify/mock"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/client-go/kubernetes/fake"

	testutil "simkube/lib/go/test/util"
)

type mockNodeLifecycleManager struct {
	mock.Mock
	wg sync.WaitGroup
}

func (self *mockNodeLifecycleManager) CreateNodeObject(nodeSkeletonFile string) (*corev1.Node, error) {
	retvals := self.Called(nodeSkeletonFile)
	return retvals.Get(0).(*corev1.Node), retvals.Error(1)
}

func (self *mockNodeLifecycleManager) Run(ctx context.Context, cancel context.CancelCauseFunc, n *corev1.Node) {
	self.Called(ctx, cancel, n)
	self.wg.Done()
}

func (self *mockNodeLifecycleManager) DeleteNode(stop context.CancelFunc) error {
	retvals := self.Called(stop)
	return retvals.Error(0)
}

type mockPodLifecycleManager struct {
	mock.Mock
}

func (self *mockPodLifecycleManager) Run(ctx context.Context, cancel context.CancelCauseFunc) {
	self.Called(ctx, cancel)
}

func TestRunInternalCleanShutdown(t *testing.T) {
	// Ensure that the main goroutine waits for the node to get cleaned up on SIGTERM
	skelFile := "skel.yml"
	n := &corev1.Node{}
	testWg := sync.WaitGroup{}
	testWg.Add(1)

	nlm := &mockNodeLifecycleManager{}
	nlm.On("CreateNodeObject", skelFile).Once().Return(n, nil)
	nlm.On("Run", mock.Anything, mock.Anything, n).Once().Return(nil)
	nlm.On("DeleteNode", mock.Anything).Once().Return(nil)
	nlm.wg.Add(1)

	plm := &mockPodLifecycleManager{}
	plm.On("Run", mock.Anything, mock.Anything).Once().Return(nil)

	runner := &Runner{"test-node", fake.NewSimpleClientset(), nlm, plm, testutil.GetFakeLogger()}

	go func() {
		runner.Run("skel.yml")
		testWg.Done()
	}()

	// We wait for the Run goroutine to start before issuing the SIGTERM
	nlm.wg.Wait()
	if err := syscall.Kill(syscall.Getpid(), syscall.SIGTERM); err != nil {
		panic(err)
	}

	// Wait for runInternal to complete; if DeleteNode doesn't get called before
	// runInternal finishes (i.e., it's not in a defer or similar), this Assert will fail
	testWg.Wait()
	nlm.AssertExpectations(t)
}
