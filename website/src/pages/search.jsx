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

import React, { useEffect } from "react";
import { useHistory } from "@docusaurus/router";
import useIsBrowser from "@docusaurus/useIsBrowser";
import Layout from "@theme/Layout";
import SearchBar from "@theme/SearchBar";

export default function SearchPage() {
  const history = useHistory();
  const isBrowser = useIsBrowser();

  useEffect(() => {
    if (!isBrowser) {
      return;
    }

    const desktopMedia = window.matchMedia?.("(min-width: 997px)");
    if (desktopMedia?.matches) {
      history.replace("/");
    }
  }, [history, isBrowser]);

  return (
    <Layout
      title="Search"
      description="Search Apache OpenDAL documentation, services, and project pages."
    >
      <main className="odl-search-page">
        <div className="odl-container">
          <h1>Search</h1>
          <SearchBar
            standalone
            autoFocus
            mobile
            inputId="search_page_input_react"
            isSearchBarExpanded
            showIcon={false}
            handleSearchBarToggle={() => {}}
          />
        </div>
      </main>
    </Layout>
  );
}
