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

module OpenDal
  class Metadata
    FILE = "File"
    DIRECTORY = "Directory"

    # Returns `True` if this is a file.
    # @return [Boolean]
    def file?
      mode == FILE
    end

    # Returns `True` if this is a directory.
    # @return [Boolean]
    def dir?
      mode == DIRECTORY
    end

    def inspect
      # Be concise to keep a few attributes
      "#<#{self.class.name} mode: #{mode}, \
        content_type: #{content_type}, \
        content_length: #{content_length}>"
    end
  end
end
