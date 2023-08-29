package main

import (
	"os"

	"github.com/spf13/cobra"

	"simkube/lib/go/util"
	"simkube/vnode"
)

const (
	progname = "sk-vnode"

	verbosityFlag    = "verbosity"
	jsonLogsFlag     = "jsonlogs"
	nodeSkeletonFlag = "node-skeleton"
)

func rootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   progname,
		Short: "Run a simulated Kubernetes node",
		Run:   start,
	}

	root.PersistentFlags().IntP(verbosityFlag, "v", 2, "log level output (higher is more verbose")
	root.PersistentFlags().Bool(jsonLogsFlag, false, "structured JSON logging output")
	root.PersistentFlags().StringP(nodeSkeletonFlag, "n", "node.yml", "location of config file")
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

	nodeSkeletonFile, err := cmd.PersistentFlags().GetString(nodeSkeletonFlag)
	if err != nil {
		panic(err)
	}

	util.SetupLogging(level, jsonLogs)

	runner, err := vnode.NewRunner()
	if err != nil {
		panic(err)
	}

	runner.Run(nodeSkeletonFile)
}

func main() {
	if err := rootCmd().Execute(); err != nil {
		os.Exit(1)
	}
}
