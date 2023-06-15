package simkube

import (
	"context"
	"os"
	"os/signal"
	"syscall"

	log "github.com/sirupsen/logrus"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/rest"

	"simkube/pkg/util"
)

const podNameEnv = "POD_NAME"

func Run(nodeSkeletonFile string) {
	nodeName := os.Getenv(podNameEnv)
	if nodeName == "" {
		log.Fatal("could not determine pod name")
	}

	logger := util.GetLogger(nodeName)
	logger.Info("Initializing simkube")

	config, err := rest.InClusterConfig()
	if err != nil {
		logger.WithError(err).Fatal("could not get client config")
	}

	k8sClient, err := kubernetes.NewForConfig(config)
	if err != nil {
		logger.WithError(err).Fatal("could not initialize Kubernetes client")
	}

	nlm := &NodeLifecycleManager{
		nodeName:  nodeName,
		k8sClient: k8sClient,
	}

	runInternal(nodeSkeletonFile, nlm, logger)
}

func runInternal(nodeSkeletonFile string, nlm NodeLifecycleManagerI, logger *log.Entry) {
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM)
	defer func() {
		logger.Info("shutting down")
		if err := nlm.DeleteNode(stop); err != nil {
			logger.WithError(err).Fatal("could not delete node")
		}
	}()

	n, err := nlm.CreateNodeObject(nodeSkeletonFile)
	if err != nil {
		logger.WithError(err).Fatal("could not create node object")
	}

	go func() {
		if err = nlm.RunNode(ctx, n); err != nil {
			logger.WithError(err).Fatal("could not run node")
		}
	}()

	<-ctx.Done()
}
