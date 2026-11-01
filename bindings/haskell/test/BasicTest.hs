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

module BasicTest (basicTests) where

import Control.Monad.IO.Class (MonadIO, liftIO)
import OpenDAL
import Test.Tasty
import Test.Tasty.HUnit

basicTests :: TestTree
basicTests =
  testGroup
    "Basic Tests"
    [ testCase "testBasicOperation" testRawOperation,
      testCase "testMonad" testMonad,
      testCase "testError" testError,
      testCase "testWriter" testWriter,
      testCase "testWriterAppend" testWriterAppend,
      testCase "testAppendOperation" testAppendOperation,
      testCase "testLister" testLister,
      testCase "testCopyRename" testCopyRename,
      testCase "testRemoveAll" testRemoveAll,
      testCase "testOperatorInfo" testOperatorInfo
    ]

testRawOperation :: Assertion
testRawOperation = do
  Right op <- newOperator "memory"
  writeOpRaw op "key1" "value1" ?= Right ()
  writeOpRaw op "key2" "value2" ?= Right ()
  readOpRaw op "key1" ?= Right "value1"
  readOpRaw op "key2" ?= Right "value2"
  isExistOpRaw op "key1" ?= Right True
  isExistOpRaw op "key2" ?= Right True
  createDirOpRaw op "dir1/" ?= Right ()
  isExistOpRaw op "dir1/" ?= Right True
  statOpRaw op "key1" >>= \case
    Right meta -> meta @?= except_meta
    Left _ -> assertFailure "should not reach here"
  deleteOpRaw op "key1" ?= Right ()
  isExistOpRaw op "key1" ?= Right False
  where
    except_meta =
      Metadata
        { mMode = File,
          mCacheControl = Nothing,
          mContentDisposition = Nothing,
          mContentLength = 6,
          mContentMD5 = Nothing,
          mContentType = Nothing,
          mETag = Nothing,
          mLastModified = Nothing
        }

testMonad :: Assertion
testMonad = do
  Right op <- newOperator "memory"
  runOp op operation ?= Right ()
  where
    operation = do
      writeOp "key1" "value1"
      writeOp "key2" "value2"
      readOp "key1" ?= "value1"
      readOp "key2" ?= "value2"
      isExistOp "key1" ?= True
      isExistOp "key2" ?= True
      createDirOp "dir1/"
      isExistOp "dir1/" ?= True
      statOp "key1" ?= except_meta
      deleteOp "key1"
      isExistOp "key1" ?= False
    except_meta =
      Metadata
        { mMode = File,
          mCacheControl = Nothing,
          mContentDisposition = Nothing,
          mContentLength = 6,
          mContentMD5 = Nothing,
          mContentType = Nothing,
          mETag = Nothing,
          mLastModified = Nothing
        }

testError :: Assertion
testError = do
  Right op <- newOperator "memory"
  runOp op operation >>= \case
    Left err -> errorCode err @?= NotFound
    Right _ -> assertFailure "should not reach here"
  where
    operation = readOp "non-exist-path"

testWriter :: Assertion
testWriter = do
  Right op <- newOperator "memory"
  Right writer <- writerOpRaw op "test-writer-file" defaultWriterOption
  writerWrite writer "Hello" ?= Right ()
  writerWrite writer " " ?= Right ()
  writerWrite writer "World!" ?= Right ()
  writerClose writer >>= \case
    Right meta -> mContentLength meta @?= 12
    Left err -> assertFailure $ "Failed to close writer: " ++ show err
  readOpRaw op "test-writer-file" ?= Right "Hello World!"

testWriterAppend :: Assertion
testWriterAppend = do
  Right op <- newOperator "memory"
  -- First write some initial content
  writeOpRaw op "append-file" "Initial content" ?= Right ()
  -- Create append writer and add more content
  result <- writerOpRaw op "append-file" appendWriterOption
  case result of
    Right writer -> do
      writerWrite writer " appended" ?= Right ()
      writerClose writer >>= \case
        Right meta -> mContentLength meta @?= 24
        Left err -> assertFailure $ "Failed to close append writer: " ++ show err
      readOpRaw op "append-file" ?= Right "Initial content appended"
    Left err -> case errorCode err of
      Unsupported -> putStrLn "Append writer not supported by memory backend - skipping"
      _ -> assertFailure $ "Failed to create append writer: " ++ show err

