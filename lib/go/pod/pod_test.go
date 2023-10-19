package pod

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

//nolint:gochecknoglobals
var pod = &corev1.Pod{
	ObjectMeta: metav1.ObjectMeta{
		Name:      "test-pod",
		Namespace: "testing",
	},
}

func TestCreatePod(t *testing.T) {
	podHandler := &podLifecycleHandler{"test-node", map[string]*corev1.Pod{}}
	err := podHandler.CreatePod(context.TODO(), pod)
	assert.Nil(t, err)
}

func TestUpdatePod(t *testing.T) {
	podHandler := &podLifecycleHandler{"test-node", map[string]*corev1.Pod{}}
	err := podHandler.UpdatePod(context.TODO(), pod)
	assert.Nil(t, err)
}

func TestDeletePod(t *testing.T) {
	podHandler := &podLifecycleHandler{"test-node", map[string]*corev1.Pod{}}
	err := podHandler.DeletePod(context.TODO(), pod)
	assert.Nil(t, err)
}

func TestGetPod(t *testing.T) {
	podHandler := &podLifecycleHandler{"test-node", map[string]*corev1.Pod{"testing/test-pod": {}}}
	_, err := podHandler.GetPod(context.TODO(), "testing", "test-pod")
	assert.Nil(t, err)
}

func TestGetPodStatus(t *testing.T) {
	podHandler := &podLifecycleHandler{"test-node", map[string]*corev1.Pod{"testing/test-pod": {}}}
	_, err := podHandler.GetPodStatus(context.TODO(), "testing", "test-pod")
	assert.Nil(t, err)
}

func TestGetPods(t *testing.T) {
	podHandler := &podLifecycleHandler{"test-node", map[string]*corev1.Pod{}}
	_, err := podHandler.GetPods(context.TODO())
	assert.Nil(t, err)
}
