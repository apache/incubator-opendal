--[[
    Licensed to the Apache Software Foundation (ASF) under one
    or more contributor license agreements.  See the NOTICE file
    distributed with this work for additional information
    regarding copyright ownership.  The ASF licenses this file
    to you under the Apache License, Version 2.0 (the
    "License"); you may not use this file except in compliance
    with the License.  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing,
    software distributed under the License is distributed on an
    "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
    KIND, either express or implied.  See the License for the
    specific language governing permissions and limitations
    under the License.
]]

--- OpenDAL operator.
-- @classmod opendal.operator
-- @pragma nostrip

local _M = {}

--- Construct an operator based on scheme and option. Uses an table of key-value pairs to initialize
--- the operator based on provided scheme and options. For each scheme, i.e. Backend,
--- different options could be set, you may  reference the https://opendal.apache.org/docs/category/services/
--- for each service, especially for the **Configuration Part**.
-- @param string scheme the service scheme you want to specify, e.g. "fs", "s3"
-- @param table options the table to the options for this operators
-- @return table, error  opendal operator table which contain opendal operator instance, error nil if success, others otherwise
-- @function new


--- Blockingly write raw bytes to path.
---  Write the bytes into the path blockingly, returns nil if success, others otherwise
-- @param string path the designated path you want to write your bytes in
-- @param string bytes the bytes to be written
-- @return error nil if success, otherwise error message
-- @function write

--- Blockingly read raw bytes from path.
---  Read the data out from `path` blockingly by operator, returns the string if success, others otherwise
-- @param string path the designated path you want to write your bytes in
-- @return string, error read data, error nil if success, otherwise error message
-- @function read

--- Blockingly delete the object in path.
---  Delete the object in path blockingly, returns error nil
---  if success, others otherwise
-- @param string path the designated path you want to write your delete
-- @return error error nil if success, otherwise error message
-- @function delete

--- Blockingly rename the object of src to dst.
---  Rename the object in path blockingly, returns error nil
---  if success, others otherwise
-- @param string src the designated path you want to rename your source path
-- @param string dst the designated path you want to rename your destination path
-- @return error error nil if success, otherwise error message
-- @function rename

--- Blockingly create the object in path.
---  Create directory the object in path blockingly, returns error nil
---  if success, others otherwise
-- @param string path the designated path you want to create your directory
-- @return error error nil if success, otherwise error message
-- @function create_dir

--- Check whether the path exists.
-- @param string path the designated path you want to write your delete
-- @return bool, error true or false depend on operator instance and path, error nil if success, otherwise error message
-- @function is_exist

--- Stat the path, return its metadata.
-- @param string ,path the designated path you want to write your delete
-- @return table , error opendal.metadata instance table, error nil if success, otherwise error message
-- @function stat


return _M
