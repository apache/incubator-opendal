--[[
    Licensed to the Apache Software Foundation (ASF) under one
    or more contributor license agreements.  See the NOTICE file
    distributed with this work for additional information
    regarding copyright ownership.  The ASF licenses this file
    to you under the Apache License, Version 2.0 (the
    "License"); you may not use this file except in compliance
    with the License.  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing,
    software distributed under the License is distributed on an
    "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
    KIND, either express or implied.  See the License for the
    specific language governing permissions and limitations
    under the License.
]]

-- The Getting Started example for the Lua binding, embedded into the website
-- guide. It runs against the in-memory service with no credentials, so CI can
-- execute it directly. The region between the ANCHOR markers is what the docs
-- show — keep it copy-pasteable.

-- ANCHOR: quickstart
local opendal = require("opendal")

-- Create an operator against the in-memory service — no credentials needed.
-- Wrap in pcall because mlua raises errors as Lua errors, not (val, err) tuples.
local ok, op = pcall(opendal.operator.new, "memory", {})
if not ok then
    print("failed to create operator:", op)
    os.exit(1)
end

-- Write a string to a path.
local ok, err = pcall(function() op:write("hello.txt", "Hello, World!") end)
if not ok then
    print("write failed:", err)
    os.exit(1)
end

-- Read it back. Returns the content as a Lua string on success.
local ok, data = pcall(function() return op:read("hello.txt") end)
if not ok then
    print("read failed:", data)
    os.exit(1)
end
print("read " .. #data .. " bytes")

-- Inspect metadata.
local ok, meta = pcall(function() return op:stat("hello.txt") end)
if not ok then
    print("stat failed:", meta)
    os.exit(1)
end
print("size = " .. meta:content_length() .. " bytes")

-- Clean up.
local ok, err = pcall(function() op:delete("hello.txt") end)
if not ok then
    print("delete failed:", err)
    os.exit(1)
end
-- ANCHOR_END: quickstart