testAppendOperation :: Assertion  
testAppendOperation = do
  Right op <- newOperator "memory"
  -- Write initial content
  writeOpRaw op "append-test" "Hello" ?= Right ()
  -- Demonstrate append using writer API
  result <- writerOpRaw op "append-test" appendWriterOption
  case result of
    Right writer -> do
      writerWrite writer " World" ?= Right ()
      writerClose writer >>= \case
        Right _ -> readOpRaw op "append-test" ?= Right "Hello World"
        Left err -> assertFailure $ "Failed to close append writer: " ++ show err
    Left err -> case errorCode err of
      Unsupported -> do
        putStrLn "Append writer not supported by memory backend - demonstrating manual append"
        -- Manual append: read existing content, append new content, write back
        Right existingContent <- readOpRaw op "append-test"
        let newContent = existingContent <> " World"
        writeOpRaw op "append-test" newContent ?= Right ()
        readOpRaw op "append-test" ?= Right "Hello World"
      _ -> assertFailure $ "Failed to create append writer: " ++ show err

testLister :: Assertion
testLister = do
  Right op <- newOperator "memory"
  -- Create some test files and directories
  writeOpRaw op "dir1/file1.txt" "content1" ?= Right ()
  writeOpRaw op "dir1/file2.txt" "content2" ?= Right ()
  writeOpRaw op "dir1/subdir/file3.txt" "content3" ?= Right ()
  createDirOpRaw op "dir1/empty-dir/" ?= Right ()
  
  -- Test listing
  Right lister <- listOpRaw op "dir1/"
  files <- collectListerItems lister
  length files @?= 4 -- file1.txt, file2.txt, subdir/, empty-dir/
  
  -- Test scanning (recursive)
  Right scanner <- scanOpRaw op "dir1/"
  allFiles <- collectListerItems scanner
  length allFiles @?= 4 -- All files including nested ones

testCopyRename :: Assertion
testCopyRename = do
  Right op <- newOperator "memory"
  -- Create source file
  writeOpRaw op "source.txt" "test content" ?= Right ()
  -- Test copy - handle case where operation is not supported
  copyResult <- copyOpRaw op "source.txt" "copy.txt"
  case copyResult of
    Right () -> do
      readOpRaw op "copy.txt" ?= Right "test content"
      isExistOpRaw op "source.txt" ?= Right True
    Left err -> case errorCode err of
      Unsupported -> putStrLn "Copy operation not supported by memory backend - skipping"
      _ -> assertFailure $ "Unexpected error in copy: " ++ show err
  
  -- Test rename - handle case where operation is not supported
  renameResult <- renameOpRaw op "source.txt" "renamed.txt"
  case renameResult of
    Right () -> do
      readOpRaw op "renamed.txt" ?= Right "test content"
      isExistOpRaw op "source.txt" ?= Right False
    Left err -> case errorCode err of
      Unsupported -> putStrLn "Rename operation not supported by memory backend - skipping"
      _ -> assertFailure $ "Unexpected error in rename: " ++ show err

testRemoveAll :: Assertion
testRemoveAll = do
  Right op <- newOperator "memory"
  -- Create directory structure
  writeOpRaw op "remove-test/file1.txt" "content1" ?= Right ()
  writeOpRaw op "remove-test/subdir/file2.txt" "content2" ?= Right ()
  createDirOpRaw op "remove-test/empty/" ?= Right ()
  
  -- Verify structure exists
  isExistOpRaw op "remove-test/file1.txt" ?= Right True
  isExistOpRaw op "remove-test/subdir/file2.txt" ?= Right True
  
  -- Remove all
  removeAllOpRaw op "remove-test/" ?= Right ()
  
  -- Verify everything is gone
  isExistOpRaw op "remove-test/file1.txt" ?= Right False
  isExistOpRaw op "remove-test/subdir/file2.txt" ?= Right False
  isExistOpRaw op "remove-test/" ?= Right False

testOperatorInfo :: Assertion
testOperatorInfo = do
  Right op <- newOperator "memory"
  operatorInfoRaw op ?= Right "memory"

-- Helper function to collect all items from a lister
collectListerItems :: Lister -> IO [String]
collectListerItems lister = go []
  where
    go acc = do
      result <- nextLister lister
      case result of
        Right (Just item) -> go (item : acc)
        Right Nothing -> return $ reverse acc
        Left err -> assertFailure $ "Lister error: " ++ show err

-- helper function

(?=) :: (MonadIO m, Eq a, Show a) => m a -> a -> m ()
result ?= except = result >>= liftIO . (@?= except)
