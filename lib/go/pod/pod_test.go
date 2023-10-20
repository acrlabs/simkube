package pod

import (
	"context"
	"fmt"
	"testing"
	"time"

	"github.com/jonboulle/clockwork"
	"github.com/samber/lo"
	"github.com/stretchr/testify/assert"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

const (
	testNamespace     = "test"
	testPodName       = "the-pod"
	testContainerName = "the-container"
	testNodeName      = "test-node"
)

//nolint:gochecknoglobals
var (
	testContainer   = corev1.Container{Name: testContainerName}
	testPodFullName = fmt.Sprintf("%s/%s", testNamespace, testPodName)
	testEndTime     = time.Date(1, time.January, 1, 0, 0, 5, 0, time.UTC)
)

func makePodLifecycleHandler(opts ...func(*podLifecycleHandler)) *podLifecycleHandler {
	handler := &podLifecycleHandler{
		testNodeName,
		map[string]*corev1.Pod{},
		map[string]time.Time{},
		clockwork.NewFakeClock(),
	}
	for _, opt := range opts {
		opt(handler)
	}
	return handler
}

func withPod(h *podLifecycleHandler) {
	pod := makePod(nil, []corev1.Container{testContainer}, nil)
	pod.Status = corev1.PodStatus{
		Phase: corev1.PodRunning,
	}
	h.pods[testPodFullName] = pod
}

func withEndTime(h *podLifecycleHandler) {
	h.podEndTimes[testPodFullName] = testEndTime
}

func makePod(initContainers []corev1.Container, containers []corev1.Container, lifetime *time.Duration) *corev1.Pod {
	var annotations map[string]string
	if lifetime != nil {
		annotations = map[string]string{
			lifetimeAnnotationKey: fmt.Sprint(int64(*lifetime / time.Second)),
		}
	}
	return &corev1.Pod{
		ObjectMeta: metav1.ObjectMeta{
			Name:        testPodName,
			Namespace:   testNamespace,
			Annotations: annotations,
		},
		Spec: corev1.PodSpec{
			InitContainers: initContainers,
			Containers:     containers,
		},
		Status: corev1.PodStatus{},
	}
}

func TestCreatePod(t *testing.T) {
	cases := map[string]struct {
		initContainers []corev1.Container
		containers     []corev1.Container
		lifetime       *time.Duration
	}{
		"basic pod": {
			containers: []corev1.Container{testContainer},
		},
		"pod with init containers": {
			initContainers: []corev1.Container{{Name: "test-init-container"}},
			containers:     []corev1.Container{testContainer},
		},
		"pod with lifetime": {
			containers: []corev1.Container{testContainer},
			lifetime:   lo.ToPtr(5 * time.Second),
		},
	}

	for name, tc := range cases {
		t.Run(name, func(t *testing.T) {
			c := clockwork.NewFakeClockAt(time.Time{})
			pod := makePod(tc.containers, tc.initContainers, tc.lifetime)
			podHandler := makePodLifecycleHandler(func(h *podLifecycleHandler) { h.clock = c })

			err := podHandler.CreatePod(context.TODO(), pod)

			assert.Nil(t, err)
			assert.Equal(t, pod.Status.Phase, corev1.PodRunning)
			assert.Len(t, pod.Status.InitContainerStatuses, len(pod.Spec.InitContainers))
			assert.Len(t, pod.Status.ContainerStatuses, len(pod.Spec.Containers))

			if tc.lifetime != nil {
				assert.Equal(t, testEndTime, podHandler.podEndTimes[testPodFullName])
			}
		})
	}
}

func TestCreatePodUnparseableLifetime(t *testing.T) {
	pod := makePod(nil, nil, nil)
	pod.ObjectMeta.Annotations = map[string]string{
		lifetimeAnnotationKey: "asdf",
	}
	podHandler := makePodLifecycleHandler()

	err := podHandler.CreatePod(context.TODO(), pod)

	assert.Nil(t, err)
	assert.NotContains(t, podHandler.podEndTimes, testPodFullName)
}

func TestUpdatePod(t *testing.T) {
	pod := makePod(nil, []corev1.Container{testContainer}, nil)
	podHandler := makePodLifecycleHandler()

	err := podHandler.UpdatePod(context.TODO(), pod)
	assert.Nil(t, err)
}

func TestDeletePod(t *testing.T) {
	pod := makePod(nil, []corev1.Container{testContainer}, nil)
	podHandler := makePodLifecycleHandler()

	err := podHandler.DeletePod(context.TODO(), pod)
	assert.Nil(t, err)
	assert.NotContains(t, podHandler.pods, testPodName)
}

func TestGetUnknownPod(t *testing.T) {
	podHandler := makePodLifecycleHandler(withPod)

	_, err := podHandler.GetPod(context.TODO(), "foo", "bar")
	assert.ErrorIs(t, err, ErrorPodNotFound)
}

func TestGetPod(t *testing.T) {
	podHandler := makePodLifecycleHandler(withPod)

	pod, err := podHandler.GetPod(context.TODO(), testNamespace, testPodName)
	assert.Nil(t, err)
	assert.Equal(t, testNamespace, pod.ObjectMeta.Namespace)
	assert.Equal(t, testPodName, pod.ObjectMeta.Name)
}

func TestGetUnknownPodStatus(t *testing.T) {
	podHandler := makePodLifecycleHandler(withPod)

	_, err := podHandler.GetPodStatus(context.TODO(), "foo", "bar")
	assert.ErrorIs(t, err, ErrorPodNotFound)
}

func TestGetPodStatus(t *testing.T) {
	podHandler := makePodLifecycleHandler(withPod)

	status, err := podHandler.GetPodStatus(context.TODO(), testNamespace, testPodName)
	assert.Nil(t, err)
	assert.Equal(t, corev1.PodRunning, status.Phase)
}

func TestGetPodStatusWithExpiration(t *testing.T) {
	cases := map[string]struct {
		duration      time.Duration
		expectedPhase corev1.PodPhase
		expectedState corev1.ContainerState
		expectedReady bool
	}{
		"not expired": {
			duration:      2 * time.Second,
			expectedPhase: corev1.PodRunning,
			expectedState: corev1.ContainerState{Running: &corev1.ContainerStateRunning{StartedAt: metav1.Time{}}},
			expectedReady: true,
		},
		"expired": {
			duration:      10 * time.Second,
			expectedPhase: corev1.PodSucceeded,
			expectedState: corev1.ContainerState{Terminated: &corev1.ContainerStateTerminated{
				StartedAt:  metav1.Time{},
				FinishedAt: metav1.Time{Time: testEndTime},
			}},
			expectedReady: false,
		},
	}

	for name, tc := range cases {
		t.Run(name, func(t *testing.T) {
			c := clockwork.NewFakeClockAt(time.Time{})
			podHandler := makePodLifecycleHandler(
				withPod,
				withEndTime,
				func(h *podLifecycleHandler) { h.clock = c },
				func(h *podLifecycleHandler) {
					h.pods[testPodFullName].Status.ContainerStatuses = []corev1.ContainerStatus{
						{Name: testContainerName,
							State: corev1.ContainerState{
								Running: &corev1.ContainerStateRunning{
									StartedAt: metav1.Time{},
								},
							},
							Ready: true,
						},
					}
				},
			)
			c.Advance(tc.duration)

			status, err := podHandler.GetPodStatus(context.TODO(), testNamespace, testPodName)

			assert.Nil(t, err)
			assert.Equal(t, tc.expectedPhase, status.Phase)
			for _, cs := range status.ContainerStatuses {
				assert.Equal(t, cs.Ready, tc.expectedReady)
				assert.Equal(t, cs.State, tc.expectedState)
			}
		})
	}
}

func TestGetPods(t *testing.T) {
	podHandler := makePodLifecycleHandler(withPod)

	pods, err := podHandler.GetPods(context.TODO())
	assert.Nil(t, err)
	assert.Len(t, pods, 1)
	assert.Equal(t, pods[0].ObjectMeta.Name, testPodName)
}
