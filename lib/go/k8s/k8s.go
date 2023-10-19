package k8s

import (
	"fmt"
	"strings"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/rest"
)

func NewClient() (*kubernetes.Clientset, error) {
	config, err := rest.InClusterConfig()
	if err != nil {
		return nil, fmt.Errorf("could not get client config: %w", err)
	}

	k8sClient, err := kubernetes.NewForConfig(config)
	if err != nil {
		return nil, fmt.Errorf("could not initialize Kubernetes client: %w", err)
	}

	return k8sClient, nil
}

func NamespacedNameFromObjectMeta(objmeta metav1.ObjectMeta) string {
	return NamespacedName(objmeta.Namespace, objmeta.Name)
}

func NamespacedName(namespace, name string) string {
	return fmt.Sprintf("%s/%s", namespace, name)
}

func ProviderID(nodeName string) string {
	return fmt.Sprintf("simkube://%s", nodeName)
}

func SplitNamespacedName(namespacedName string) (string, string) {
	split := strings.SplitN(namespacedName, "/", 2)
	if len(split) == 2 {
		return split[0], split[1]
	} else {
		return split[0], ""
	}
}
