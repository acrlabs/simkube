package util

import (
	"testing"
	"time"

	"github.com/jonboulle/clockwork"
	"github.com/stretchr/testify/assert"
)

func TestParseTimeStr(t *testing.T) {
	cases := map[string]struct {
		str      string
		start    time.Time
		expected time.Time
	}{
		"now": {str: "now"},
		"duration": {
			str:      "-15m",
			expected: time.Time{}.Add(-15 * time.Minute),
		},
		"duration from start": {
			str:      "-15m",
			start:    time.Unix(12345678, 0),
			expected: time.Unix(12345678, 0).Add(-15 * time.Minute),
		},
		"abs time": {
			str:      "2023-10-19T01:04:32",
			expected: time.Date(2023, 10, 19, 01, 04, 32, 0, time.Local),
		},
	}

	for name, tc := range cases {
		t.Run(name, func(t *testing.T) {
			c := clockwork.NewFakeClockAt(time.Time{})
			res, err := parseTimeStrWithClock(tc.str, tc.start, c)
			assert.Nil(t, err)
			assert.Equal(t, res, tc.expected)
		})
	}
}

func TestParseTimeStrError(t *testing.T) {
	_, err := ParseTimeStr("asdf", time.Time{})
	assert.NotNil(t, err)
}
