(*
   Licensed to the Apache Software Foundation (ASF) under one
   or more contributor license agreements.  See the NOTICE file
   distributed with this work for additional information
   regarding copyright ownership.  The ASF licenses this file
   to you under the Apache License, Version 2.0 (the
   "License")you may not use this file except in compliance
   with the License.  You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing,
   software distributed under the License is distributed on an
   "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
   KIND, either express or implied.  See the License for the
   specific language governing permissions and limitations
   under the License.
*)

module Operator = Operator

(* Re-export types from Opendal_core for convenience *)
type operator = Opendal_core.Operator.operator
type reader = Opendal_core.Operator.reader
type writer = Opendal_core.Operator.writer
type lister = Opendal_core.Operator.lister
type metadata = Opendal_core.Operator.metadata
type entry = Opendal_core.Operator.entry
type operator_info = Opendal_core.Operator.operator_info
type capability = Opendal_core.Operator.capability
