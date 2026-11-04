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

using System.Runtime.InteropServices;
using System.Runtime.CompilerServices;
using OpenDAL.Interop.Buffers;
using OpenDAL.Interop.Result;
using OpenDAL.Options;

namespace OpenDAL;

internal partial class NativeMethods
{
    const string __DllName = "opendal_dotnet";

    #region Operator Lifecycle

    [LibraryImport(__DllName, EntryPoint = "operator_construct", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALOperatorResult operator_construct(
        string scheme,
        IntPtr* optionKeys,
        IntPtr* optionValues,
        nuint optionLen,
        IntPtr executor
    );

    [LibraryImport(__DllName, EntryPoint = "operator_free")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void operator_free(IntPtr op);

    [LibraryImport(__DllName, EntryPoint = "operator_info_get")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorInfoResult operator_info_get(Operator op);

    [LibraryImport(__DllName, EntryPoint = "operator_duplicate")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_duplicate(Operator op);

    #endregion

    #region Layer

    [LibraryImport(__DllName, EntryPoint = "operator_layer_retry")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_layer_retry(
        Operator op,
        [MarshalAs(UnmanagedType.I1)] bool jitter,
        float factor,
        ulong minDelayNanos,
        ulong maxDelayNanos,
        nuint maxTimes
    );

    [LibraryImport(__DllName, EntryPoint = "operator_layer_concurrent_limit")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_layer_concurrent_limit(
        Operator op,
        nuint permits,
        nuint httpPermits,
        [MarshalAs(UnmanagedType.I1)] bool hasHttpPermits
    );

    [LibraryImport(__DllName, EntryPoint = "operator_layer_capability_override", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_layer_capability_override(
        Operator op,
        string overrides
    );

    [LibraryImport(__DllName, EntryPoint = "operator_layer_timeout")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_layer_timeout(
        Operator op,
        ulong timeoutNanos,
        ulong ioTimeoutNanos
    );

    [LibraryImport(__DllName, EntryPoint = "operator_layer_throttle")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_layer_throttle(
        Operator op,
        uint bandwidth,
        uint burst
    );

    [LibraryImport(__DllName, EntryPoint = "operator_layer_mime_guess")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALOperatorResult operator_layer_mime_guess(
        Operator op
    );

    #endregion

    #region Buffer

    [LibraryImport(__DllName, EntryPoint = "read_buffer_copy_to")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial nuint read_buffer_copy_to(
        IntPtr handle,
        nuint sourceOffset,
        byte* destination,
        nuint destinationLen
    );

    [LibraryImport(__DllName, EntryPoint = "read_buffer_chunks")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial nuint read_buffer_chunks(
        IntPtr handle,
        OpenDALChunk* chunks,
        nuint cap
    );

    [LibraryImport(__DllName, EntryPoint = "write_buffer_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALWriteResult write_buffer_create(nuint capacity);

    [LibraryImport(__DllName, EntryPoint = "write_buffer_add_segment")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALWriteResult write_buffer_add_segment(
        IntPtr handle,
        nuint committedInCurrent,
        nuint minCapacity
    );

    [LibraryImport(__DllName, EntryPoint = "write_buffer_free")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void write_buffer_free(IntPtr handle);

    #endregion

    #region IO Operations

    #region Write

    [LibraryImport(__DllName, EntryPoint = "operator_write_bytes_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALMetadataResult operator_write_bytes_with_options(
        Operator op,
        string path,
        [In] byte[] data,
        nuint len,
        OpenDALWriteOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_write_bytes_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_write_bytes_with_options_async(
        Operator op,
        string path,
        [In] byte[] data,
        nuint len,
        OpenDALWriteOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALMetadataResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_write_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALMetadataResult operator_write_with_options(
        Operator op,
        string path,
        IntPtr buffer,
        nuint committedInCurrent,
        OpenDALWriteOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_write_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_write_with_options_async(
        Operator op,
        string path,
        IntPtr buffer,
        nuint committedInCurrent,
        OpenDALWriteOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALMetadataResult, void> callback,
        long context
    );

    #endregion

    #region Read

    [LibraryImport(__DllName, EntryPoint = "operator_read_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALReadResult operator_read_with_options(
        Operator op,
        string path,
        OpenDALReadOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_read_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_read_with_options_async(
        Operator op,
        string path,
        OpenDALReadOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALReadResult, void> callback,
        long context
    );

    #endregion

    #region Stat

    [LibraryImport(__DllName, EntryPoint = "operator_stat_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALMetadataResult operator_stat_with_options(
        Operator op,
        string path,
        OpenDALStatOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_stat_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_stat_with_options_async(
        Operator op,
        string path,
        OpenDALStatOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALMetadataResult, void> callback,
        long context
    );

    #endregion

    #region Exists

    [LibraryImport(__DllName, EntryPoint = "operator_exists", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALBoolResult operator_exists(
        Operator op,
        string path
    );

    [LibraryImport(__DllName, EntryPoint = "operator_exists_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_exists_async(
        Operator op,
        string path,
        delegate* unmanaged[Cdecl]<long, OpenDALBoolResult, void> callback,
        long context
    );

    #endregion

    #region Check

    [LibraryImport(__DllName, EntryPoint = "operator_check")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALResult operator_check(Operator op);

    [LibraryImport(__DllName, EntryPoint = "operator_check_async")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_check_async(
        Operator op,
        delegate* unmanaged[Cdecl]<long, OpenDALResult, void> callback,
        long context
    );

    #endregion

    #region List

    [LibraryImport(__DllName, EntryPoint = "operator_list_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALEntryListResult operator_list_with_options(
        Operator op,
        string path,
        OpenDALListOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_list_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_list_with_options_async(
        Operator op,
        string path,
        OpenDALListOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALEntryListResult, void> callback,
        long context
    );

    #endregion

    #region Delete

    [LibraryImport(__DllName, EntryPoint = "operator_delete_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_delete_with_options(
        Operator op,
        string path,
        OpenDALDeleteOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_delete_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_delete_with_options_async(
        Operator op,
        string path,
        OpenDALDeleteOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALResult, void> callback,
        long context
    );

    #endregion

    #region CreateDir

    [LibraryImport(__DllName, EntryPoint = "operator_create_dir", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALResult operator_create_dir(
        Operator op,
        string path
    );

    [LibraryImport(__DllName, EntryPoint = "operator_create_dir_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_create_dir_async(
        Operator op,
        string path,
        delegate* unmanaged[Cdecl]<long, OpenDALResult, void> callback,
        long context
    );

    #endregion

    #region Copy

    [LibraryImport(__DllName, EntryPoint = "operator_copy_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALMetadataResult operator_copy_with_options(
        Operator op,
        string sourcePath,
        string targetPath,
        OpenDALCopyOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_copy_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_copy_with_options_async(
        Operator op,
        string sourcePath,
        string targetPath,
        OpenDALCopyOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALMetadataResult, void> callback,
        long context
    );

    #endregion

    #region Rename

    [LibraryImport(__DllName, EntryPoint = "operator_rename_with_options", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_rename_with_options(
        Operator op,
        string sourcePath,
        string targetPath,
        OpenDALRenameOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_rename_with_options_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_rename_with_options_async(
        Operator op,
        string sourcePath,
        string targetPath,
        OpenDALRenameOptions* options,
        delegate* unmanaged[Cdecl]<long, OpenDALResult, void> callback,
        long context
    );

    #endregion

    #region Presign

    [LibraryImport(__DllName, EntryPoint = "operator_presign_read_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_presign_read_async(
        Operator op,
        string path,
        ulong expireNanos,
        delegate* unmanaged[Cdecl]<long, OpenDALPresignedRequestResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_presign_write_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_presign_write_async(
        Operator op,
        string path,
        ulong expireNanos,
        delegate* unmanaged[Cdecl]<long, OpenDALPresignedRequestResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_presign_stat_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_presign_stat_async(
        Operator op,
        string path,
        ulong expireNanos,
        delegate* unmanaged[Cdecl]<long, OpenDALPresignedRequestResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_presign_delete_async", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_presign_delete_async(
        Operator op,
        string path,
        ulong expireNanos,
        delegate* unmanaged[Cdecl]<long, OpenDALPresignedRequestResult, void> callback,
        long context
    );

    #endregion

    #region Streams

    [LibraryImport(__DllName, EntryPoint = "operator_input_stream_create", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALOperatorResult operator_input_stream_create(
        Operator op,
        string path,
        OpenDALReadOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_input_stream_read_next")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALReadResult operator_input_stream_read_next(IntPtr stream);

    [LibraryImport(__DllName, EntryPoint = "operator_input_stream_read_next_async")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_input_stream_read_next_async(
        IntPtr stream,
        delegate* unmanaged[Cdecl]<long, OpenDALReadResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_input_stream_free")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void operator_input_stream_free(IntPtr stream);

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_create", StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALOperatorResult operator_output_stream_create(
        Operator op,
        string path,
        OpenDALWriteOptions* options
    );

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_write")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALResult operator_output_stream_write(
        IntPtr stream,
        [In] byte[] data,
        nuint len
    );

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_write_async")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_output_stream_write_async(
        IntPtr stream,
        [In] byte[] data,
        nuint len,
        delegate* unmanaged[Cdecl]<long, OpenDALResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_flush")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALResult operator_output_stream_flush(IntPtr stream);

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_close")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALResult operator_output_stream_close(IntPtr stream);

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_close_async")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial OpenDALResult operator_output_stream_close_async(
        IntPtr stream,
        delegate* unmanaged[Cdecl]<long, OpenDALResult, void> callback,
        long context
    );

    [LibraryImport(__DllName, EntryPoint = "operator_output_stream_free")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void operator_output_stream_free(IntPtr stream);

    #endregion

    #endregion

    #region Executor

    [LibraryImport(__DllName, EntryPoint = "executor_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial OpenDALExecutorResult executor_create(nuint threads);

    [LibraryImport(__DllName, EntryPoint = "executor_free")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void executor_free(IntPtr executor);

    #endregion

    #region Result Release

    [LibraryImport(__DllName, EntryPoint = "opendal_error_release")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void opendal_error_release(OpenDALError error);

    [LibraryImport(__DllName, EntryPoint = "opendal_operator_info_result_release")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void opendal_operator_info_result_release(OpenDALOperatorInfoResult result);

    [LibraryImport(__DllName, EntryPoint = "opendal_metadata_result_release")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void opendal_metadata_result_release(OpenDALMetadataResult result);

    [LibraryImport(__DllName, EntryPoint = "opendal_entry_list_result_release")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void opendal_entry_list_result_release(OpenDALEntryListResult result);

    [LibraryImport(__DllName, EntryPoint = "opendal_read_result_release")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void opendal_read_result_release(OpenDALReadResult result);

    [LibraryImport(__DllName, EntryPoint = "opendal_presigned_request_result_release")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void opendal_presigned_request_result_release(OpenDALPresignedRequestResult result);

    #endregion

}
