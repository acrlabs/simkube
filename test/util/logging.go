package util

import (
	log "github.com/sirupsen/logrus"
	"github.com/sirupsen/logrus/hooks/test"
)

func GetFakeLogger() *log.Entry {
	l, _ := test.NewNullLogger()
	return l.WithFields(log.Fields{"test": "true"})
}
