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

class ListerTest < ActiveSupport::TestCase
  setup do
    @root = Dir.mktmpdir
    File.write("#{@root}/sample", "Sample data for testing")
    Dir.mkdir("#{@root}/sub")
    File.write("#{@root}/sub/sample", "Sample data for testing")
    @op = OpenDal::Operator.new("fs", {"root" => @root})
  end

  test "lists the directory" do
    lister = @op.list("")

    lists = lister.map(&:to_h).map { |e| e[:path] }.sort

    assert_equal ["/", "sample", "sub/"], lists
  end

  test "list returns the entry" do
    entry = @op.list("/").first

    assert entry.is_a?(OpenDal::Entry)
    assert entry.name.length > 0
  end

  test "entry has the metadata" do
    metadata = @op.list("sample").first.metadata

    assert metadata.file?
    assert !metadata.dir?
  end

  test "lists the directory recursively" do
    lister = @op.list("", recursive: true)

    lists = lister.map(&:to_h).map { |e| e[:path] }.sort

    assert_equal ["/", "sample", "sub/", "sub/sample"], lists
  end

  test "lists the directory with limit" do
    lister = @op.list("", limit: 1)

    lists = lister.map(&:to_h).map { |e| e[:path] }.sort

    assert_equal ["/", "sample", "sub/"], lists
  end

  test "lists the directory with start_after" do
    lister = @op.list("", start_after: "sub/")

    lists = lister.map(&:to_h).map { |e| e[:path] }.sort

    assert_equal ["/", "sample", "sub/"], lists # fs backend doesn't support start_after
  end
end
