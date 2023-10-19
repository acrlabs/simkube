package cloudprov

import (
	"context"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	confautoscalingv1 "k8s.io/client-go/applyconfigurations/autoscaling/v1"
	"k8s.io/client-go/kubernetes"
)

type scalerI interface {
	ScaleTo(context.Context, string, string, int32) error
}

type scaler struct {
	k8sClient kubernetes.Interface
}

func (self *scaler) ScaleTo(ctx context.Context, namespace, name string, target int32) error {
	scale := confautoscalingv1.Scale().WithSpec(&confautoscalingv1.ScaleSpecApplyConfiguration{
		Replicas: &target,
	})
	if _, err := self.k8sClient.AppsV1().Deployments(namespace).ApplyScale(
		ctx,
		name,
		scale,
		metav1.ApplyOptions{Force: true, FieldManager: providerName},
	); err != nil {
		//nolint:wrapcheck // this is just a passthrough interface for testing
		return err
	}
	return nil
}
