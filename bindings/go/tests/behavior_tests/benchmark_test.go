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
	"testing"

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/credentials"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/s3"
	"github.com/google/uuid"
)

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
