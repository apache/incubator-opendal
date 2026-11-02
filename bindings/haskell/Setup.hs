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

{-# LANGUAGE CPP #-}

import Control.Monad
import Data.Maybe
import qualified Distribution.PackageDescription as PD
import Distribution.Simple
import Distribution.Simple.LocalBuildInfo
import Distribution.Simple.Setup
import Distribution.Simple.Utils
#if MIN_VERSION_Cabal(3,14,0)
import Distribution.Utils.Path (makeSymbolicPath)
#endif
import Distribution.System
import System.Directory
import System.Environment
import System.Process

main :: IO ()
main =
  defaultMainWithHooks
    simpleUserHooks
      { confHook = rustConfHook,
        buildHook = rustBuildHook
      }

rustConfHook :: (PD.GenericPackageDescription, PD.HookedBuildInfo) -> ConfigFlags -> IO LocalBuildInfo
rustConfHook (description, buildInfo) flags = do
  localBuildInfo <- confHook simpleUserHooks (description, buildInfo) flags
  let packageDescription = localPkgDescr localBuildInfo
      library = fromJust $ PD.library packageDescription
      libraryBuildInfo = PD.libBuildInfo library
  dir <- getLibDir $ fromFlagOrDefault False (configProf flags)
  return
    localBuildInfo
      { localPkgDescr =
          packageDescription
            { PD.library =
                Just $
                  library
                    { PD.libBuildInfo =
                        libraryBuildInfo
                          { PD.extraLibDirs =
#if MIN_VERSION_Cabal(3,14,0)
                              makeSymbolicPath dir : PD.extraLibDirs libraryBuildInfo,
#else
                              dir : PD.extraLibDirs libraryBuildInfo,
#endif
                            PD.ldOptions = ("-Wl,-rpath," ++ dir) : (PD.ldOptions libraryBuildInfo)
                          }
                    }
            }
      }

rustBuildHook :: PD.PackageDescription -> LocalBuildInfo -> UserHooks -> BuildFlags -> IO ()
rustBuildHook pkg_descr lbi hooks flags = do
  putStrLn "Building Rust code..."
  let isRelease = withProfLib lbi
  let cargoArgs = if isRelease then ["build", "--release"] else ["build"]
#if MIN_VERSION_Cabal(3,14,0)
  rawSystemExit (fromFlag $ buildVerbosity flags) Nothing "cargo" cargoArgs
#else
  rawSystemExit (fromFlag $ buildVerbosity flags) "cargo" cargoArgs
#endif
  putStrLn "Build Rust code success!"
  buildHook simpleUserHooks pkg_descr lbi hooks flags

getLibDir :: Bool -> IO String
getLibDir isRelease = do
  cargoPath <- readProcess "cargo" ["locate-project", "--workspace", "--message-format=plain"] ""
  let dir = take (length cargoPath - 11) cargoPath -- <dir>/Cargo.toml -> <dir>
  let targetDir = if isRelease then "release" else "debug"
  return $ dir ++ "target/" ++ targetDir

getDynamicLibExtension :: LocalBuildInfo -> String
getDynamicLibExtension lbi =
  let Platform _ os = hostPlatform lbi
   in case os of
        OSX -> "dylib"
        Linux -> "so"
        Windows -> "dll"
        _ -> error "Unsupported OS"
