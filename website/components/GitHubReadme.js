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

/**
 * GitHub README Component: parse all `./xxx` into `GITHUB_URL/xxx`
 */
export default function GitHubReadme({
  children,
  owner = "apache",
  repo = "opendal",
  branch = "main",
  basePath = "",
  components = {},
}) {
  const createGitHubUrl = (path) => {
    const cleanPath = path.replace(/^\.\//, "");
    return `https://github.com/${owner}/${repo}/blob/${branch}/${basePath}${cleanPath}`;
  };

  const CustomLink = (props) => {
    const { href, ...restProps } = props;

    if (href && href.startsWith("./")) {
      return <a {...restProps} href={createGitHubUrl(href)} />;
    }

    return <a {...props} />;
  };

  const CustomParagraph = (props) => {
    const { children } = props;

    if (typeof children === "string") {
      const pattern = /\[(.*?)\]:\s*(\.\/[^\s]+)/g;

      if (pattern.test(children)) {
        const processedText = children.replace(
          pattern,
          (match, label, path) => {
            return `[${label}]: ${createGitHubUrl(path)}`;
          },
        );

        return <p>{processedText}</p>;
      }
    }

    return <p {...props} />;
  };

  const mergedComponents = {
    ...components,
    a: CustomLink,
    p: CustomParagraph,
  };

  const processRawMarkdown = (content) => {
    if (typeof content !== "string") return content;

    let processed = content.replace(
      /\[(.*?)\]\((\.\/[^)]+)\)/g,
      (match, text, path) => `[${text}](${createGitHubUrl(path)})`,
    );

    processed = processed.replace(
      /\[(.*?)\]:\s*(\.\/[^\s]+)/g,
      (match, label, path) => `[${label}]: ${createGitHubUrl(path)}`,
    );

    return processed;
  };

  if (typeof children === "string") {
    return processRawMarkdown(children);
  }

  if (React.isValidElement(children)) {
    return React.cloneElement(children, {
      components: {
        ...(children.props.components || {}),
        ...mergedComponents,
      },
    });
  }

  return children;
}
