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

package opendal_test

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/credentials"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/s3"
	"github.com/google/uuid"
)

func BenchmarkOpenDALCopy(b *testing.B) {
	for _, objectSize := range []int{16 << 20, 256 << 20} {
		b.Run(fmt.Sprintf("%dMiB", objectSize>>20), func(b *testing.B) {
			cap := op.Info().GetCapability()
			if !cap.Read() || !cap.WriteCanMulti() {
				b.Skip("service must support reading and streaming writes")
			}
			if limit := cap.WriteTotalMaxSize(); limit > 0 && limit < uint(objectSize) {
				b.Skip("service write limit is smaller than the benchmark object")
			}
			source, destination := uuid.NewString(), uuid.NewString()
			b.Cleanup(func() {
				_ = op.Delete(source)
				_ = op.Delete(destination)
			})
			data := make([]byte, objectSize)
			if _, err := op.Write(source, data); err != nil {
				b.Fatal(err)
			}
			for _, native := range []bool{true, false} {
				name := "native"
				if !native {
					name = "buffered"
				}
				b.Run(name, func(b *testing.B) {
					b.SetBytes(int64(len(data)))
					b.ReportAllocs()
					var copyTime, closeTime time.Duration
					b.ResetTimer()
					for i := 0; i < b.N; i++ {
						r, err := op.Reader(source)
						if err != nil {
							b.Fatal(err)
						}
						w, err := op.Writer(destination)
						if err != nil {
							_ = r.Close()
							b.Fatal(err)
						}
						var n int64
						copyStart := time.Now()
						if native {
							n, err = io.Copy(w, r)
						} else {
							n, err = w.ReadFrom(struct{ io.Reader }{r})
						}
						copyTime += time.Since(copyStart)
						closeStart := time.Now()
						readCloseErr := r.Close()
						_, writeCloseErr := w.Close()
						closeTime += time.Since(closeStart)
						if err != nil || readCloseErr != nil || writeCloseErr != nil || n != int64(len(data)) {
							b.Fatalf("copy = (%d, %v), close = (%v, %v)", n, err, readCloseErr, writeCloseErr)
						}
					}
					b.ReportMetric(float64(copyTime.Nanoseconds())/float64(b.N), "copy-ns/op")
					b.ReportMetric(float64(closeTime.Nanoseconds())/float64(b.N), "close-ns/op")
				})
			}
		})
	}
}

func BenchmarkWriterReadFrom(b *testing.B) {
	for _, objectSize := range []int{16 << 20, 256 << 20} {
		b.Run(fmt.Sprintf("%dMiB", objectSize>>20), func(b *testing.B) {
			cap := op.Info().GetCapability()
			if !cap.WriteCanMulti() {
				b.Skip("service does not support streaming writes")
			}
			if limit := cap.WriteTotalMaxSize(); limit > 0 && limit < uint(objectSize) {
				b.Skip("service write limit is smaller than the benchmark object")
			}
			data := make([]byte, objectSize)
			source := filepath.Join(b.TempDir(), "source")
			if err := os.WriteFile(source, data, 0600); err != nil {
				b.Fatal(err)
			}
			for _, size := range []int{0, 32 << 10, 64 << 10, 256 << 10, 1 << 20} {
				name := "io.Copy"
				if size != 0 {
					name = fmt.Sprint(size)
				}
				b.Run(name, func(b *testing.B) {
					file, err := os.Open(source)
					if err != nil {
						b.Fatal(err)
					}
					b.Cleanup(func() {
						if err := file.Close(); err != nil {
							b.Error(err)
						}
					})
					path := uuid.NewString()
					b.Cleanup(func() { _ = op.Delete(path) })
					b.SetBytes(int64(len(data)))
					b.ReportAllocs()
					var copyTime, closeTime time.Duration
					b.ResetTimer()
					for i := 0; i < b.N; i++ {
						if _, err := file.Seek(0, io.SeekStart); err != nil {
							b.Fatal(err)
						}
						w, err := op.Writer(path)
						if err != nil {
							b.Fatal(err)
						}
						var copyErr error
						copyStart := time.Now()
						if size == 0 {
							_, copyErr = io.Copy(w, file)
						} else {
							_, copyErr = io.CopyBuffer(struct{ io.Writer }{w}, struct{ io.Reader }{file}, make([]byte, size))
						}
						copyTime += time.Since(copyStart)
						closeStart := time.Now()
						_, closeErr := w.Close()
						closeTime += time.Since(closeStart)
						if copyErr != nil || closeErr != nil {
							b.Fatalf("copy = %v, close = %v", copyErr, closeErr)
						}
					}
					b.ReportMetric(float64(copyTime.Nanoseconds())/float64(b.N), "copy-ns/op")
					b.ReportMetric(float64(closeTime.Nanoseconds())/float64(b.N), "close-ns/op")
				})
			}
		})
	}
}

