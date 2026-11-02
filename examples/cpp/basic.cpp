/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

#include "opendal.hpp"

#include <iostream>
#include <string>

int main() {
  std::string_view data = "abc";

  // Init operator
  opendal::Operator op = opendal::Operator("memory");

  // Write data to operator
  op.Write("test", data);

  // Read data from operator
  auto res = op.Read("test"); // res == data
  std::cout << res << std::endl;

  // Using reader
  auto reader = op.GetReader("test");
  std::string res2(3, 0);
  reader.Read(res2.data(), data.size()); // res2 == "abc"
  std::cout << res2 << std::endl;

  // Using reader stream
  opendal::ReaderStream stream(op.GetReader("test"));
  std::string res3;
  stream >> res3; // res3 == "abc"
  std::cout << res3 << std::endl;
}
