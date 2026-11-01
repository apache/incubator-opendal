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
import { useLocation } from "@docusaurus/router";
import Breadcrumb from "@site/src/components/ui/Breadcrumb";

const ROOTS = {
  docs: "Docs",
  community: "Community",
};

const LABELS = {
  core: "Rust",
  cpp: "C++",
  dotnet: ".NET",
  nodejs: "Node.js",
  ocaml: "OCaml",
  php: "PHP",
  "pmc-members": "PMC Members",
};

function labelFromSegment(segment) {
  if (LABELS[segment]) {
    return LABELS[segment];
  }

  return decodeURIComponent(segment)
    .split(/[-_]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function breadcrumbFromPath(pathname) {
  const segments = pathname.split("/").filter(Boolean);
  const rootIndex = segments.findIndex((segment) => ROOTS[segment]);
  if (rootIndex === -1) {
    return null;
  }

  const root = segments[rootIndex];
  const pathSegments = segments.slice(rootIndex + 1);
  const items = [];

  if (root === "docs" && pathSegments[0] === "core") {
    items.push({ label: labelFromSegment("core"), href: "/docs/core" });
    pathSegments.slice(1).forEach((segment, index) => {
      items.push({
        label: labelFromSegment(segment),
        href: `/docs/core/${pathSegments.slice(1, index + 2).join("/")}`,
      });
    });
  } else if (root === "docs" && pathSegments[0] === "bindings") {
    const binding = pathSegments[1];
    if (binding) {
      items.push({
        label: labelFromSegment(binding),
        href: `/docs/bindings/${binding}`,
      });
      pathSegments.slice(2).forEach((segment, index) => {
        items.push({
          label: labelFromSegment(segment),
          href: `/docs/bindings/${binding}/${pathSegments
            .slice(2, index + 3)
            .join("/")}`,
        });
      });
    }
  } else {
    const current = pathSegments[pathSegments.length - 1] || "overview";
    items.push({ label: labelFromSegment(current) });
  }

  return {
    rootLabel: ROOTS[root],
    rootHref: `/${root}`,
    items,
  };
}

export default function DocBreadcrumbs() {
  const location = useLocation();
  const breadcrumb = breadcrumbFromPath(location.pathname);

  if (!breadcrumb) {
    return null;
  }

  const items = breadcrumb.items.map((item, index) => ({
    ...item,
    href: index === breadcrumb.items.length - 1 ? undefined : item.href,
  }));

  return (
    <Breadcrumb
      aria-label={translate({
        id: "theme.docs.breadcrumbs.navAriaLabel",
        message: "Breadcrumbs",
        description: "The ARIA label for the breadcrumbs",
      })}
      rootLabel={breadcrumb.rootLabel}
      rootHref={breadcrumb.rootHref}
      items={items}
    />
  );
}
