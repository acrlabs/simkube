package simkube

import (
	"context"

	corev1 "k8s.io/api/core/v1"

	"simkube/pkg/util"
)

type podLifecycleHandler struct {
	nodeName string
}

func (self *podLifecycleHandler) CreatePod(ctx context.Context, pod *corev1.Pod) error {
	podName := util.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Creating pod")
	return nil
}

func (self *podLifecycleHandler) UpdatePod(ctx context.Context, pod *corev1.Pod) error {
	podName := util.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Updating pod")

	return nil
}

func (self *podLifecycleHandler) DeletePod(ctx context.Context, pod *corev1.Pod) error {
	podName := util.NamespacedNameFromObjectMeta(pod.ObjectMeta)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Deleting pod")

	return nil
}

func (self *podLifecycleHandler) GetPod(ctx context.Context, namespace, name string) (*corev1.Pod, error) {
	podName := util.NamespacedName(namespace, name)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Info("Getting pod")

	return nil, nil
}

func (self *podLifecycleHandler) GetPodStatus(ctx context.Context, namespace, name string) (*corev1.PodStatus, error) {
	podName := util.NamespacedName(namespace, name)
	logger := util.GetLogger(self.nodeName, "podName", podName)
	logger.Infof("Getting pod status")

	return nil, nil
}

func (self *podLifecycleHandler) GetPods(context.Context) ([]*corev1.Pod, error) {
	logger := util.GetLogger(self.nodeName)
	logger.Info("Getting all pods")

	return nil, nil
}
