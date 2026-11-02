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

class IOTest < ActiveSupport::TestCase
  setup do
    @root = Dir.mktmpdir
    File.write(
      "#{@root}/sample",
      <<~EOF
        Sample data for testing
        A line after title
      EOF
    )
    @op = OpenDal::Operator.new("fs", {"root" => @root})
    @io_read = @op.open("sample", "r")
    @io_write = @op.open("sample_write", "w")
  end

  teardown do
    @io_read.close
    @io_write.close
    FileUtils.remove_entry(@root) if File.exist?(@root)
  end

  test "#binmode? returns" do
    assert_not @io_read.binmode?
  end

  test "#binmode returns" do
    assert_nothing_raised do
      @io_read.binmode
    end
  end

  test "#close closes IO" do
    assert_nothing_raised do
      @io_read.close
    end
  end

  test "#close_read closes reader" do
    assert_nothing_raised do
      @io_read.close_read
    end
  end

  test "#close_write closes writer" do
    assert_nothing_raised do
      @io_read.close_write
    end
  end

  test "#closed? returns" do
    assert_not @io_read.closed?
  end

  test "#closed_read? returns" do
    assert_not @io_read.closed_read?
  end

  test "#closed_write? returns" do
    assert @io_read.closed_write?
  end

  test "#read reads" do
    result = @io_read.read

    assert_equal "Sample data for testing\nA line after title\n", result
    # should be `assert_equal Encoding::UTF_8, result.encoding`
  end

  test "#read reads with nil" do
    result = @io_read.read(nil)

    assert_equal "Sample data for testing\nA line after title\n", result
  end

  test "#read reads with size" do
    result = @io_read.read(6)

    assert_equal "Sample", result
  end

  test "#read raises ArgumentError with -1" do
    assert_raise(ArgumentError) do |err|
      @io_read.read(-1)

      assert_equal "negative length -1 given", err.message
    end
  end

  test "#read reads to a buffer" do
    buffer = +"hi " # unfreezes string literal
    result = @io_read.read(6, buffer)

    assert_equal "Sample", result
    assert_equal "hi Sample", buffer
  end

  test "#write writes" do
    @io_write.write("This is a sentence.")
    @io_write.close
    assert_equal "This is a sentence.", File.read("#{@root}/sample_write")
  end

  test "#readline reads a line" do
    line = @io_read.readline

    assert_equal "Sample data for testing\n", line
    # should be `assert_equal Encoding::UTF_8, line.encoding`
  end

  test "#readlines reads all lines" do
    lines = @io_read.readlines

    assert_equal ["Sample data for testing\n", "A line after title\n"], lines
    # should be `assert_equal Encoding::UTF_8, lines.first.encoding`
  end

  test "#tell returns position" do
    assert_equal 0, @io_read.tell
  end

  test "#pos= moves position" do
    @io_read.pos = 5

    assert_equal 5, @io_read.pos
  end

  test "#rewind moves position to start" do
    @io_read.pos = 5
    @io_read.rewind

    assert_equal 0, @io_read.pos
  end

  test "#eof returns" do
    assert_not @io_read.eof
  end

  test "#eof? returns" do
    @io_read.read
    assert @io_read.eof?
  end

  test "#length returns" do
    assert_equal 43, @io_read.length
  end

  test "#size returns" do
    assert_equal 43, @io_read.size
  end
end
