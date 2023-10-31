package cmd

import (
	"github.com/spf13/cobra"
	"sigs.k8s.io/controller-runtime/pkg/client"
)

const (
	progname = "skctl"

	// Global flags
	verbosityFlag = "verbosity"

	// Subcommand flags
	endTimeFlag            = "end-time"
	excludedNamespacesFlag = "excluded-namespaces"
	excludedLabelsFlag     = "excluded-labels"
	outputFlag             = "output"
	simNameFlag            = "sim-name"
	startTimeFlag          = "start-time"
	tracerAddrFlag         = "tracer-addr"
)

func Root(k8sClient client.Client) *cobra.Command {
	root := &cobra.Command{
		Use:   progname,
		Short: "simkube CLI utility for exporting and running simulations",
	}

	root.PersistentFlags().IntP(verbosityFlag, "v", 2, "log level output (higher is more verbose)")
	root.AddCommand(Export())
	root.AddCommand(Run(k8sClient))
	root.AddCommand(Rm(k8sClient))
	return root
}
