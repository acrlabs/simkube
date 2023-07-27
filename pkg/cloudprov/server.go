package cloudprov

import (
	"net"

	log "github.com/sirupsen/logrus"
	"google.golang.org/grpc"
	"k8s.io/autoscaler/cluster-autoscaler/cloudprovider/externalgrpc/protos"
)

const (
	address = ":8086"
)

func Run() {
	srv := grpc.NewServer()
	logger := log.WithFields(log.Fields{"provider": "sk-cloudprov"})

	//nolint:gosec // this is fine.jpg
	lis, err := net.Listen("tcp", address)
	if err != nil {
		logger.Fatalf("failed to listen: %s", err)
	}

	cp, err := NewCloudProvider("app=simkube")
	if err != nil {
		logger.Fatalf("could not create cloud provider: %s", err)
	}

	// serve
	protos.RegisterCloudProviderServer(srv, cp)
	if err := srv.Serve(lis); err != nil {
		logger.Fatalf("failed to serve: %v", err)
	}
}
