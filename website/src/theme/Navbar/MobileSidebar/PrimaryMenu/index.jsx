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
import { useLocation } from "@docusaurus/router";
import { translate } from "@docusaurus/Translate";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import { useNavbarMobileSidebar } from "@docusaurus/theme-common/internal";
import NavbarItem from "@theme/NavbarItem";
import NavbarNavLink from "@theme/NavbarItem/NavbarNavLink";
import UtilityItems, { isUtilityItem } from "./UtilityItems";

function classNames(...values) {
  return values.filter(Boolean).join(" ");
}

function normalizePath(path) {
  if (!path) {
    return "";
  }

  return path.replace(/\/+$/, "") || "/";
}

function isExternalUrl(value) {
  return /^https?:\/\//i.test(value || "");
}

function isRegexpStringMatch(pattern, text) {
  if (!pattern) {
    return false;
  }

  return new RegExp(pattern).test(text);
}

function isSamePath(path, localPathname) {
  return normalizePath(path) === normalizePath(localPathname);
}

function isItemActive(item, localPathname) {
  if (!isExternalUrl(item.to) && isSamePath(item.to, localPathname)) {
    return true;
  }
  if (isRegexpStringMatch(item.activeBaseRegex, localPathname)) {
    return true;
  }
  if (item.activeBasePath && localPathname.startsWith(item.activeBasePath)) {
    return true;
  }
  return false;
}

function containsActiveItems(items, localPathname) {
  return items.some((item) => isItemActive(item, localPathname));
}

function isAsfDropdownItem(item) {
  return item.label === "ASF" && Array.isArray(item.items);
}

function CollapseButton({ collapsed, onClick }) {
  return (
    <button
      aria-label={
        collapsed
          ? translate({
              id: "theme.navbar.mobileDropdown.collapseButton.expandAriaLabel",
              message: "Expand the dropdown",
              description:
                "The ARIA label of the button to expand the mobile dropdown navbar item",
            })
          : translate({
              id: "theme.navbar.mobileDropdown.collapseButton.collapseAriaLabel",
              message: "Collapse the dropdown",
              description:
                "The ARIA label of the button to collapse the mobile dropdown navbar item",
            })
      }
      aria-expanded={!collapsed}
      type="button"
      className="clean-btn menu__caret"
      onClick={onClick}
    />
  );
}

function useItemCollapsible({ active }) {
  const [collapsed, setCollapsed] = React.useState(() => !active);

  React.useEffect(() => {
    if (active) {
      setCollapsed(false);
    }
  }, [active, setCollapsed]);

  return {
    collapsed,
    toggleCollapsed: () => {
      setCollapsed((value) => !value);
    },
  };
}

function AsfMobileDropdown({ item, onNavigate }) {
  const { pathname } = useLocation();
  const { items, className, ...dropdownProps } = item;
  const props = { ...dropdownProps };
  delete props.position;
  delete props.type;
  delete props.onClick;
  const isActive = isSamePath(props.to, pathname);
  const containsActive = containsActiveItems(items, pathname);
  const { collapsed, toggleCollapsed } = useItemCollapsible({
    active: isActive || containsActive,
  });
  const href = props.to ? undefined : "#";

  return (
    <li
      className={classNames(
        "menu__list-item",
        collapsed && "menu__list-item--collapsed",
      )}
    >
      <div
        className={classNames(
          "menu__list-item-collapsible",
          isActive && "menu__list-item-collapsible--active",
        )}
      >
        <NavbarNavLink
          role="button"
          className={classNames(
            "menu__link menu__link--sublist odl-mobile-drawer-asf-link",
            className,
          )}
          href={href}
          {...props}
          onClick={(event) => {
            if (href === "#") {
              event.preventDefault();
            }
            toggleCollapsed();
          }}
        >
          {props.children ?? props.label}
        </NavbarNavLink>
        <CollapseButton
          collapsed={collapsed}
          onClick={(event) => {
            event.preventDefault();
            toggleCollapsed();
          }}
        />
      </div>

      <ul
        className="menu__list odl-mobile-drawer-asf-items"
        style={{ display: collapsed ? "none" : "block" }}
      >
        {items.map((childItemProps, index) => (
          <NavbarItem
            mobile
            isDropdownItem
            onClick={onNavigate}
            activeClassName="menu__link--active"
            {...childItemProps}
            key={index}
          />
        ))}
      </ul>
    </li>
  );
}

export default function NavbarMobilePrimaryMenu() {
  const { siteConfig } = useDocusaurusContext();
  const mobileSidebar = useNavbarMobileSidebar();
  const items = siteConfig.themeConfig.navbar.items;
  let renderedUtilities = false;

  const handleNavigate = () => {
    mobileSidebar.toggle();
  };

  return (
    <ul className="menu__list">
      {items.map((item, index) => {
        if (isUtilityItem(item)) {
          return null;
        }

        if (isAsfDropdownItem(item)) {
          renderedUtilities = true;
          return (
            <React.Fragment key={index}>
              <AsfMobileDropdown item={item} onNavigate={handleNavigate} />
              <UtilityItems onNavigate={handleNavigate} />
            </React.Fragment>
          );
        }

        return (
          <NavbarItem
            mobile
            {...item}
            onClick={handleNavigate}
            key={index}
          />
        );
      })}
      {!renderedUtilities && <UtilityItems onNavigate={handleNavigate} />}
    </ul>
  );
}
