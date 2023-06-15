package simkube

import (
	"testing"

	"github.com/stretchr/testify/assert"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

func TestApplyStandardNodeLabelsMerge(t *testing.T) {
	// ensure user preferences override defaults
	expectedArch := "arm64"
	node := &corev1.Node{
		ObjectMeta: metav1.ObjectMeta{
			Name: "test-node",
			Labels: map[string]string{
				kubernetesArchLabel: expectedArch,
			},
		},
	}

	applyStandardNodeLabels(node)
	assert.Equal(t, node.ObjectMeta.Labels[kubernetesArchLabel], expectedArch)
}
