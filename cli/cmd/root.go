package main

import (
	"os"

	"github.com/spf13/cobra"

	"simkube/cli"
)

const (
	progname = "skctl"

	verbosityFlag = "verbosity"
)

func rootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   progname,
		Short: "simkube CLI utility for exporting and running simulations",
		Run:   start,
	}

	root.PersistentFlags().IntP(verbosityFlag, "v", 2, "log level output (higher is more verbose)")
	return root
}

func start(cmd *cobra.Command, _ []string) {
	cli.Run()
}

func main() {
	if err := rootCmd().Execute(); err != nil {
		os.Exit(1)
	}
}
