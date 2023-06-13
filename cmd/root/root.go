package root

import (
	"github.com/spf13/cobra"

	"simkube/pkg/simkube"
)

func Cmd() *cobra.Command {
	return &cobra.Command{
		Use:   "simkube",
		Short: "Run a simulated Kubernetes node",
		Run:   start,
	}
}

func start(_ *cobra.Command, _ []string) {
	simkube.Run()
}
