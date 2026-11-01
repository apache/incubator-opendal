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

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.concurrent.atomic.AtomicReference;
import lombok.experimental.UtilityClass;

/**
 * Utility for loading the native library.
 */
@UtilityClass
public class NativeLibrary {
    private enum LibraryState {
        NOT_LOADED,
        LOADING,
        LOADED
    }

    private static final AtomicReference<LibraryState> libraryLoaded = new AtomicReference<>(LibraryState.NOT_LOADED);

    static {
        NativeLibrary.loadLibrary();
    }

    /**
     * Try load the native library from the following locations:
     *
     * <ol>
     *     <li>
     *         Load from the system dynamic library (<code>opendal_java</code>),
     *         the search path can be configured via <code>-Djava.library.path</code>.
     *     </li>
     *     <li>
     *         Load from the bundled library in the classpath (<code>/native/{classifier}/{libraryName}</code>).
     *         You can use the prebuilt library:
     *         <ul>
     *             <li>org.apache.opendal:opendal-{version}-linux-x86_64</li>
     *             <li>org.apache.opendal:opendal-{version}-linux-x86_64-musl</li>
     *             <li>org.apache.opendal:opendal-{version}-linux-aarch_64</li>
     *             <li>org.apache.opendal:opendal-{version}-linux-aarch_64-musl</li>
     *             <li>org.apache.opendal:opendal-{version}-osx-x86_64</li>
     *             <li>org.apache.opendal:opendal-{version}-osx-aarch_64</li>
     *             <li>org.apache.opendal:opendal-{version}-windows-x86_64</li>
     *         </ul>
     *     </li>
     * </ol>
     */
    public static void loadLibrary() {
        if (libraryLoaded.get() == LibraryState.LOADED) {
            return;
        }

        if (libraryLoaded.compareAndSet(LibraryState.NOT_LOADED, LibraryState.LOADING)) {
            try {
                doLoadLibrary();
            } catch (IOException e) {
                libraryLoaded.set(LibraryState.NOT_LOADED);
                throw new UncheckedIOException("Unable to load the OpenDAL shared library", e);
            } catch (UnsatisfiedLinkError e) {
                libraryLoaded.set(LibraryState.NOT_LOADED);
                throw e;
            }
            libraryLoaded.set(LibraryState.LOADED);
            return;
        }

        while (libraryLoaded.get() == LibraryState.LOADING) {
            try {
                Thread.sleep(10);
            } catch (InterruptedException ignore) {
            }
        }
    }

    private static void doLoadLibrary() throws IOException {
        try {
            // try dynamic library - the search path can be configured via "-Djava.library.path"
            System.loadLibrary("opendal_java");
            return;
        } catch (UnsatisfiedLinkError ignore) {
            // ignore - try from classpath
        }

        doLoadBundledLibrary();
    }

    private static void doLoadBundledLibrary() throws IOException {
        final String libraryPath = bundledLibraryPath();
        UnsatisfiedLinkError linkError = null;
        try (final InputStream is = NativeObject.class.getResourceAsStream(libraryPath)) {
            if (is != null) {
                final int dot = libraryPath.indexOf('.');
                final File tmpFile = File.createTempFile(libraryPath.substring(0, dot), libraryPath.substring(dot));
                tmpFile.deleteOnExit();
                Files.copy(is, tmpFile.toPath(), StandardCopyOption.REPLACE_EXISTING);
                try {
                    System.load(tmpFile.getAbsolutePath());
                    return;
                } catch (UnsatisfiedLinkError e) {
                    linkError = e;
                }
            }
        }

        final String fallbackLibraryPath = fallbackBundledLibraryPath();
        if (fallbackLibraryPath == null) {
            if (linkError != null) {
                throw linkError;
            }
            throw new IOException("cannot find " + libraryPath);
        }
        try (final InputStream is = NativeObject.class.getResourceAsStream(fallbackLibraryPath)) {
            if (is == null) {
                if (linkError != null) {
                    throw linkError;
                }
                throw new IOException("cannot find " + libraryPath);
            }
            final int dot = fallbackLibraryPath.indexOf('.');
            final File tmpFile =
                    File.createTempFile(fallbackLibraryPath.substring(0, dot), fallbackLibraryPath.substring(dot));
            tmpFile.deleteOnExit();
            Files.copy(is, tmpFile.toPath(), StandardCopyOption.REPLACE_EXISTING);
            System.load(tmpFile.getAbsolutePath());
        }
    }

    private static String bundledLibraryPath() {
        final String classifier = Environment.getClassifier();
        final String libraryName = System.mapLibraryName("opendal_java");
        return "/native/" + classifier + "/" + libraryName;
    }

    private static String fallbackBundledLibraryPath() {
        final String classifier = Environment.getClassifier();
        if (!classifier.startsWith("linux-")) {
            return null;
        }
        final String libraryName = System.mapLibraryName("opendal_java");
        if (classifier.endsWith("-musl")) {
            final String gnu = classifier.substring(0, classifier.length() - "-musl".length());
            return "/native/" + gnu + "/" + libraryName;
        }
        return "/native/" + classifier + "-musl/" + libraryName;
    }
}
