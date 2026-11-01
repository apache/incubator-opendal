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

import React, {
  useCallback,
  useEffect,
  useState,
} from "react";
import { useLocation } from "@docusaurus/router";
import useIsBrowser from "@docusaurus/useIsBrowser";
import { HighlightSearchResults } from "docusaurus-lunr-search/src/theme/SearchBar/HighlightSearchResults";
import useLunrSearch from "./useLunrSearch";

function classNames(...values) {
  return values
    .flatMap((value) => {
      if (!value) {
        return [];
      }
      if (typeof value === "string") {
        return [value];
      }
      return Object.entries(value)
        .filter(([, enabled]) => enabled)
        .map(([className]) => className);
    })
    .join(" ");
}

/* Shortcut hints are desktop-only; mobile search should not imply a keyboard. */
const DESKTOP_SEARCH_SHORTCUT_QUERY =
  "(min-width: 997px) and (hover: hover) and (pointer: fine)";

function getDesktopSearchShortcut() {
  const platform =
    window.navigator.userAgentData?.platform || window.navigator.platform || "";

  return /mac/i.test(platform) ? "Cmd+K" : "Ctrl+K";
}

function getSearchPlaceholder(desktopShortcutMedia) {
  if (!desktopShortcutMedia?.matches) {
    return "Search";
  }

  return `Search (${getDesktopSearchShortcut()})`;
}

export default function SearchBar(props) {
  const [placeholder, setPlaceholder] = useState("Search");
  const [isMobileLayout, setIsMobileLayout] = useState(false);
  const location = useLocation();
  const isBrowser = useIsBrowser();
  const inputId = props.inputId || "search_input_react";
  const inputSelector = `#${inputId}`;
  const showSearchIcon = props.showIcon !== false;
  const shouldHideSearchBar =
    (!props.standalone && /\/search\/?$/.test(location.pathname)) ||
    (isMobileLayout && !props.mobile && !props.standalone);
  const { indexReady, loadSearch, searchBarRef } = useLunrSearch({
    disabled: shouldHideSearchBar,
    inputSelector,
  });

  const toggleSearchIconClick = useCallback(
    (event) => {
      if (
        searchBarRef.current &&
        !searchBarRef.current.contains(event.target)
      ) {
        searchBarRef.current.focus();
      }

      props.handleSearchBarToggle?.(!props.isSearchBarExpanded);
    },
    [props],
  );

  useEffect(() => {
    loadSearch();
  }, [loadSearch]);

  useEffect(() => {
    if (!isBrowser) {
      return undefined;
    }

    const desktopShortcutMedia = window.matchMedia?.(
      DESKTOP_SEARCH_SHORTCUT_QUERY,
    );
    const updatePlaceholder = () => {
      setPlaceholder(getSearchPlaceholder(desktopShortcutMedia));
    };

    updatePlaceholder();

    if (desktopShortcutMedia?.addEventListener) {
      desktopShortcutMedia.addEventListener("change", updatePlaceholder);
      return () => {
        desktopShortcutMedia.removeEventListener("change", updatePlaceholder);
      };
    }

    if (desktopShortcutMedia?.addListener) {
      desktopShortcutMedia.addListener(updatePlaceholder);
      return () => {
        desktopShortcutMedia.removeListener(updatePlaceholder);
      };
    }

    return undefined;
  }, [isBrowser]);

  useEffect(() => {
    if (!isBrowser) {
      return undefined;
    }

    const mobileMedia = window.matchMedia?.("(max-width: 996px)");
    const updateMobileLayout = () => {
      setIsMobileLayout(Boolean(mobileMedia?.matches));
    };

    updateMobileLayout();

    if (mobileMedia?.addEventListener) {
      mobileMedia.addEventListener("change", updateMobileLayout);
      return () => {
        mobileMedia.removeEventListener("change", updateMobileLayout);
      };
    }

    if (mobileMedia?.addListener) {
      mobileMedia.addListener(updateMobileLayout);
      return () => {
        mobileMedia.removeListener(updateMobileLayout);
      };
    }

    return undefined;
  }, [isBrowser]);

  useEffect(() => {
    if (props.autoFocus && indexReady) {
      searchBarRef.current?.focus();
    }
  }, [indexReady, props.autoFocus]);

  if (shouldHideSearchBar) {
    return null;
  }

  return (
    <div
      className={classNames("navbar__search", {
        "navbar__search--no-icon": !showSearchIcon,
      })}
      key="search-box"
    >
      {showSearchIcon && (
        <span
          aria-label="expand searchbar"
          role="button"
          className={classNames("search-icon", {
            "search-icon-hidden": props.isSearchBarExpanded,
          })}
          onClick={toggleSearchIconClick}
          onKeyDown={toggleSearchIconClick}
          tabIndex={0}
        />
      )}
      <input
        id={inputId}
        type="search"
        placeholder={placeholder}
        aria-label="Search"
        className={classNames(
          "navbar__search-input",
          { "search-bar-expanded": props.isSearchBarExpanded },
          { "search-bar": !props.isSearchBarExpanded },
        )}
        onClick={loadSearch}
        onMouseOver={loadSearch}
        onFocus={toggleSearchIconClick}
        onBlur={toggleSearchIconClick}
        ref={searchBarRef}
        readOnly={!indexReady}
        aria-disabled={!indexReady}
      />
      <HighlightSearchResults />
    </div>
  );
}
