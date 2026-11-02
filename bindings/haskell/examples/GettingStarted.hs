-- Licensed to the Apache Software Foundation (ASF) under one
-- or more contributor license agreements.  See the NOTICE file
-- distributed with this work for additional information
-- regarding copyright ownership.  The ASF licenses this file
-- to you under the Apache License, Version 2.0 (the
-- "License"); you may not use this file except in compliance
-- with the License.  You may obtain a copy of the License at
--
--   http://www.apache.org/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing,
-- software distributed under the License is distributed on an
-- "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
-- KIND, either express or implied.  See the License for the
-- specific language governing permissions and limitations
-- under the License.

-- The Getting Started example for the Haskell binding, embedded into the
-- website guide. It runs against the in-memory service with no credentials,
-- so CI can execute it directly. The region between the ANCHOR markers is
-- what the docs show — keep it copy-pasteable.

-- ANCHOR: quickstart
import OpenDAL
import qualified Data.ByteString as BS

main :: IO ()
main = do
  -- newOperator accepts any IsString value; "memory" needs no credentials.
  Right op <- newOperator "memory"

  -- The same verbs work on every service.
  Right () <- writeOpRaw op "hello.txt" "Hello, World!"

  Right bytes <- readOpRaw op "hello.txt"
  putStrLn $ "read " ++ show (BS.length bytes) ++ " bytes"

  Right meta <- statOpRaw op "hello.txt"
  putStrLn $ "size = " ++ show (mContentLength meta) ++ " bytes"

  Right () <- deleteOpRaw op "hello.txt"
  return ()
-- ANCHOR_END: quickstart
