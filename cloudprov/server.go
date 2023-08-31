package cloudprov

import (
	"fmt"
	"net"

	log "github.com/sirupsen/logrus"
	"google.golang.org/grpc"
	"k8s.io/autoscaler/cluster-autoscaler/cloudprovider/externalgrpc/protos"
)

const (
	address = ":8086"
)

func Run(appLabel string) {
	srv := grpc.NewServer()
	logger := log.WithFields(log.Fields{"provider": providerName})

	//nolint:gosec // this is fine.jpg
	lis, err := net.Listen("tcp", address)
	if err != nil {
		logger.Fatalf("failed to listen: %s", err)
	}

	cp, err := NewCloudProvider(fmt.Sprintf("app=%s", appLabel))
	if err != nil {
		logger.Fatalf("could not create cloud provider: %s", err)
	}

	// serve
	protos.RegisterCloudProviderServer(srv, cp)
	if err := srv.Serve(lis); err != nil {
		logger.Fatalf("failed to serve: %v", err)
	}
}
