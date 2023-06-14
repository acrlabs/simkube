package simkube

import (
	"context"
	"os/signal"
	"syscall"

	log "github.com/sirupsen/logrus"
	"github.com/virtual-kubelet/virtual-kubelet/node"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
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

	config, err := rest.InClusterConfig()
	if err != nil {
		logger.WithError(err).Fatal("could not get client config")
	}

	client, err := kubernetes.NewForConfig(config)
	if err != nil {
		logger.WithError(err).Fatal("could not initialize Kubernetes client")
	}

	var kubeVersion string
	kubeServerInfo, err := client.Discovery().ServerVersion()
	if err != nil {
		logger.WithError(err).Error("could not determine Kubernetes version, using default")
	} else {
		kubeVersion = kubeServerInfo.String()
	}

	n, err := makeNode(nodeSkeletonFile, kubeVersion)
	if err != nil {
		logger.WithError(err).Fatal("could not create node object")
	}

	leaseClient := client.CoordinationV1().Leases(corev1.NamespaceNodeLease)

	nodeCtrl, err := node.NewNodeController(
		node.NaiveNodeProvider{},
		n,
		client.CoreV1().Nodes(),
		node.WithNodeEnableLeaseV1(leaseClient, 0),
	)
	if err != nil {
		logger.WithError(err).Fatal("could not start node controller")
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM)
	defer func() {
		logger.Info("shutting down")
		stop()
		if err := client.CoreV1().Nodes().Delete(
			context.Background(),
			n.ObjectMeta.Name,
			metav1.DeleteOptions{},
		); err != nil {
			logger.WithError(err).Fatal("could not delete node")
		}
	}()
	go func() {
		logger.Info("running node")
		if err := nodeCtrl.Run(ctx); err != nil {
			logger.WithError(err).Fatal("could not run the node")
		}
	}()

	<-ctx.Done()
}
