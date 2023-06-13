package root

import (
	"fmt"
	"runtime"
	"strings"

	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"

	"simkube/pkg/simkube"
)

const progname = "simkube"

func Cmd() *cobra.Command {
	return &cobra.Command{
		Use:   progname,
		Short: "Run a simulated Kubernetes node",
		Run:   start,
	}
}

func start(_ *cobra.Command, _ []string) {
	log.SetFormatter(&log.JSONFormatter{
		CallerPrettyfier: func(f *runtime.Frame) (string, string) {
			// Build with -trimpath to hide info about the devel environment
			// Strip off the leading package name for "pretty" output
			filename := strings.SplitN(f.File, "/", 2)[1]
			return f.Function, fmt.Sprintf("%s:%d", filename, f.Line)
		},
	})
	log.SetLevel(log.InfoLevel)
	log.SetReportCaller(true)

	simkube.Run()
}
