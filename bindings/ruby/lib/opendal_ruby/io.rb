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
  class IO
    # Reads all lines from the stream into an array.
    # @raise [EOFError] when the end of the file is reached.
    # @return [Array<String>]
    def readlines
      results = []

      loop do
        results << readline
      rescue EOFError
        break
      end

      results
    end

    # Rewinds the stream to the beginning.
    def rewind
      seek(0, ::IO::SEEK_SET)
    end

    # Sets the file position to `new_position`.
    # @param new_position [Integer]
    def pos=(new_position)
      seek(new_position, ::IO::SEEK_SET)
    end

    alias_method :pos, :tell

    # Checks if the stream is at the end of the file.
    # @return [Boolean]
    def eof
      position = tell
      seek(0, ::IO::SEEK_END)
      tell == position
    end

    alias_method :eof?, :eof

    # Returns the total length of the stream.
    # @return [Integer]
    def length
      current_position = tell
      seek(0, ::IO::SEEK_END)
      tell.tap { self.pos = current_position }
    end

    alias_method :size, :length
  end
end
