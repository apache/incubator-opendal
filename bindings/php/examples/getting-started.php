<?php
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

// ANCHOR: quickstart
use OpenDAL\Operator;

$op = new Operator("memory", []);

// Write a UTF-8 string.
$op->write("hello.txt", "Hello, World!");

// Read it back. Returns a string (binary-safe).
$content = $op->read("hello.txt");
echo $content . "\n"; // Hello, World!

// Inspect metadata.
$meta = $op->stat("hello.txt");
echo $meta->content_length . " bytes\n"; // 13

// Delete. Succeeds even if the path does not exist.
$op->delete("hello.txt");

// Confirm deletion.
$exists = $op->is_exist("hello.txt");
echo $exists ? "still exists\n" : "deleted\n"; // deleted
// ANCHOR_END: quickstart
