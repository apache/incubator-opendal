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

SERVICE = ENV["OPENDAL_TEST"]

module Service
  class BlockingOperatorTest < ActiveSupport::TestCase
    setup do
      config = config_from_env(SERVICE)

      @op = OpenDal::Operator.new(SERVICE, config)
    end

    test "stat empty path" do
      skip("Not supported") if !@op.capability.stat

      exception = assert_raises(RuntimeError) do
        @op.stat("/invalid-path")
      end

      assert exception.message.start_with?("NotFound")
    end

    test "read and write text" do
      skip("Not supported") if !(@op.capability.write && @op.capability.read)

      @op.write("content", "OpenDAL Ruby is ready.")

      content = @op.read("content")

      assert_equal "OpenDAL Ruby is ready.", content
    end

    test "read and write binary" do
      skip("Not supported") if !(@op.capability.write && @op.capability.read)

      # writes 32-bit signed integers
      @op.write("binary", [67305985, -50462977].pack("l*"))

      bin = @op.read("binary")

      assert_equal [67305985, -50462977], bin.unpack("l*")
    end

    test "stat returns file metadata" do
      skip("Not supported") if !(@op.capability.write && @op.capability.stat)

      @op.write("test_stat", "test")

      stat = @op.stat("test_stat")

      assert stat.is_a?(OpenDal::Metadata)
      assert_equal 4, stat.content_length
      assert stat.file?
    end

    test "create_dir creates directory" do
      skip("Not supported") if !(@op.capability.create_dir && @op.capability.stat)

      @op.create_dir("new/directory/")

      assert_equal @op.stat("new/directory/").mode, OpenDal::Metadata::DIRECTORY
    end

    test "exists returns existence" do
      skip("Not supported") if !(@op.capability.read && @op.capability.write && @op.capability.create_dir)

      @op.write("exist_file", "test")
      @op.create_dir("exist/directory/")

      assert @op.exist?("exist_file")
      assert @op.exist?("exist/directory/")
    end

    test "delete removes file" do
      skip("Not supported") if !(@op.capability.delete && @op.capability.write && @op.capability.stat)

      @op.write("deletion_test", "test")

      assert @op.exist?("deletion_test"), "expect file to exist before deletion"

      @op.delete("deletion_test")
      assert !@op.exist?("/deletion_test"), "expect file not to exist after deletion"
    end

    test "rename renames file" do
      skip("Not supported") if !(@op.capability.rename && @op.capability.write && @op.capability.stat)

      @op.write("rename_test", "test")

      @op.rename("rename_test", "new_name")
      assert !@op.exist?("rename_test"), "expect file not to exist after renaming"
      assert @op.exist?("new_name"), "expect file exist after renaming"
    end

    test "remove_all removes files" do
      skip("Not supported") if !(@op.capability.delete && @op.capability.create_dir)

      @op.create_dir("nested/directory/")
      @op.write("nested/directory/text", "content")

      @op.remove_all("nested")
      assert !@op.exist?("nested/directory/"), "expect file not to exist after removal"
    end

    test "copy copies file" do
      skip("Not supported") if !(@op.capability.write && @op.capability.read && @op.capability.copy)

      @op.write("copy_test", "test")
      @op.copy("copy_test", "copy_destination")

      assert @op.exist?("copy_test"), "expect source file to exist"
      assert @op.exist?("copy_destination"), "expect destination file to exist"
    end

    test "opens an IO" do
      skip("Not supported") if !(@op.capability.write && @op.capability.read)

      @op.write("io_test", "test")

      io = @op.open("/io_test", "rb")
      content = io.read

      assert_not io.closed?
      assert_equal "test", content

      io.close
    end

    test "middleware applies a middleware" do
      @op.middleware(OpenDal::Middleware::Retry.new)

      assert @op.is_a?(OpenDal::Operator)
    end

    private

    def config_from_env(service)
      prefix = "OPENDAL_#{service.upcase}_"
      config = {}
      ENV.each do |key, value|
        if key.start_with?(prefix)
          config[key[prefix.length..].downcase] = value
        end
      end

      disable_random_root = ENV["OPENDAL_DISABLE_RANDOM_ROOT"] == "true"
      unless disable_random_root
        root = config.fetch("root", "/")
        uuid = SecureRandom.uuid
        config["root"] = "#{root}/#{uuid}/"
      end

      config
    end
  end
end
