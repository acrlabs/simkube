package cli

import (
	"fmt"

	simkubev1 "simkube/lib/go/api/v1"
)

func Run() {
	filters := simkubev1.NewExportFilters(nil, nil, true)
	req := simkubev1.NewExportRequest(1, 2, *filters)
	fmt.Printf("%v\n", req)
}
