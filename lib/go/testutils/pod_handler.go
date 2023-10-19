package testutils

import (
	"context"

	"github.com/stretchr/testify/mock"
	corev1 "k8s.io/api/core/v1"
)

type PodHandler struct {
	mock.Mock
}

func (self *PodHandler) CreatePod(ctx context.Context, pod *corev1.Pod) error {
	retvals := self.Called(ctx, pod)
	return retvals.Error(0)
}

func (self *PodHandler) UpdatePod(ctx context.Context, pod *corev1.Pod) error {
	retvals := self.Called(ctx, pod)
	return retvals.Error(0)
}

func (self *PodHandler) DeletePod(ctx context.Context, pod *corev1.Pod) error {
	retvals := self.Called(ctx, pod)
	return retvals.Error(0)
}

func (self *PodHandler) GetPod(ctx context.Context, namespace, name string) (*corev1.Pod, error) {
	retvals := self.Called(ctx, namespace, name)
	return retvals.Get(0).(*corev1.Pod), retvals.Error(1)
}

func (self *PodHandler) GetPodStatus(ctx context.Context, namespace, name string) (*corev1.PodStatus, error) {
	retvals := self.Called(ctx, namespace, name)
	return retvals.Get(0).(*corev1.PodStatus), retvals.Error(1)
}

func (self *PodHandler) GetPods(ctx context.Context) ([]*corev1.Pod, error) {
	retvals := self.Called(ctx)
	return retvals.Get(0).([]*corev1.Pod), retvals.Error(1)
}

func NewPodHandler() *PodHandler {
	ph := &PodHandler{}

	ph.On("CreatePod", mock.Anything, mock.Anything).Return(nil)
	ph.On("UpdatePod", mock.Anything, mock.Anything).Return(nil)
	ph.On("DeletePod", mock.Anything, mock.Anything).Return(nil)
	ph.On("GetPod", mock.Anything, mock.Anything, mock.Anything).Return(nil, nil)
	ph.On("GetPodStatus", mock.Anything, mock.Anything, mock.Anything).Return(nil, nil)
	ph.On("GetPods", mock.Anything).Return([]*corev1.Pod{}, nil)
	return ph
}
