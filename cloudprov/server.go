package cloudprov

import (
	"fmt"
	"net"

	log "github.com/sirupsen/logrus"
	"google.golang.org/grpc"
	"k8s.io/autoscaler/cluster-autoscaler/cloudprovider/externalgrpc/protos"

	"simkube/lib/go/cloudprov"
)

const (
	address = ":8086"
)

func Run(appLabel string) {
	srv := grpc.NewServer()

	//nolint:gosec // this is fine.jpg
	lis, err := net.Listen("tcp", address)
	if err != nil {
		log.Fatalf("failed to listen: %s", err)
	}

	cp, err := cloudprov.New(fmt.Sprintf("app=%s", appLabel))
	if err != nil {
		log.Fatalf("could not create cloud provider: %s", err)
	}

	// serve
	protos.RegisterCloudProviderServer(srv, cp)
	if err := srv.Serve(lis); err != nil {
		log.Fatalf("failed to serve: %v", err)
	}
}
