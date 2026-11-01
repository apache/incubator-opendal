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

// The Getting Started example for the Go binding, embedded into the website
// guide. It runs against the in-memory service with no credentials, so CI can
// execute it directly. The region between the ANCHOR markers is what the docs
// show -- keep it copy-pasteable.

// ANCHOR: quickstart
package main

import (
	"fmt"
	"log"

	"github.com/apache/opendal-go-services/memory"
	opendal "github.com/apache/opendal/bindings/go"
)

func main() {
	// Configure a service, then build an operator from it.
	op, err := opendal.NewOperator(memory.Scheme, opendal.OperatorOptions{})
	if err != nil {
		log.Fatal(err)
	}
	defer op.Close() // Always close the operator to release resources.

	// The same verbs work on every service.
	if _, err := op.Write("hello.txt", []byte("Hello, World!")); err != nil {
		log.Fatal(err)
	}

	data, err := op.Read("hello.txt")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("read %d bytes: %s\n", len(data), data)

	meta, err := op.Stat("hello.txt")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("size = %d bytes\n", meta.ContentLength())

	if err := op.Delete("hello.txt"); err != nil {
		log.Fatal(err)
	}
}

// ANCHOR_END: quickstart
