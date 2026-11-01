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

module ErrorTest (errorTests) where

import Control.Monad.IO.Class (MonadIO, liftIO)
import OpenDAL
import Test.Tasty
import Test.Tasty.HUnit

errorTests :: TestTree
errorTests =
  testGroup
    "Error Handling Tests"
    [ testCase "testReadNonExistentFile" testReadNonExistentFile,
      testCase "testStatNonExistentFile" testStatNonExistentFile,
      testCase "testDeleteNonExistentFile" testDeleteNonExistentFile,
      testCase "testCopyNonExistentFile" testCopyNonExistentFile,
      testCase "testRenameNonExistentFile" testRenameNonExistentFile,
      testCase "testListNonExistentDir" testListNonExistentDir,
      testCase "testInvalidOperatorConfig" testInvalidOperatorConfig,
      testCase "testMonadErrorPropagation" testMonadErrorPropagation
    ]

testReadNonExistentFile :: Assertion
testReadNonExistentFile = do
  Right op <- newOperator "memory"
  readOpRaw op "nonexistent-file.txt" >>= \case
    Left err -> errorCode err @?= NotFound
    Right _ -> assertFailure "Expected NotFound error"

testStatNonExistentFile :: Assertion
testStatNonExistentFile = do
  Right op <- newOperator "memory"
  statOpRaw op "nonexistent-file.txt" >>= \case
    Left err -> errorCode err @?= NotFound
    Right _ -> assertFailure "Expected NotFound error"

testDeleteNonExistentFile :: Assertion
testDeleteNonExistentFile = do
  Right op <- newOperator "memory"
  -- Deleting non-existent file should succeed (idempotent)
  deleteOpRaw op "nonexistent-file.txt" ?= Right ()

testCopyNonExistentFile :: Assertion
testCopyNonExistentFile = do
  Right op <- newOperator "memory"
  copyOpRaw op "nonexistent-source.txt" "destination.txt" >>= \case
    Left err -> case errorCode err of
      NotFound -> return () -- Expected behavior
      Unsupported -> putStrLn "Copy operation not supported by memory backend - this is acceptable"
      _ -> assertFailure $ "Expected NotFound or Unsupported error, got: " ++ show err
    Right _ -> assertFailure "Expected error for copying non-existent file"

testRenameNonExistentFile :: Assertion
testRenameNonExistentFile = do
  Right op <- newOperator "memory"
  renameOpRaw op "nonexistent-source.txt" "destination.txt" >>= \case
    Left err -> case errorCode err of
      NotFound -> return () -- Expected behavior
      Unsupported -> putStrLn "Rename operation not supported by memory backend - this is acceptable"
      _ -> assertFailure $ "Expected NotFound or Unsupported error, got: " ++ show err
    Right _ -> assertFailure "Expected error for renaming non-existent file"

testListNonExistentDir :: Assertion
testListNonExistentDir = do
  Right op <- newOperator "memory"
  listOpRaw op "nonexistent-dir/" >>= \case
    Left err -> case errorCode err of
      NotFound -> return () -- Expected behavior
      _ -> assertFailure $ "Expected NotFound error, got: " ++ show err
    Right _ -> putStrLn "Listing non-existent directory succeeded (empty result) - this is acceptable for memory backend"

testInvalidOperatorConfig :: Assertion
testInvalidOperatorConfig = do
  -- Test with invalid scheme
  newOperator "invalid-scheme-that-does-not-exist" >>= \case
    Left err -> case errorCode err of
      ConfigInvalid -> return () -- Expected behavior
      Unsupported -> putStrLn "Invalid scheme returned Unsupported instead of ConfigInvalid - this is acceptable"
      _ -> assertFailure $ "Expected ConfigInvalid or Unsupported error, got: " ++ show err
    Right _ -> assertFailure "Expected error for invalid scheme"

testMonadErrorPropagation :: Assertion
testMonadErrorPropagation = do
  Right op <- newOperator "memory"
  
  -- Test error propagation in monad
  runOp op errorOperation >>= \case
    Left err -> errorCode err @?= NotFound
    Right _ -> assertFailure "Expected error to propagate"
  
  where
    errorOperation = do
      writeOp "test-file" "content"
      _ <- readOp "nonexistent-file" -- This should fail
      writeOp "should-not-reach" "content"

-- helper function

(?=) :: (MonadIO m, Eq a, Show a) => m a -> a -> m ()
result ?= except = result >>= liftIO . (@?= except) 