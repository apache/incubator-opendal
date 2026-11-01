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

class CapabilityTest < ActiveSupport::TestCase
  setup do
    @op = OpenDal::Operator.new("memory", nil)
  end

  test "has read capability" do
    capability = @op.capability

    assert_not capability.nil?
    assert capability.read
  end

  test "doesn't respond to undefined capability" do
    capability = @op.capability

    assert_not capability.nil?
    assert_raises(NoMethodError) do
      capability.not_exist
    end
  end
end
