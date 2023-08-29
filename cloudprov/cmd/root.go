package main

import (
	"os"

	"github.com/spf13/cobra"

	"simkube/cloudprov"
	"simkube/lib/go/util"
)

const (
	progname = "sk-cloudprov"

	verbosityFlag = "verbosity"
	jsonLogsFlag  = "jsonlogs"
)

func rootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   progname,
		Short: "gRPC cloud provider for simkube",
		Run:   start,
	}

	root.PersistentFlags().IntP(verbosityFlag, "v", 2, "log level output (higher is more verbose")
	root.PersistentFlags().Bool(jsonLogsFlag, false, "structured JSON logging output")
	return root
}

func start(cmd *cobra.Command, _ []string) {
	jsonLogs, err := cmd.PersistentFlags().GetBool(jsonLogsFlag)
	if err != nil {
		panic(err)
	}

	level, err := cmd.PersistentFlags().GetInt(verbosityFlag)
	if err != nil {
		panic(err)
	}

	util.SetupLogging(level, jsonLogs)
	cloudprov.Run()
}

func main() {
	if err := rootCmd().Execute(); err != nil {
		os.Exit(1)
	}
}
