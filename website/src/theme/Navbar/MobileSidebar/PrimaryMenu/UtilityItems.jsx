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

import React from "react";
import { translate } from "@docusaurus/Translate";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import { useColorMode, useThemeConfig } from "@docusaurus/theme-common";
import IconDarkMode from "@theme/Icon/DarkMode";
import IconLightMode from "@theme/Icon/LightMode";
import IconSystemColorMode from "@theme/Icon/SystemColorMode";

const utilityItemClasses = {
  github: "header-github-link",
  discord: "header-discord-link",
};

export function isUtilityItem(item) {
  return Object.values(utilityItemClasses).includes(item.className);
}

function useUtilityLink(className) {
  const { siteConfig } = useDocusaurusContext();
  return siteConfig.themeConfig.navbar.items.find(
    (item) => item.className === className,
  );
}

function withBaseUrl(baseUrl, path) {
  const normalizedBaseUrl = (baseUrl || "/").replace(/\/+$/, "");
  return `${normalizedBaseUrl}/${path.replace(/^\/+/, "")}`;
}

function getNextColorMode(colorMode, respectPrefersColorScheme) {
  if (!respectPrefersColorScheme) {
    return colorMode === "dark" ? "light" : "dark";
  }

  switch (colorMode) {
    case null:
      return "light";
    case "light":
      return "dark";
    case "dark":
      return null;
    default:
      return "light";
  }
}

function getColorModeLabel(colorMode) {
  switch (colorMode) {
    case null:
      return translate({
        message: "system mode",
        id: "theme.colorToggle.ariaLabel.mode.system",
        description: "The name for the system color mode",
      });
    case "dark":
      return translate({
        message: "dark mode",
        id: "theme.colorToggle.ariaLabel.mode.dark",
        description: "The name for the dark color mode",
      });
    case "light":
    default:
      return translate({
        message: "light mode",
        id: "theme.colorToggle.ariaLabel.mode.light",
        description: "The name for the light color mode",
      });
  }
}

function getColorModeAriaLabel(colorMode) {
  return translate(
    {
      message: "Switch between dark and light mode (currently {mode})",
      id: "theme.colorToggle.ariaLabel",
      description: "The ARIA label for the color mode toggle",
    },
    {
      mode: getColorModeLabel(colorMode),
    },
  );
}

function getColorModeIcon(colorMode) {
  switch (colorMode) {
    case null:
      return IconSystemColorMode;
    case "dark":
      return IconDarkMode;
    case "light":
    default:
      return IconLightMode;
  }
}

function UtilityLink({ item, modifier, label, onClick }) {
  if (!item) {
    return null;
  }

  const href = item.href || item.to || "#";
  return (
    <li className="menu__list-item">
      <a
        className={`menu__link odl-mobile-drawer-utility odl-mobile-drawer-utility--${modifier}`}
        href={href}
        aria-label={item["aria-label"] || label}
        target={item.target}
        rel={item.target === "_blank" ? "noopener noreferrer" : undefined}
        onClick={onClick}
      >
        <span>{label}</span>
      </a>
    </li>
  );
}

function ThemeAction() {
  const { colorModeChoice, setColorMode } = useColorMode();
  const { colorMode } = useThemeConfig();
  const colorModeLabel = getColorModeLabel(colorModeChoice);
  const ColorModeIcon = getColorModeIcon(colorModeChoice);

  if (colorMode.disableSwitch) {
    return null;
  }

  return (
    <li className="menu__list-item">
      <button
        type="button"
        className="clean-btn menu__link odl-mobile-drawer-utility odl-mobile-drawer-utility--theme"
        title={colorModeLabel}
        aria-label={getColorModeAriaLabel(colorModeChoice)}
        onClick={() => {
          setColorMode(
            getNextColorMode(
              colorModeChoice,
              colorMode.respectPrefersColorScheme,
            ),
          );
        }}
      >
        <span
          className="odl-mobile-drawer-utility-icon"
          aria-hidden="true"
        >
          <ColorModeIcon
            className="odl-mobile-drawer-utility-svg"
            height={16}
            width={16}
          />
        </span>
        <span>Theme: {colorModeLabel}</span>
      </button>
    </li>
  );
}

function SearchLink({ href, onClick }) {
  return (
    <li className="menu__list-item">
      <a
        className="menu__link odl-mobile-drawer-utility odl-mobile-drawer-utility--search"
        href={href}
        onClick={onClick}
      >
        <span>Search</span>
      </a>
    </li>
  );
}

export default function UtilityItems({ onNavigate }) {
  const { siteConfig } = useDocusaurusContext();
  const githubItem = useUtilityLink(utilityItemClasses.github);
  const discordItem = useUtilityLink(utilityItemClasses.discord);
  const searchHref = withBaseUrl(siteConfig.baseUrl, "/search");

  return (
    <>
      <li
        className="odl-mobile-drawer-utility-separator"
        role="separator"
        aria-hidden="true"
      />
      <UtilityLink
        item={githubItem}
        modifier="github"
        label="GitHub"
        onClick={onNavigate}
      />
      <UtilityLink
        item={discordItem}
        modifier="discord"
        label="Discord"
        onClick={onNavigate}
      />
      <ThemeAction />
      <SearchLink href={searchHref} onClick={onNavigate} />
    </>
  );
}
