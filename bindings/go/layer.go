// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

package opendal

import (
	"context"
	"errors"
	"math"
	"time"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// OperatorOption configures operator layers.
type OperatorOption interface {
	apply(*operatorConfig) error
}

type operatorConfig struct {
	layers []operatorLayer
}

type operatorOptionFunc func(*operatorConfig) error

func (f operatorOptionFunc) apply(config *operatorConfig) error {
	return f(config)
}

type operatorLayer interface {
	apply(context.Context, *operatorLayers) error
}

type timeoutLayer struct {
	timeout   time.Duration
	ioTimeout time.Duration
}

type retryLayer struct {
	jitter   bool
	factor   float32
	minDelay time.Duration
	maxDelay time.Duration
	maxTimes uint64
}

type retryConfig struct {
	jitter   bool
	factor   float32
	minDelay time.Duration
	maxDelay time.Duration
	maxTimes uint64
}

// RetryOption configures the retry layer.
type RetryOption interface {
	applyRetry(*retryConfig) error
}

type retryOptionFunc func(*retryConfig) error

func (f retryOptionFunc) applyRetry(config *retryConfig) error {
	return f(config)
}

func defaultRetryConfig() retryConfig {
	return retryConfig{
		factor:   2,
		minDelay: time.Second,
		maxDelay: time.Minute,
		maxTimes: 3,
	}
}

// WithRetry adds a retry layer to the operator.
//
// Layers are applied in the order passed to NewOperator. When combining retry
// and timeout, pass WithTimeout before WithRetry so each retry attempt has its
// own timeout.
func WithRetry(options ...RetryOption) OperatorOption {
	return operatorOptionFunc(func(config *operatorConfig) error {
		retry := defaultRetryConfig()
		for _, option := range options {
			if option == nil {
				continue
			}
			if err := option.applyRetry(&retry); err != nil {
				return err
			}
		}
		if retry.maxDelay < retry.minDelay {
			return errors.New("retry max delay must be greater than or equal to retry min delay")
		}
		config.layers = append(config.layers, retryLayer(retry))
		return nil
	})
}

// RetryMaxTimes sets the retry layer max times.
func RetryMaxTimes(maxTimes int) RetryOption {
	return retryOptionFunc(func(config *retryConfig) error {
		if maxTimes <= 0 {
			return errors.New("retry max times must be positive")
		}
		config.maxTimes = uint64(maxTimes)
		return nil
	})
}

// RetryFactor sets the retry layer backoff factor.
func RetryFactor(factor float64) RetryOption {
	return retryOptionFunc(func(config *retryConfig) error {
		if math.IsNaN(factor) || math.IsInf(factor, 0) || factor < 1 {
			return errors.New("retry factor must be finite and greater than or equal to 1")
		}
		config.factor = float32(factor)
		return nil
	})
}

// RetryJitter enables jitter for the retry layer.
func RetryJitter() RetryOption {
	return retryOptionFunc(func(config *retryConfig) error {
		config.jitter = true
		return nil
	})
}

// RetryMinDelay sets the retry layer min delay.
func RetryMinDelay(delay time.Duration) RetryOption {
	return retryOptionFunc(func(config *retryConfig) error {
		if delay <= 0 {
			return errors.New("retry min delay must be positive")
		}
		config.minDelay = delay
		return nil
	})
}

// RetryMaxDelay sets the retry layer max delay.
func RetryMaxDelay(delay time.Duration) RetryOption {
	return retryOptionFunc(func(config *retryConfig) error {
		if delay <= 0 {
			return errors.New("retry max delay must be positive")
		}
		config.maxDelay = delay
		return nil
	})
}

// WithTimeout adds a timeout layer to the operator.
//
// timeout controls non-IO operations, while ioTimeout controls read, write,
// list, and streaming IO operations.
func WithTimeout(timeout, ioTimeout time.Duration) OperatorOption {
	return operatorOptionFunc(func(config *operatorConfig) error {
		if timeout <= 0 {
			return errors.New("timeout must be positive")
		}
		if ioTimeout <= 0 {
			return errors.New("io timeout must be positive")
		}
		config.layers = append(config.layers, timeoutLayer{
			timeout:   timeout,
			ioTimeout: ioTimeout,
		})
		return nil
	})
}

func (l timeoutLayer) apply(ctx context.Context, layers *operatorLayers) error {
	ffiOperatorLayersAddTimeout.symbol(ctx)(layers, uint64(l.timeout), uint64(l.ioTimeout))
	return nil
}

func (l retryLayer) apply(ctx context.Context, layers *operatorLayers) error {
	ffiOperatorLayersAddRetry.symbol(ctx)(
		layers,
		l.jitter,
		l.factor,
		uint64(l.minDelay),
		uint64(l.maxDelay),
		l.maxTimes,
	)
	return nil
}

type operatorLayers struct{}

var ffiOperatorLayersNew = newFFI(ffiOpts{
	sym:   "opendal_operator_layers_new",
	rType: &ffi.TypePointer,
}, func(_ context.Context, ffiCall ffiCall) func() (layers *operatorLayers) {
	return func() (layers *operatorLayers) {
		ffiCall(unsafe.Pointer(&layers))
		return
	}
})

var ffiOperatorLayersAddRetry = newFFI(ffiOpts{
	sym:   "opendal_operator_layers_add_retry",
	rType: &ffi.TypeVoid,
	aTypes: []*ffi.Type{
		&ffi.TypePointer,
		&ffi.TypeUint8,
		&ffi.TypeFloat,
		&ffi.TypeUint64,
		&ffi.TypeUint64,
		&ffi.TypeUint64,
	},
}, func(_ context.Context, ffiCall ffiCall) func(layers *operatorLayers, jitter bool, factor float32, minDelayNS, maxDelayNS, maxTimes uint64) {
	return func(layers *operatorLayers, jitter bool, factor float32, minDelayNS, maxDelayNS, maxTimes uint64) {
		ffiCall(
			nil,
			unsafe.Pointer(&layers),
			unsafe.Pointer(&jitter),
			unsafe.Pointer(&factor),
			unsafe.Pointer(&minDelayNS),
			unsafe.Pointer(&maxDelayNS),
			unsafe.Pointer(&maxTimes),
		)
	}
})

var ffiOperatorLayersAddTimeout = newFFI(ffiOpts{
	sym:    "opendal_operator_layers_add_timeout",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint64, &ffi.TypeUint64},
}, func(_ context.Context, ffiCall ffiCall) func(layers *operatorLayers, timeoutNS, ioTimeoutNS uint64) {
	return func(layers *operatorLayers, timeoutNS, ioTimeoutNS uint64) {
		ffiCall(
			nil,
			unsafe.Pointer(&layers),
			unsafe.Pointer(&timeoutNS),
			unsafe.Pointer(&ioTimeoutNS),
		)
	}
})

var ffiOperatorLayersFree = newFFI(ffiOpts{
	sym:    "opendal_operator_layers_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(layers *operatorLayers) {
	return func(layers *operatorLayers) {
		ffiCall(
			nil,
			unsafe.Pointer(&layers),
		)
	}
})
