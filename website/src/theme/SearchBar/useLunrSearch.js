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

import { useCallback, useMemo, useRef, useState } from "react";
import { useHistory } from "@docusaurus/router";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import { usePluginData } from "@docusaurus/useGlobalData";
import useIsBrowser from "@docusaurus/useIsBrowser";

function joinAssetUrl(assetUrl, filename) {
  return `${assetUrl.replace(/\/?$/, "/")}${filename.replace(/^\//, "")}`;
}

function getHighlightedWord(suggestion, options) {
  if (!options.highlightResult) {
    return "";
  }

  try {
    const matchedLine =
      suggestion.text || suggestion.subcategory || suggestion.title;
    const matchedWordResult = matchedLine.match(
      new RegExp("<span.+span>\\w*", "g"),
    );
    if (matchedWordResult && matchedWordResult.length > 0) {
      const tempDoc = document.createElement("div");
      tempDoc.innerHTML = matchedWordResult[0];
      return tempDoc.textContent;
    }
  } catch (error) {
    console.log(error);
  }

  return "";
}

export default function useLunrSearch({ inputSelector, disabled }) {
  const initialized = useRef(false);
  const searchBarRef = useRef(null);
  const [indexReady, setIndexReady] = useState(false);
  const history = useHistory();
  const { siteConfig = {} } = useDocusaurusContext();
  const isBrowser = useIsBrowser();
  const pluginData = usePluginData("docusaurus-lunr-search");
  const { baseUrl = "/" } = siteConfig;

  const pluginConfig = useMemo(
    () =>
      (siteConfig.plugins || []).find(
        (plugin) =>
          Array.isArray(plugin) &&
          typeof plugin[0] === "string" &&
          plugin[0].includes("docusaurus-lunr-search"),
      ),
    [siteConfig.plugins],
  );

  const assetUrl = pluginConfig?.[1]?.assetUrl || baseUrl;

  const initSearch = useCallback(
    (searchDocs, searchIndex, DocSearch, options) => {
      if (!searchBarRef.current) {
        return false;
      }

      new DocSearch({
        searchDocs,
        searchIndex,
        baseUrl,
        inputSelector,
        handleSelected: (_input, _event, suggestion) => {
          const url = suggestion.url || "/";
          _input.setVal("");
          _event.target.blur();

          history.push(url, {
            highlightState: {
              wordToHighlight: getHighlightedWord(suggestion, options),
            },
          });
        },
        maxHits: options.maxHits,
      });

      return true;
    },
    [baseUrl, history, inputSelector],
  );

  const loadSearch = useCallback(() => {
    if (!isBrowser || initialized.current || disabled) {
      return;
    }

    const fileNames = pluginData?.fileNames;
    if (!fileNames?.searchDoc || !fileNames?.lunrIndex) {
      initialized.current = true;
      return;
    }

    initialized.current = true;

    Promise.all([
      fetch(joinAssetUrl(assetUrl, fileNames.searchDoc)).then((content) =>
        content.json(),
      ),
      fetch(joinAssetUrl(assetUrl, fileNames.lunrIndex)).then((content) =>
        content.json(),
      ),
      import("docusaurus-lunr-search/src/theme/SearchBar/DocSearch"),
      import("docusaurus-lunr-search/src/theme/SearchBar/algolia.css"),
    ])
      .then(([searchDocFile, searchIndex, { default: DocSearch }]) => {
        const { searchDocs = [], options = {} } = searchDocFile || {};
        if (searchDocs.length === 0) {
          return;
        }

        if (initSearch(searchDocs, searchIndex, DocSearch, options)) {
          setIndexReady(true);
        }
      })
      .catch(() => {
        initialized.current = false;
      });
  }, [assetUrl, disabled, initSearch, isBrowser, pluginData]);

  return {
    indexReady,
    loadSearch,
    searchBarRef,
  };
}
