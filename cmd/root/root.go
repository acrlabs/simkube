package root

import (
	"fmt"
	"runtime"
	"strings"

	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"

	"simkube/pkg/simkube"
)

const (
	progname = "simkube"

	verbosityFlag    = "verbosity"
	jsonLogsFlag     = "jsonlogs"
	nodeSkeletonFlag = "node-skeleton"

	osDefault = "linux"
)

var logLevels = []log.Level{
	log.ErrorLevel,
	log.WarnLevel,
	log.InfoLevel,
}

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

func setupLogging(level int, jsonLogs bool) {
	prettyfier := func(f *runtime.Frame) (string, string) {
		// Build with -trimpath to hide info about the devel environment
		// Strip off the leading package name for "pretty" output
		filename := strings.SplitN(f.File, "/", 2)[1]
		return f.Function, fmt.Sprintf("%s:%d", filename, f.Line)
	}
	if jsonLogs {
		log.SetFormatter(&log.JSONFormatter{CallerPrettyfier: prettyfier})
	} else {
		log.SetFormatter(&log.TextFormatter{
			FullTimestamp:    true,
			PadLevelText:     true,
			CallerPrettyfier: prettyfier,
		})
	}

	if level >= len(logLevels) {
		log.SetLevel(log.DebugLevel)
	} else {
		log.SetLevel(logLevels[level])
	}
	log.SetReportCaller(true)
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

	setupLogging(level, jsonLogs)
	simkube.Run(nodeSkeletonFile)
}
