package simkube

import (
	"context"

	log "github.com/sirupsen/logrus"
	"github.com/virtual-kubelet/virtual-kubelet/node"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/rest"
)

type SimkubeProvider struct{}

func Run(nodeSkeletonFile string) {
	logger := log.WithFields(log.Fields{
		"provider": "simkube",
		"nodeName": "foo", // TODO
	})
	logger.Info("Initializing simkube")

	n, err := makeNode(nodeSkeletonFile)
	if err != nil {
		logger.WithError(err).Fatal("could not create node object")
	}

	config, err := rest.InClusterConfig()
	if err != nil {
		logger.WithError(err).Fatal("could not get client config")
	}

	client, err := kubernetes.NewForConfig(config)
	if err != nil {
		logger.WithError(err).Fatal("could not initialize Kubernetes client")
	}

	nodeRunner, err := node.NewNodeController(
		node.NaiveNodeProvider{},
		n,
		client.CoreV1().Nodes(),
	)

	ctx := context.Background()
	if err := nodeRunner.Run(ctx); err != nil {
		logger.WithError(err).Fatal("could not run the node")
	}
	log.Info("running node")
	<-ctx.Done()
}
