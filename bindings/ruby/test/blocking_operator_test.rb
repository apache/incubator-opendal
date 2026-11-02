# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

# frozen_string_literal: true

require "test_helper"
require "tmpdir"

class BlockingOperatorTest < ActiveSupport::TestCase
  setup do
    @root = Dir.mktmpdir
    File.write("#{@root}/sample", "Sample data for testing")
    @op = OpenDal::Operator.new("fs", {"root" => @root})
  end

  teardown do
    FileUtils.remove_entry(@root) if File.exist?(@root)
  end

  test "write writes to a file" do
    @op.write("/file", "OpenDAL Ruby is ready.")

    assert_equal "OpenDAL Ruby is ready.", File.read("/#{@root}/file")
  end

  test "write writes binary" do
    # writes 32-bit signed integers
    @op.write("/file", [67305985, -50462977].pack("l*"))

    assert_equal [67305985, -50462977], File.binread("/#{@root}/file").unpack("l*")
  end

  test "read reads file data" do
    data = @op.read("/sample")

    assert_equal "Sample data for testing", data
  end

  test "stat returns file metadata" do
    stat = @op.stat("sample")

    assert stat.is_a?(OpenDal::Metadata)
    assert_equal 23, stat.content_length
    assert stat.file?
  end

  test "create_dir creates directory" do
    @op.create_dir("/new/directory/")
    assert File.directory?("#{@root}/new/directory/")
  end

  test "exists returns existence" do
    assert @op.exist?("sample")
  end

  test "delete removes file" do
    @op.delete("/sample")

    assert_not File.exist?("#{@root}/sample")
  end

  test "rename renames file" do
    @op.rename("/sample", "/new_name")

    assert_not File.exist?("#{@root}/sample")
    assert File.exist?("#{@root}/new_name")
  end

  test "remove_all removes files" do
    @op.create_dir("/nested/directory/")
    @op.write("/nested/directory/text", "content")

    @op.remove_all("/")

    assert_not File.exist?(@root)
  end

  test "copy copies file" do
    @op.copy("/sample", "/new_name")

    assert File.exist?("#{@root}/sample")
    assert File.exist?("#{@root}/new_name")
  end

  test "opens an IO" do
    io = @op.open("/sample", "rb")

    assert_not io.closed?

    io.close
  end

  test "middleware applies a middleware" do
    @op.middleware(OpenDal::Middleware::Retry.new)

    assert @op.is_a?(OpenDal::Operator)
  end
end
