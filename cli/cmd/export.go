package cmd

import (
	"bytes"
	"fmt"
	"io"
	"io/fs"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/spf13/cobra"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	simkubev1 "simkube/lib/go/api/v1"
	"simkube/lib/go/util"
)

const (
	subcmdName = "export"

	startTimeFlag          = "start-time"
	endTimeFlag            = "end-time"
	excludedNamespacesFlag = "excluded-namespaces"
	excludedLabelsFlag     = "excluded-labels"
	outputFlag             = "output"
	tracerAddrFlag         = "tracer-addr"
)

func Export() *cobra.Command {
	export := &cobra.Command{
		Use:   subcmdName,
		Short: "export trace data",
		Run:   doExport,
	}
	export.Flags().String(
		startTimeFlag,
		"-30m",
		"start time; can be a relative duration or absolute (local) timestamp\n"+
			"    in ISO-8601 extended format (YYYY-MM-DDThh:mm:ss).\n"+
			"    durations are computed relative to the specified end time,\n"+
			"    _not_ the current time\n",
	)
	export.Flags().String(endTimeFlag, "now", "end time; can be a relative or absolute (local) timestamp\n")
	export.Flags().StringArray(
		excludedNamespacesFlag,
		[]string{"kube-system", "monitoring", "local-path-storage", "simkube", "cert-manager", "volcano-system"},
		"namespaces to exclude from the trace\n",
	)
	export.Flags().StringArray(
		excludedLabelsFlag,
		[]string{},
		"label selectors to exclude from the trace (key=value pairs)",
	)

	export.Flags().String(tracerAddrFlag, "http://localhost:7777", "tracer server address\n")
	export.Flags().StringP(outputFlag, "o", "file:///tmp/kind-node-data", "location to save exported trace\n")
	return export
}

func doExport(cmd *cobra.Command, _ []string) {
	// None of these error conditions should get hit, since they are all assigned default values?
	// I'm not sure if there's a better way to do this or not.
	startTimeStr, err := cmd.Flags().GetString(startTimeFlag)
	if err != nil {
		fmt.Printf("no start time flag: %v\n", err)
		os.Exit(1)
	}
	endTimeStr, err := cmd.Flags().GetString(endTimeFlag)
	if err != nil {
		fmt.Printf("no end time flag: %v\n", err)
		os.Exit(1)
	}
	excludedNamespaces, err := cmd.Flags().GetStringArray(excludedNamespacesFlag)
	if err != nil {
		fmt.Printf("no namespaces flag: %v\n", err)
		os.Exit(1)
	}
	tracerAddr, err := cmd.Flags().GetString(tracerAddrFlag)
	if err != nil {
		fmt.Printf("no tracer-addr flag: %v\n", err)
		os.Exit(1)
	}
	output, err := cmd.Flags().GetString(outputFlag)
	if err != nil {
		fmt.Printf("no output flag: %v\n", err)
		os.Exit(1)
	}

	// TODO actually parse excluded labels
	// excludedLabels, _ := cmd.Flags().GetStringArray(excludedLabelsFlag)

	endTime, err := util.ParseTimeStr(endTimeStr, time.Time{})
	if err != nil {
		fmt.Printf("could not parse end time: %v", err)
		os.Exit(1)
	}
	startTime, err := util.ParseTimeStr(startTimeStr, endTime)
	if err != nil {
		fmt.Printf("could not parse start time: %v", err)
		os.Exit(1)
	}

	filters := *simkubev1.NewExportFilters(
		excludedNamespaces,
		[]metav1.LabelSelector{},
		true,
	)
	request := simkubev1.NewExportRequest(startTime.Unix(), endTime.Unix(), filters)
	requestJSON, err := request.MarshalJSON()
	if err != nil {
		fmt.Printf("could not marshal request to JSON: %v\n", err)
		os.Exit(1)
	}

	requestBody := bytes.NewReader(requestJSON)

	exportUrl := fmt.Sprintf("%s/export", tracerAddr)
	fmt.Println("exporting trace data")
	fmt.Printf("start_ts = %v, end_ts = %v\n", startTime, endTime)
	fmt.Printf("using filters:\n\texcluded_namespaces: %v\n\texcluded_labels: none\n", excludedNamespaces)
	fmt.Printf("making request to %s\n", exportUrl)

	req, err := http.NewRequest(http.MethodPost, exportUrl, requestBody)
	if err != nil {
		fmt.Printf("could not create request: %v\n", err)
		os.Exit(1)
	}

	//nolint:bodyclose // this gets closed at the end of the function anyways it's fine NBD
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		fmt.Printf("error making request: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("got response status: %d\n", resp.StatusCode)
	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("could not read response body: %v\n", err)
		os.Exit(1)
	}

	if err = writeOutput(output, respBody); err != nil {
		fmt.Printf("could not write trace data to %s: %v\n", output, err)
		os.Exit(1)
	}
}

func writeOutput(output string, data []byte) error {
	if !strings.HasPrefix(output, "file://") {
		return fmt.Errorf("only local output locations supported: %s", output)
	}

	location := strings.TrimPrefix(output, "file://")
	if err := os.MkdirAll(location, fs.ModeDir|0755); err != nil {
		return fmt.Errorf("could not create location %s: %w", location, err)
	}
	fullname := fmt.Sprintf("%s/trace", location)
	out, err := os.Create(fullname)
	if err != nil {
		return fmt.Errorf("could not open %s for writing: %w", fullname, err)
	}
	defer func() {
		if err := out.Close(); err != nil {
			panic(err)
		}
	}()

	if _, err = out.Write(data); err != nil {
		return fmt.Errorf("could not write data to %s: %w", location, err)
	}
	fmt.Printf("trace successfully stored to %s\n", output)
	return nil
}
