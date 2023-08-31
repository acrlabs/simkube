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
	appLabelFlag  = "applabel"
)

func rootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   progname,
		Short: "gRPC cloud provider for simkube",
		Run:   start,
	}

	root.PersistentFlags().IntP(verbosityFlag, "v", 2, "log level output (higher is more verbose")
	root.PersistentFlags().Bool(jsonLogsFlag, false, "structured JSON logging output")
	root.PersistentFlags().StringP(appLabelFlag, "A", "sk-vnode", "app label selector for virtual nodes")
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
	appLabel, err := cmd.PersistentFlags().GetString(appLabelFlag)
	if err != nil {
		panic(err)
	}
	cloudprov.Run(appLabel)
}

func main() {
	if err := rootCmd().Execute(); err != nil {
		os.Exit(1)
	}
}
