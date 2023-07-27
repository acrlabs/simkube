package simkube

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	log "github.com/sirupsen/logrus"
	vklog "github.com/virtual-kubelet/virtual-kubelet/log"
	vklogrus "github.com/virtual-kubelet/virtual-kubelet/log/logrus"
	"k8s.io/client-go/kubernetes"

	"simkube/pkg/util"
)

const podNameEnv = "POD_NAME"

type Runner struct {
	nodeName  string
	k8sClient kubernetes.Interface
	nlm       NodeLifecycleManagerI
	plm       PodLifecycleManagerI
	logger    *log.Entry
}

func NewRunner() (*Runner, error) {
	nodeName := os.Getenv(podNameEnv)
	if nodeName == "" {
		return nil, errors.New("could not determine pod name")
	}

	k8sClient, err := util.NewKubernetesClient()
	if err != nil {
		return nil, fmt.Errorf("could not initialize Kubernetes client: %w", err)
	}

	logger := util.GetLogger(nodeName)
	nlm := &NodeLifecycleManager{nodeName, k8sClient, logger}
	plm := NewPodLifecycleManager(nodeName, k8sClient)

	return &Runner{nodeName, k8sClient, nlm, plm, logger}, nil
}

func (self *Runner) Run(nodeSkeletonFile string) {
	self.logger.Info("Initializing simkube controllers...")

	ctx := vklog.WithLogger(context.Background(), vklogrus.FromLogrus(self.logger))
	ctx, stop := signal.NotifyContext(ctx, syscall.SIGTERM)
	ctx, cancel := context.WithCancelCause(ctx)
	defer func() {
		// If the context was canceled by k8s, the cause is just "context.Canceled",
		// so don't report an error in this case
		if ctx.Err() == context.Canceled && context.Cause(ctx) != context.Canceled {
			self.logger.WithError(context.Cause(ctx)).Error("shutting down")
		} else {
			self.logger.Info("shutting down")
		}
		if err := self.nlm.DeleteNode(stop); err != nil {
			self.logger.WithError(err).Error("could not delete node")
		}
	}()

	n, err := self.nlm.CreateNodeObject(nodeSkeletonFile)
	if err != nil {
		self.logger.WithError(err).Error("could not create node object")
		return
	}

	self.plm.Run(ctx, cancel)
	self.nlm.Run(ctx, cancel, n)

	<-ctx.Done()
}
