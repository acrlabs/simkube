package cmd

import (
	"github.com/spf13/cobra"
)

const (
	progname = "skctl"

	verbosityFlag = "verbosity"
)

func Root() *cobra.Command {
	root := &cobra.Command{
		Use:   progname,
		Short: "simkube CLI utility for exporting and running simulations",
	}

	root.PersistentFlags().IntP(verbosityFlag, "v", 2, "log level output (higher is more verbose)")
	root.AddCommand(Export())
	return root
}
