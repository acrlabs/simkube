package util

import (
	"fmt"
	"time"

	"github.com/jonboulle/clockwork"
)

const ISO8601DateTimeExtended = "2006-01-02T03:04:05"

func ParseTimeStr(timeStr string, relTime time.Time) (time.Time, error) {
	return parseTimeStrWithClock(timeStr, relTime, clockwork.NewRealClock())
}

func parseTimeStrWithClock(timeStr string, relTime time.Time, clock clockwork.Clock) (time.Time, error) {
	if timeStr == "now" {
		return clock.Now(), nil
	} else {
		if res, parseErr1 := time.ParseInLocation(ISO8601DateTimeExtended, timeStr, time.Local); parseErr1 != nil {
			delta, parseErr2 := time.ParseDuration(timeStr)
			if parseErr2 != nil {
				return time.Time{}, fmt.Errorf(
					"could not parse time %s as absolute or relative time: %w, %w",
					timeStr,
					parseErr1,
					parseErr2,
				)
			}
			if relTime.IsZero() {
				relTime = clock.Now()
			}
			return relTime.Add(delta), nil
		} else {
			return res, nil
		}
	}
}
