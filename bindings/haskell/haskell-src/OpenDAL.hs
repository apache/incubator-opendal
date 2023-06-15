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

module OpenDAL (
  Operator,
  createOp,
  readOp,
  writeOp,
) where

import Data.ByteString (ByteString)
import qualified Data.ByteString as BS
import Data.HashMap.Strict (HashMap)
import qualified Data.HashMap.Strict as HashMap
import Foreign
import Foreign.C.String
import OpenDAL.FFI

newtype Operator = Operator (Ptr RawOperator)

byteSliceToByteString :: ByteSlice -> IO ByteString
byteSliceToByteString (ByteSlice bsDataPtr len) = BS.packCStringLen (bsDataPtr, fromIntegral len)

-- | Create a new Operator.
createOp :: String -> HashMap String String -> IO (Either String Operator)
createOp scheme hashMap = do
  let keysAndValues = HashMap.toList hashMap
  withCString scheme $ \cScheme ->
    withMany withCString (map fst keysAndValues) $ \cKeys ->
      withMany withCString (map snd keysAndValues) $ \cValues ->
        allocaArray (length keysAndValues) $ \cKeysPtr ->
          allocaArray (length keysAndValues) $ \cValuesPtr ->
            alloca $ \ffiResultPtr -> do
              pokeArray cKeysPtr cKeys
              pokeArray cValuesPtr cValues
              c_via_map_ffi cScheme cKeysPtr cValuesPtr (fromIntegral $ length keysAndValues) ffiResultPtr
              ffiResult <- peek ffiResultPtr
              if success ffiResult
                then do
                  let op = Operator (castPtr $ dataPtr ffiResult)
                  return $ Right op
                else do
                  errMsg <- peekCString (errorMessage ffiResult)
                  return $ Left errMsg

readOp :: Operator -> String -> IO (Either String ByteString)
readOp (Operator op) path = (flip ($)) op $ \opptr ->
  withCString path $ \cPath ->
    alloca $ \ffiResultPtr -> do
      c_blocking_read opptr cPath ffiResultPtr
      ffiResult <- peek ffiResultPtr
      if success ffiResult
        then do
          byteslice <- peek (castPtr $ dataPtr ffiResult)
          byte <- byteSliceToByteString byteslice
          c_free_byteslice (bsData byteslice) (bsLen byteslice)
          return $ Right byte
        else do
          errMsg <- peekCString (errorMessage ffiResult)
          return $ Left errMsg

writeOp :: Operator -> String -> ByteString -> IO (Either String ())
writeOp (Operator op) path byte = (flip ($)) op $ \opptr ->
  withCString path $ \cPath ->
    BS.useAsCStringLen byte $ \(cByte, len) ->
      alloca $ \ffiResultPtr -> do
        c_blocking_write opptr cPath cByte (fromIntegral len) ffiResultPtr
        ffiResult <- peek ffiResultPtr
        if success ffiResult
          then return $ Right ()
          else do
            errMsg <- peekCString (errorMessage ffiResult)
            return $ Left errMsg