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

class MiddlewaresTest < ActiveSupport::TestCase
  test "builds a retry middleware" do
    assert_nothing_raised do
      OpenDal::Middleware::Retry.new
    end
  end

  test "builds a concurrent limit middleware" do
    assert_nothing_raised do
      OpenDal::Middleware::ConcurrentLimit.new(10)
    end
  end

  test "builds a throttle middleware" do
    assert_nothing_raised do
      OpenDal::Middleware::Throttle.new(100, 10)
    end
  end

  test "builds a timeout middleware" do
    assert_nothing_raised do
      OpenDal::Middleware::Timeout.new(1.23, 1.0)
    end
  end
end