func BenchmarkReaderWriteTo(b *testing.B) {
	for _, objectSize := range []int{16 << 20, 256 << 20} {
		b.Run(fmt.Sprintf("%dMiB", objectSize>>20), func(b *testing.B) {
			cap := op.Info().GetCapability()
			if !cap.Read() || !cap.Write() {
				b.Skip("service must support reading and writing")
			}
			if limit := cap.WriteTotalMaxSize(); limit > 0 && limit < uint(objectSize) {
				b.Skip("service write limit is smaller than the benchmark object")
			}
			path := uuid.NewString()
			b.Cleanup(func() { _ = op.Delete(path) })
			data := make([]byte, objectSize)
			if _, err := op.Write(path, data); err != nil {
				b.Fatal(err)
			}
			for _, size := range []int{0, 256 << 10} {
				name := "io.Copy"
				if size != 0 {
					name = fmt.Sprint(size)
				}
				b.Run(name, func(b *testing.B) {
					file, err := os.Create(filepath.Join(b.TempDir(), "download"))
					if err != nil {
						b.Fatal(err)
					}
					b.Cleanup(func() {
						if err := file.Close(); err != nil {
							b.Error(err)
						}
					})
					writes := 0
					var firstWrite, copyTime, closeTime time.Duration
					b.SetBytes(int64(len(data)))
					b.ReportAllocs()
					b.ResetTimer()
					for i := 0; i < b.N; i++ {
						if _, err := file.Seek(0, io.SeekStart); err != nil {
							b.Fatal(err)
						}
						r, err := op.Reader(path)
						if err != nil {
							b.Fatal(err)
						}
						first := true
						copyStart := time.Now()
						dst := copyWriterFunc(func(p []byte) (int, error) {
							if first {
								firstWrite += time.Since(copyStart)
								first = false
							}
							writes++
							return file.Write(p)
						})
						var n int64
						var copyErr error
						if size == 0 {
							n, copyErr = io.Copy(dst, r)
						} else {
							n, copyErr = io.CopyBuffer(dst, struct{ io.Reader }{r}, make([]byte, size))
						}
						copyTime += time.Since(copyStart)
						closeStart := time.Now()
						closeErr := r.Close()
						closeTime += time.Since(closeStart)
						if copyErr != nil || closeErr != nil || n != int64(len(data)) {
							b.Fatalf("copy = (%d, %v), close = %v", n, copyErr, closeErr)
						}
					}
					b.ReportMetric(float64(writes)/float64(b.N), "writes/op")
					b.ReportMetric(float64(firstWrite.Nanoseconds())/float64(b.N), "first-write-ns/op")
					b.ReportMetric(float64(copyTime.Nanoseconds())/float64(b.N), "copy-ns/op")
					b.ReportMetric(float64(closeTime.Nanoseconds())/float64(b.N), "close-ns/op")
				})
			}
		})
	}
}

type Size uint64

const (
	_   = iota
	KiB = 1 << (10 * iota)
	MiB
)

func fromKibibytes(kib uint64) Size {
	return Size(kib * KiB)
}

func fromMebibytes(mib uint64) Size {
	return Size(mib * MiB)
}

func (s Size) Bytes() uint64 {
	return uint64(s)
}

func (s Size) String() string {
	switch {
	case s >= MiB:
		return fmt.Sprintf("%dMiB", s.Bytes()/MiB)
	case s >= KiB:
		return fmt.Sprintf("%dKiB", s.Bytes()/KiB)
	default:
		return fmt.Sprintf("%dB", s.Bytes())
	}
}

