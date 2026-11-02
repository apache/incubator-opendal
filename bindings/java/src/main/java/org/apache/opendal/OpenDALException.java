/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

package org.apache.opendal;

/**
 * An OpenDALException encapsulates the error of an operation. This exception
 * type is used to describe an internal error from the native opendal library.
 */
public class OpenDALException extends RuntimeException {
    private final Code code;

    /**
     * Construct an OpenDALException. This constructor is called from native code.
     *
     * @param code string representation of the error code
     * @param message error message
     */
    @SuppressWarnings("unused")
    public OpenDALException(String code, String message) {
        this(Code.valueOf(code), message);
    }

    public OpenDALException(Code code, String message) {
        super(message);
        this.code = code;
    }

    /**
     * Get the error code returned from OpenDAL.
     *
     * @return The error code reported by OpenDAL.
     */
    public Code getCode() {
        return code;
    }

    /**
     * Enumerate all kinds of Error that OpenDAL may return.
     *
     * <p>
     * Read the document of
     * <a href="https://docs.rs/opendal/latest/opendal/enum.ErrorKind.html">opendal::ErrorKind</a>
     * for the details of each code.
     */
    public enum Code {
        Unexpected,
        Unsupported,
        ConfigInvalid,
        NotFound,
        PermissionDenied,
        IsADirectory,
        NotADirectory,
        AlreadyExists,
        RateLimited,
        IsSameFile,
        ConditionNotMatch,
        RangeNotSatisfied,
        Conflict,
    }
}
