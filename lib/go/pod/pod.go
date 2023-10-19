package pod

import (
	"context"

	vkerr "github.com/virtual-kubelet/virtual-kubelet/errdefs"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	"simkube/lib/go/k8s"
	"simkube/lib/go/util"
)

var ErrorPodNotFound = vkerr.NotFound("pod not found")

type podLifecycleHandler struct {
	nodeName string
	pods     map[string]*corev1.Pod
}

func newPodHandler(nodeName string) *podLifecycleHandler {
	return &podLifecycleHandler{nodeName, map[string]*corev1.Pod{}}
}

func (self *podLifecycleHandler) CreatePod(ctx context.Context, pod *corev1.Pod) error {
	podName := k8s.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Creating pod")

	pod.Status.Phase = corev1.PodRunning

	for _, c := range pod.Spec.InitContainers {
		cStatus := corev1.ContainerStatus{
			Name: c.Name,
			State: corev1.ContainerState{
				Terminated: &corev1.ContainerStateTerminated{FinishedAt: metav1.Now()},
			},
			Ready: true,
		}
		pod.Status.InitContainerStatuses = append(pod.Status.InitContainerStatuses, cStatus)
	}
	for _, c := range pod.Spec.Containers {
		cStatus := corev1.ContainerStatus{
			Name: c.Name,
			State: corev1.ContainerState{
				Running: &corev1.ContainerStateRunning{StartedAt: metav1.Now()},
			},
			Ready: true,
		}
		pod.Status.ContainerStatuses = append(pod.Status.InitContainerStatuses, cStatus)
	}

	pod.Status.Conditions = append(pod.Status.Conditions, []corev1.PodCondition{
		{
			Type:   corev1.PodInitialized,
			Status: corev1.ConditionTrue,
		},
		{
			Type:   corev1.ContainersReady,
			Status: corev1.ConditionTrue,
		},
		{
			Type:   corev1.PodReady,
			Status: corev1.ConditionTrue,
		},
	}...)

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
	logger.Infof("Getting pod status")

	if pod, ok := self.pods[podName]; !ok {
		//nolint:wrapcheck // this is my error, doesn't need to be wrapped
		return nil, ErrorPodNotFound
	} else {
		return pod.Status.DeepCopy(), nil
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
