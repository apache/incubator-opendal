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

module OpenDAL.FFI where

import Foreign
import Foreign.C.String
import Foreign.C.Types

data RawOperator

data RawLister

data RawWriter

data FFIResult a = FFIResult
  { ffiCode :: CUInt,
    dataPtr :: Ptr a,
    errorMessage :: CString
  }
  deriving (Show)

instance Storable (FFIResult a) where
  sizeOf _ = sizeOf (undefined :: CSize) + sizeOf (undefined :: Ptr ()) + sizeOf (undefined :: CString)
  alignment _ = alignment (undefined :: CSize)
  peek ptr = do
    s <- peekByteOff ptr codeOffset
    d <- peekByteOff ptr dataPtrOffset
    errMsg <- peekByteOff ptr errorMessageOffset
    return $ FFIResult s d errMsg
    where
      codeOffset = 0
      dataPtrOffset = sizeOf (undefined :: CSize)
      errorMessageOffset = dataPtrOffset + sizeOf (undefined :: Ptr ())
  poke ptr (FFIResult s d errMsg) = do
    pokeByteOff ptr codeOffset s
    pokeByteOff ptr dataPtrOffset d
    pokeByteOff ptr errorMessageOffset errMsg
    where
      codeOffset = 0
      dataPtrOffset = sizeOf (undefined :: CSize)
      errorMessageOffset = dataPtrOffset + sizeOf (undefined :: Ptr ())

data ByteSlice = ByteSlice
  { bsData :: Ptr CChar,
    bsLen :: CSize
  }

instance Storable ByteSlice where
  sizeOf _ = sizeOf (undefined :: Ptr CChar) + sizeOf (undefined :: CSize)
  alignment _ = alignment (undefined :: CSize)
  peek ptr = do
    bsDataPtr <- peekByteOff ptr dataOffset
    len <- peekByteOff ptr lenOffset
    return $ ByteSlice bsDataPtr len
    where
      dataOffset = 0
      lenOffset = sizeOf (undefined :: Ptr ())
  poke ptr (ByteSlice bsDataPtr len) = do
    pokeByteOff ptr dataOffset bsDataPtr
    pokeByteOff ptr lenOffset len
    where
      dataOffset = 0
      lenOffset = sizeOf (undefined :: Ptr ())

data FFIMetadata = FFIMetadata
  { ffiMode :: CUInt,
    ffiCacheControl :: CString,
    ffiContentDisposition :: CString,
    ffiContentLength :: CULong,
    ffiContentMD5 :: CString,
    ffiContentType :: CString,
    ffiETag :: CString,
    ffiLastModified :: CString
  }
  deriving (Show)

instance Storable FFIMetadata where
  sizeOf _ = sizeOf (undefined :: CSize) + sizeOf (undefined :: CString) * 6 + sizeOf (undefined :: CULong)
  alignment _ = alignment (undefined :: CSize)
  peek ptr = do
    mode <- peekByteOff ptr modeOffset
    cacheControl <- peekByteOff ptr cacheControlOffset
    contentDisposition <- peekByteOff ptr contentDispositionOffset
    contentLength <- peekByteOff ptr contentLengthOffset
    contentMD5 <- peekByteOff ptr contentMD5Offset
    contentType <- peekByteOff ptr contentTypeOffset
    eTag <- peekByteOff ptr eTagOffset
    lastModified <- peekByteOff ptr lastModifiedOffset
    return $ FFIMetadata mode cacheControl contentDisposition contentLength contentMD5 contentType eTag lastModified
    where
      modeOffset = 0
      cacheControlOffset = modeOffset + sizeOf (undefined :: CSize)
      contentDispositionOffset = cacheControlOffset + sizeOf (undefined :: CString)
      contentLengthOffset = contentDispositionOffset + sizeOf (undefined :: CString)
      contentMD5Offset = contentLengthOffset + sizeOf (undefined :: CULong)
      contentTypeOffset = contentMD5Offset + sizeOf (undefined :: CString)
      eTagOffset = contentTypeOffset + sizeOf (undefined :: CString)
      lastModifiedOffset = eTagOffset + sizeOf (undefined :: CString)
  poke ptr (FFIMetadata mode cacheControl contentDisposition contentLength contentMD5 contentType eTag lastModified) = do
    pokeByteOff ptr modeOffset mode
    pokeByteOff ptr cacheControlOffset cacheControl
    pokeByteOff ptr contentDispositionOffset contentDisposition
    pokeByteOff ptr contentLengthOffset contentLength
    pokeByteOff ptr contentMD5Offset contentMD5
    pokeByteOff ptr contentTypeOffset contentType
    pokeByteOff ptr eTagOffset eTag
    pokeByteOff ptr lastModifiedOffset lastModified
    where
      modeOffset = 0
      cacheControlOffset = modeOffset + sizeOf (undefined :: CSize)
      contentDispositionOffset = cacheControlOffset + sizeOf (undefined :: CString)
      contentLengthOffset = contentDispositionOffset + sizeOf (undefined :: CString)
      contentMD5Offset = contentLengthOffset + sizeOf (undefined :: CULong)
      contentTypeOffset = contentMD5Offset + sizeOf (undefined :: CString)
      eTagOffset = contentTypeOffset + sizeOf (undefined :: CString)
      lastModifiedOffset = eTagOffset + sizeOf (undefined :: CString)

