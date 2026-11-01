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

class OperatorInfoTest < ActiveSupport::TestCase
  setup do
    @op = OpenDal::Operator.new("memory", {})
  end

  test "returns meta information" do
    info = @op.info

    assert_equal "memory", info.scheme
    assert_equal "/", info.root
    assert info.name.length > 0
    assert info.capability.stat
  end
end
