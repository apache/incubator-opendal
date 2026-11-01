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

local opendal = require("opendal")

describe("opendal unit test", function()
  describe("opendal fs schema", function()
    it("operator function in fs schema", function()
      local op, err = opendal.operator.new("fs",{root="/tmp"})
      assert.is_nil(err)
      assert.is_nil(op:write("test.txt","hello world"))
      local res, err = op:read("test.txt")
      assert.is_nil(err)
      assert.are.equal(res, "hello world")
      assert.is_nil(op:delete("test_dir/"))
      assert.is_nil(op:delete("test_dir_1/"))
      assert.is_nil(op:create_dir("test_dir/"))
      assert.equal(op:is_exist("test_dir/"), true)
      assert.is_nil(op:rename("test.txt", "test_1.txt"))
      assert.equal(op:is_exist("test_1.txt"), true)
      assert.equal(op:is_exist("test.txt"), false)
      assert.equal(op:is_exist("test_1.txt"), true)
      assert.is_nil(op:delete("test_1.txt"))
      assert.equal(op:is_exist("test_1.txt"), false)
    end)
    it("meta function in fs schema", function()
      local op, err = opendal.operator.new("fs",{root="/tmp"})
      assert.is_nil(err)
      assert.is_nil(op:write("test.txt","hello world"))
      local meta, err = op:stat("test.txt")
      assert.is_nil(err)
      local res, err = meta:content_length()
      assert.is_nil(err)
      assert.are.equal(res, 11)
      local res, err = meta:is_file()
      assert.is_nil(err)
      assert.are.equal(res, true)
      local res, err = meta:is_dir()
      assert.is_nil(err)
      assert.are.equal(res, false)
    end)
  end)
  describe("opendal memory schema", function()
    it("operator function in memory schema", function()
      local op, err = opendal.operator.new("memory",{root="/tmp"})
      assert.is_nil(err)
      assert.is_nil(op:write("test.txt","hello world"))
      local res, err = op:read("test.txt")
      assert.is_nil(err)
      assert.are.equal(res, "hello world")
      assert.is_nil(op:delete("test_1.txt"))
      assert.equal(op:is_exist("test_1.txt"), false)
    end)
    it("meta function in memory schema", function()
      local op, err = opendal.operator.new("memory",{root="/tmp"})
      assert.is_nil(err)
      assert.is_nil(op:write("test.txt","hello world"))
      local meta, err = op:stat("test.txt")
      assert.is_nil(err)
      local res, err = meta:content_length()
      assert.is_nil(err)
      assert.are.equal(res, 11)
      local res, err = meta:is_file()
      assert.is_nil(err)
      assert.are.equal(res, true)
      local res, err = meta:is_dir()
      assert.is_nil(err)
      assert.are.equal(res, false)
    end)
  end)
end)
