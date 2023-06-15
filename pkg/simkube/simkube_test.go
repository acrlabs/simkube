package simkube

import (
	"context"
	"sync"
	"syscall"
	"testing"

	log "github.com/sirupsen/logrus"
	"github.com/sirupsen/logrus/hooks/test"
	"github.com/stretchr/testify/mock"
	corev1 "k8s.io/api/core/v1"
)

type mockNodeLifecycleManager struct {
	mock.Mock
	wg sync.WaitGroup
}

func (self *mockNodeLifecycleManager) CreateNodeObject(nodeSkeletonFile string) (*corev1.Node, error) {
	retvals := self.Called(nodeSkeletonFile)
	return retvals.Get(0).(*corev1.Node), retvals.Error(1)
}

func (self *mockNodeLifecycleManager) RunNode(ctx context.Context, n *corev1.Node) error {
	retvals := self.Called(ctx, n)
	self.wg.Done()
	<-ctx.Done()
	return retvals.Error(0)
}

func (self *mockNodeLifecycleManager) DeleteNode(stop context.CancelFunc) error {
	retvals := self.Called(stop)
	return retvals.Error(0)
}

func TestRunInternalCleanShutdown(t *testing.T) {
	// Ensure that the main goroutine waits for the node to get cleaned up on SIGTERM
	skelFile := "skel.yml"
	n := &corev1.Node{}
	logger, _ := test.NewNullLogger()
	testWg := sync.WaitGroup{}
	testWg.Add(1)

	nlm := &mockNodeLifecycleManager{}
	nlm.On("CreateNodeObject", skelFile).Once().Return(n, nil)
	nlm.On("RunNode", mock.Anything, n).Once().Return(nil)
	nlm.On("DeleteNode", mock.Anything).Once().Return(nil)
	nlm.wg.Add(1)

	go func() {
		runInternal("skel.yml", nlm, logger.WithFields(log.Fields{"provider": "test"}))
		testWg.Done()
	}()

	// We wait for the RunNode goroutine to start before issuing the SIGTERM
	nlm.wg.Wait()
	if err := syscall.Kill(syscall.Getpid(), syscall.SIGTERM); err != nil {
		panic(err)
	}

	// Wait for runInternal to complete; if DeleteNode doesn't get called before
	// runInternal finishes (i.e., it's not in a defer or similar), this Assert will fail
	testWg.Wait()
	nlm.AssertExpectations(t)
}