foreign import ccall "via_map_ffi"
  c_via_map_ffi ::
    CString -> Ptr CString -> Ptr CString -> CSize -> FunPtr (CUInt -> CString -> IO ()) -> Ptr (FFIResult RawOperator) -> IO ()

foreign import ccall "wrapper"
  wrapLogFn :: (CUInt -> CString -> IO ()) -> IO (FunPtr (CUInt -> CString -> IO ()))

foreign import ccall "&free_operator" c_free_operator :: FunPtr (Ptr RawOperator -> IO ())

foreign import ccall "free_byteslice" c_free_byteslice :: Ptr CChar -> CSize -> IO ()

foreign import ccall "blocking_read" c_blocking_read :: Ptr RawOperator -> CString -> Ptr (FFIResult ByteSlice) -> IO ()

foreign import ccall "blocking_write" c_blocking_write :: Ptr RawOperator -> CString -> Ptr CChar -> CSize -> Ptr (FFIResult ()) -> IO ()

foreign import ccall "blocking_is_exist" c_blocking_is_exist :: Ptr RawOperator -> CString -> Ptr (FFIResult CBool) -> IO ()

foreign import ccall "blocking_create_dir" c_blocking_create_dir :: Ptr RawOperator -> CString -> Ptr (FFIResult ()) -> IO ()

foreign import ccall "blocking_copy" c_blocking_copy :: Ptr RawOperator -> CString -> CString -> Ptr (FFIResult ()) -> IO ()

foreign import ccall "blocking_rename" c_blocking_rename :: Ptr RawOperator -> CString -> CString -> Ptr (FFIResult ()) -> IO ()

foreign import ccall "blocking_delete" c_blocking_delete :: Ptr RawOperator -> CString -> Ptr (FFIResult ()) -> IO ()

foreign import ccall "blocking_stat" c_blocking_stat :: Ptr RawOperator -> CString -> Ptr (FFIResult FFIMetadata) -> IO ()

foreign import ccall "blocking_list" c_blocking_list :: Ptr RawOperator -> CString -> Ptr (FFIResult (Ptr RawLister)) -> IO ()

foreign import ccall "blocking_scan" c_blocking_scan :: Ptr RawOperator -> CString -> Ptr (FFIResult (Ptr RawLister)) -> IO ()

foreign import ccall "lister_next" c_lister_next :: Ptr RawLister -> Ptr (FFIResult CString) -> IO ()

foreign import ccall "&free_lister" c_free_lister :: FunPtr (Ptr RawLister -> IO ())


foreign import ccall "operator_info" c_operator_info :: Ptr RawOperator -> Ptr (FFIResult CString) -> IO ()

foreign import ccall "blocking_writer" c_blocking_writer :: Ptr RawOperator -> CString -> Ptr (FFIResult (Ptr RawWriter)) -> IO ()

foreign import ccall "blocking_writer_append" c_blocking_writer_append :: Ptr RawOperator -> CString -> Ptr (FFIResult (Ptr RawWriter)) -> IO ()

foreign import ccall "writer_write" c_writer_write :: Ptr RawWriter -> Ptr CChar -> CSize -> Ptr (FFIResult ()) -> IO ()

foreign import ccall "writer_close" c_writer_close :: Ptr RawWriter -> Ptr (FFIResult FFIMetadata) -> IO ()

foreign import ccall "&free_writer" c_free_writer :: FunPtr (Ptr RawWriter -> IO ())

foreign import ccall "blocking_remove_all" c_blocking_remove_all :: Ptr RawOperator -> CString -> Ptr (FFIResult ()) -> IO ()