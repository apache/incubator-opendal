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

//go:build windows

package opendal

import (
	"golang.org/x/sys/windows"
)

func BytePtrFromString(s string) (*byte, error) {
	if s == "" {
		return new(byte), nil
	}
	return windows.BytePtrFromString(s)
}

func BytePtrToString(p *byte) string {
	if p == nil {
		return ""
	}
	return windows.BytePtrToString(p)
}

func LoadLibrary(path string) (uintptr, error) {
	handle, err := windows.LoadLibrary(path)
	if err != nil {
		return 0, err
	}
	return uintptr(handle), nil
}

func FreeLibrary(handle uintptr) error {
	if handle == 0 {
		return nil
	}
	err := windows.FreeLibrary(windows.Handle(handle))
	if err != nil {
		return err
	}
	return nil
}

func GetProcAddress(handle uintptr, name string) (uintptr, error) {
	if handle == 0 {
		return 0, nil
	}
	proc, err := windows.GetProcAddress(windows.Handle(handle), name)
	if err != nil {
		return 0, err
	}
	return proc, nil
}
