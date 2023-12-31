package util

import (
	"fmt"
	"runtime"
	"strings"

	log "github.com/sirupsen/logrus"
)

//nolint:gochecknoglobals
var logLevels = []log.Level{
	log.ErrorLevel,
	log.WarnLevel,
	log.InfoLevel,
}

func GetLogger(nodeName string, extraFields ...string) *log.Entry {
	fields := log.Fields{
		"provider": "simkube",
		"nodeName": nodeName,
	}

	for i := 0; i < len(extraFields); i += 2 {
		fields[extraFields[i]] = extraFields[i+1]
	}
	return log.WithFields(fields)
}

func SetupLogging(level int, jsonLogs bool) {
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
