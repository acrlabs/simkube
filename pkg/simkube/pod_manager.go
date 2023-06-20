package simkube

import (
	"context"
	"fmt"
	"path"
	"time"

	log "github.com/sirupsen/logrus"
	"github.com/virtual-kubelet/virtual-kubelet/node"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/fields"
	"k8s.io/client-go/informers"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/kubernetes/scheme"
	corev1client "k8s.io/client-go/kubernetes/typed/core/v1"
	"k8s.io/client-go/tools/record"

	"simkube/pkg/util"
)

const (
	podSyncWorkers       = 1
	informerResyncPeriod = 30 * time.Second
)

type PodLifecycleManagerI interface {
	Run(context.Context, context.CancelCauseFunc)
}

type PodLifecycleManager struct {
	nodeName   string
	k8sClient  kubernetes.Interface
	podHandler node.PodLifecycleHandler
	logger     *log.Entry
}

func NewPodLifecycleManager(nodeName string, k8sClient kubernetes.Interface) *PodLifecycleManager {
	podHandler := &podLifecycleHandler{}
	return &PodLifecycleManager{
		nodeName:   nodeName,
		k8sClient:  k8sClient,
		podHandler: podHandler,
		logger:     util.GetLogger(nodeName),
	}
}

func (self *PodLifecycleManager) Run(ctx context.Context, cancel context.CancelCauseFunc) {
	self.logger.Info("Starting pod manager...")

	podCtrlConfig := self.makePodControllerConfig(ctx)
	podCtrl, err := node.NewPodController(podCtrlConfig)
	if err != nil {
		cancel(fmt.Errorf("could not create pod controller: %w", err))
		return
	}

	go func() {
		if err := podCtrl.Run(ctx, podSyncWorkers); err != nil {
			cancel(fmt.Errorf("could not run pod controller: %w", err))
		}
	}()
	self.logger.Info("Waiting for pod controller to be ready...")
	select {
	case <-podCtrl.Ready():
		self.logger.Info("Pod controller ready!")
	case <-ctx.Done():
		self.logger.Error("context canceled")
	}
	self.logger.Info("Pod manager running!")
}

func (self *PodLifecycleManager) makePodControllerConfig(ctx context.Context) node.PodControllerConfig {
	podInformerFactory := informers.NewSharedInformerFactoryWithOptions(
		self.k8sClient,
		informerResyncPeriod,
		informers.WithNamespace(corev1.NamespaceAll),
		informers.WithTweakListOptions(func(options *metav1.ListOptions) {
			options.FieldSelector = fields.OneTermEqualSelector("spec.nodeName", self.nodeName).String()
		}))

	// If you don't call <informer>.Informer() before you call <informerFactory>.Start(), the
	// informer never gets registered and everything just hangs forever while it waits for the
	// caches of the set of empty informers to sync.  I don't know why the other virtual-kubelet
	// apps don't run into this problem; maybe some issue between when they were last released and
	// the current version of client-go?  Anyways this is the best solution I have for now.
	podInformer := podInformerFactory.Core().V1().Pods()
	podInformer.Informer()
	podInformerFactory.Start(ctx.Done())

	scmInformerFactory := informers.NewSharedInformerFactory(self.k8sClient, informerResyncPeriod)
	secretInformer := scmInformerFactory.Core().V1().Secrets()
	cmInformer := scmInformerFactory.Core().V1().ConfigMaps()
	svcInformer := scmInformerFactory.Core().V1().Services()

	// see note above
	cmInformer.Informer()
	secretInformer.Informer()
	svcInformer.Informer()
	scmInformerFactory.Start(ctx.Done())

	eventBroadcaster := record.NewBroadcaster()
	eventBroadcaster.StartLogging(util.GetLogger(self.nodeName).Infof)
	eventBroadcaster.StartRecordingToSink(
		&corev1client.EventSinkImpl{Interface: self.k8sClient.CoreV1().Events(corev1.NamespaceAll)},
	)
	recorder := eventBroadcaster.NewRecorder(
		scheme.Scheme,
		corev1.EventSource{Component: path.Join(self.nodeName, "pod-controller")},
	)

	return node.PodControllerConfig{
		PodClient:         self.k8sClient.CoreV1(),
		EventRecorder:     recorder,
		Provider:          self.podHandler,
		PodInformer:       podInformer,
		SecretInformer:    secretInformer,
		ConfigMapInformer: cmInformer,
		ServiceInformer:   svcInformer,
	}
}
