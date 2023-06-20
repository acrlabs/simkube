package util

import (
	"fmt"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

func NamespacedNameFromObjectMeta(objmeta metav1.ObjectMeta) string {
	return NamespacedName(objmeta.Namespace, objmeta.Name)
}

func NamespacedName(namespace, name string) string {
	return fmt.Sprintf("%s/%s", namespace, name)
}
