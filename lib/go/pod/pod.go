package pod

import (
	"context"
	"strconv"
	"time"

	"github.com/jonboulle/clockwork"
	"github.com/samber/lo"
	vkerr "github.com/virtual-kubelet/virtual-kubelet/errdefs"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	"simkube/lib/go/k8s"
	"simkube/lib/go/util"
)

const lifetimeAnnotationKey = "simkube.io/lifetime-seconds"

var ErrorPodNotFound = vkerr.NotFound("pod not found")

type podLifecycleHandler struct {
	nodeName    string
	pods        map[string]*corev1.Pod
	podEndTimes map[string]time.Time
	clock       clockwork.Clock
}

func newPodHandler(nodeName string) *podLifecycleHandler {
	return &podLifecycleHandler{
		nodeName,
		map[string]*corev1.Pod{},
		map[string]time.Time{},
		clockwork.NewRealClock(),
	}
}

func (self *podLifecycleHandler) CreatePod(ctx context.Context, pod *corev1.Pod) error {
	podName := k8s.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Creating pod")

	self.setRunningStatus(pod)

	if pod.ObjectMeta.Annotations != nil {
		if lifetime_str, ok := pod.ObjectMeta.Annotations[lifetimeAnnotationKey]; ok {
			lifetime_seconds, err := strconv.Atoi(lifetime_str)
			if err != nil {
				logger.Warn("Could not parse lifetime annotation, pod will not terminate")
			} else {
				endTime := self.clock.Now().Add(time.Duration(lifetime_seconds) * time.Second)
				self.podEndTimes[podName] = endTime
				logger.Infof("pod end time recorded at %v", endTime)
			}
		}
	}

	self.pods[podName] = pod
	return nil
}

func (self *podLifecycleHandler) UpdatePod(ctx context.Context, pod *corev1.Pod) error {
	podName := k8s.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Updating pod")

	return nil
}

func (self *podLifecycleHandler) DeletePod(ctx context.Context, pod *corev1.Pod) error {
	podName := k8s.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Deleting pod")

	delete(self.pods, podName)
	return nil
}

func (self *podLifecycleHandler) GetPod(ctx context.Context, namespace, name string) (*corev1.Pod, error) {
	podName := k8s.NamespacedName(namespace, name)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Getting pod")

	if pod, ok := self.pods[podName]; !ok {
		//nolint:wrapcheck // this is my error, doesn't need to be wrapped
		return nil, ErrorPodNotFound
	} else {
		return pod.DeepCopy(), nil
	}
}

func (self *podLifecycleHandler) GetPodStatus(ctx context.Context, namespace, name string) (*corev1.PodStatus, error) {
	podName := k8s.NamespacedName(namespace, name)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Debug("Getting pod status")

	if pod, ok := self.pods[podName]; !ok {
		//nolint:wrapcheck // this is my error, doesn't need to be wrapped
		return nil, ErrorPodNotFound
	} else {
		var status *corev1.PodStatus
		if endTime, ok := self.podEndTimes[podName]; ok && self.clock.Now().After(endTime) {
			status = self.makeTerminatedStatus(pod, endTime)
		} else {
			status = pod.Status.DeepCopy()
		}
		return status, nil
	}
}

func (self *podLifecycleHandler) GetPods(context.Context) ([]*corev1.Pod, error) {
	logger := util.GetLogger(self.nodeName)
	logger.Info("Getting all pods")

	pods := make([]*corev1.Pod, 0, len(self.pods))
	for _, pod := range self.pods {
		pods = append(pods, pod.DeepCopy())
	}
	return pods, nil
}

func (self *podLifecycleHandler) setRunningStatus(pod *corev1.Pod) {
	pod.Status.Phase = corev1.PodRunning

	now := metav1.Time{Time: self.clock.Now()}
	pod.Status.InitContainerStatuses = make([]corev1.ContainerStatus, len(pod.Spec.InitContainers))
	for i, c := range pod.Spec.InitContainers {
		pod.Status.InitContainerStatuses[i] = corev1.ContainerStatus{
			Name: c.Name,
			State: corev1.ContainerState{
				// TODO eventually we could read these timestamps from annotations
				Terminated: &corev1.ContainerStateTerminated{StartedAt: now, FinishedAt: now},
			},
			Ready: true,
		}
	}

	pod.Status.ContainerStatuses = make([]corev1.ContainerStatus, len(pod.Spec.Containers))
	for i, c := range pod.Spec.Containers {
		pod.Status.ContainerStatuses[i] = corev1.ContainerStatus{
			Name: c.Name,
			State: corev1.ContainerState{
				Running: &corev1.ContainerStateRunning{StartedAt: now},
			},
			Ready: true,
		}
	}

	pod.Status.Conditions = append(pod.Status.Conditions, []corev1.PodCondition{
		{
			Type:               corev1.PodInitialized,
			Status:             corev1.ConditionTrue,
			LastTransitionTime: now,
		},
		{
			Type:               corev1.ContainersReady,
			Status:             corev1.ConditionTrue,
			LastTransitionTime: now,
		},
		{
			Type:               corev1.PodReady,
			Status:             corev1.ConditionTrue,
			LastTransitionTime: now,
		},
	}...)
}

func (self *podLifecycleHandler) makeTerminatedStatus(pod *corev1.Pod, endTime time.Time) *corev1.PodStatus {
	status := pod.Status.DeepCopy()

	status.Phase = corev1.PodSucceeded
	for _, cond := range status.Conditions {
		switch cond.Type {
		case corev1.PodReady, corev1.ContainersReady:
			cond.Status = corev1.ConditionFalse
			cond.LastTransitionTime = metav1.Time{Time: endTime}
		}
		cond.Reason = "PodCompleted"
	}
	for i, c := range pod.Spec.Containers {
		status.ContainerStatuses[i] = corev1.ContainerStatus{
			Name: c.Name,
			State: corev1.ContainerState{
				Terminated: &corev1.ContainerStateTerminated{
					StartedAt:  pod.Status.ContainerStatuses[i].State.Running.StartedAt,
					FinishedAt: metav1.Time{Time: endTime},
					ExitCode:   0,
				},
			},
			Ready:   false,
			Started: lo.ToPtr(false),
		}
	}

	return status
}
