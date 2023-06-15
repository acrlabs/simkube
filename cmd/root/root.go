package root

import (
	"github.com/spf13/cobra"

	"simkube/pkg/simkube"
	"simkube/pkg/util"
)

const (
	progname = "simkube"

	verbosityFlag    = "verbosity"
	jsonLogsFlag     = "jsonlogs"
	nodeSkeletonFlag = "node-skeleton"
)

func Cmd() *cobra.Command {
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
	simkube.Run(nodeSkeletonFile)
}