var sizes = []Size{
	fromKibibytes(4),
	fromKibibytes(256),
	fromMebibytes(4),
	fromMebibytes(16),
}

type ReadWriter interface {
	Write(path string, data []byte) error
	Read(path string) ([]byte, error)

	Name() string
}

type S3ReadWriter struct {
	client *s3.S3
}

func NewS3ReadWriter() ReadWriter {
	s3 := s3.New(session.Must(session.NewSession(&aws.Config{
		Credentials:      credentials.NewStaticCredentials(os.Getenv("OPENDAL_S3_ACCESS_KEY_ID"), os.Getenv("OPENDAL_S3_SECRET_ACCESS_KEY"), ""),
		Endpoint:         aws.String(os.Getenv("OPENDAL_S3_ENDPOINT")),
		Region:           aws.String(os.Getenv("OPENDAL_S3_REGION")),
		S3ForcePathStyle: aws.Bool(true),
		DisableSSL:       aws.Bool(true),
	})))
	return &S3ReadWriter{
		client: s3,
	}
}

func (rw *S3ReadWriter) Write(path string, data []byte) error {
	_, err := rw.client.PutObject(&s3.PutObjectInput{
		Bucket: aws.String(os.Getenv("OPENDAL_S3_BUCKET")),
		Key:    aws.String(path),
		Body:   aws.ReadSeekCloser(bytes.NewReader(data)),
	})
	return err
}

func (rw *S3ReadWriter) Read(path string) ([]byte, error) {
	resp, err := rw.client.GetObject(&s3.GetObjectInput{
		Bucket: aws.String(os.Getenv("OPENDAL_S3_BUCKET")),
		Key:    aws.String(path),
	})
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	return data, nil
}

func (rw *S3ReadWriter) Name() string {
	return "AWS S3"
}

type OpenDALReadWriter struct {
	*opendal.Operator
}

func NewOpenDALReadWriter(op *opendal.Operator) ReadWriter {
	return &OpenDALReadWriter{
		Operator: op,
	}
}

func (rw *OpenDALReadWriter) Write(path string, data []byte) error {
	_, err := rw.Operator.Write(path, data)
	return err
}

func (rw *OpenDALReadWriter) Read(path string) ([]byte, error) {
	return rw.Operator.Read(path)
}

func (rw *OpenDALReadWriter) Name() string {
	return "OpenDAL"
}

func runBenchmarkWrite(b *testing.B, size Size, op ReadWriter) {
	path := uuid.NewString()

	data := genFixedBytes(uint(size.Bytes()))

	b.ResetTimer()

	for n := 0; n < b.N; n++ {
		err := op.Write(path, data)
		if err != nil {
			b.Fatalf("%s", err)
		}
	}
}

func BenchmarkWrite(b *testing.B) {
	var ops = []ReadWriter{NewOpenDALReadWriter(op)}
	if os.Getenv("OPENDAL_TEST") == "s3" {
		ops = append(ops, NewS3ReadWriter())
	}
	for _, size := range sizes {
		for _, op := range ops {
			b.Run(fmt.Sprintf("%s/%s", size, op.Name()), func(b *testing.B) {
				runBenchmarkWrite(b, size, op)
			})
		}
	}
}

func runBenchmarkRead(b *testing.B, size Size, op ReadWriter) {
	path := uuid.NewString()

	data := genFixedBytes(uint(size.Bytes()))

	err := op.Write(path, data)

	if err != nil {
		b.Fatalf("%s", err)
	}

	b.ResetTimer()

	for n := 0; n < b.N; n++ {
		_, err := op.Read(path)
		if err != nil {
			b.Fatalf("%s", err)
		}
	}
}

func BenchmarkRead(b *testing.B) {
	var ops = []ReadWriter{NewOpenDALReadWriter(op)}
	if os.Getenv("OPENDAL_TEST") == "s3" {
		ops = append(ops, NewS3ReadWriter())
	}
	for _, size := range sizes {
		for _, op := range ops {
			b.Run(fmt.Sprintf("%s/%s", size, op.Name()), func(b *testing.B) {
				runBenchmarkRead(b, size, op)
			})
		}
	}
}
