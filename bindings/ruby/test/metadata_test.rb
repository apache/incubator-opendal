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

class MetadataTest < ActiveSupport::TestCase
  setup do
    @op = OpenDal::Operator.new("memory", {})
    @op.write("/file", "OpenDAL Ruby is ready.")
  end

  test "returns metadata" do
    metadata = @op.stat("file")

    assert_nothing_raised do
      metadata.inspect
    end
  end

  test "#file? works" do
    metadata = @op.stat("file")

    assert metadata.file?
  end

  test "#dir? works" do
    metadata = @op.stat("/")
    assert metadata.dir?

    assert !@op.stat("file").dir?
  end

  test "#mode works" do
    metadata = @op.stat("file")
    assert_equal OpenDal::Metadata::FILE, metadata.mode
  end

  test "#content_disposition works" do
    metadata = @op.stat("file")
    assert_nil metadata.content_disposition
  end

  test "#content_length works" do
    metadata = @op.stat("file")
    assert_equal 22, metadata.content_length
  end

  test "#content_md5 works" do
    metadata = @op.stat("file")
    assert_nil metadata.content_md5
  end

  test "#content_type works" do
    metadata = @op.stat("file")
    assert_nil metadata.content_type
  end

  test "#etag works" do
    metadata = @op.stat("file")
    assert_nil metadata.etag
  end
end
