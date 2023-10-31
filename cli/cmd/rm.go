package cmd

import (
	"context"
	"fmt"
	"os"

	"github.com/spf13/cobra"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"sigs.k8s.io/controller-runtime/pkg/client"

	simkubev1 "simkube/lib/go/api/v1"
)

const rmCmdName = "rm"

func Rm(k8sClient client.Client) *cobra.Command {
	run := &cobra.Command{
		Use:   rmCmdName,
		Short: "run a simulation",
		Run:   func(cmd *cobra.Command, _ []string) { doRm(cmd, k8sClient) },
	}
	run.Flags().String(simNameFlag, "", "the name of simulation to run")
	return run
}

func doRm(cmd *cobra.Command, k8sClient client.Client) {
	// None of these error conditions should get hit, since they are all assigned default values?
	// I'm not sure if there's a better way to do this or not.
	simName, err := cmd.Flags().GetString(simNameFlag)
	if err != nil || simName == "" {
		fmt.Printf("no simulation name specified: %v\n", err)
		os.Exit(1)
	}

	sim := simkubev1.Simulation{
		ObjectMeta: metav1.ObjectMeta{Name: simName},
	}
	if err = k8sClient.Delete(context.Background(), &sim); err != nil {
		fmt.Printf("could not delete simulation: %v\n", err)
		os.Exit(1)
	}
}
