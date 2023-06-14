package main

import (
	"os"

	"simkube/cmd/root"
)

func main() {
	if err := root.Cmd().Execute(); err != nil {
		os.Exit(1)
	}
}
