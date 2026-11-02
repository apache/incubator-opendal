#!/usr/bin/env python3
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

import os
import sys
import argparse
import json
from datetime import datetime, timedelta
import requests
from dateutil.parser import parse
import pytz
from openai import OpenAI

import logging

logging.basicConfig(level=logging.DEBUG)


def get_github_api_token():
    """Get GitHub API token from environment variables."""
    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print(
            "Warning: GitHub API token not found. Set the GITHUB_TOKEN environment variable."
        )
        print("Without a token, API rate limits will be lower.")
    return token


def get_openai_api_key():
    """Get OpenAI API key from environment variables."""
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print(
            "Error: OpenAI API key not found. Set the OPENAI_API_KEY environment variable."
        )
        sys.exit(1)
    return api_key


def init_openai_client():
    """Initialize the OpenAI client."""
    api_key = get_openai_api_key()
    # Get the OpenAI API base URL from environment variable or use the default
    api_base = os.environ.get("OPENAI_API_BASE", "https://api.openai.com/v1")
    # Get the model from environment variable or use a default
    model = os.environ.get("OPENAI_MODEL", "gpt-4o")

    client = OpenAI(
        api_key=api_key,
        base_url=api_base,
        default_query={"api-version": "2023-05-15"},
    )
    return client, model


def is_recent(timestamp_str, days=7):
    """Check if the timestamp is within the last 'days' days."""
    now = datetime.now(pytz.utc)
    timestamp = parse(timestamp_str)
    delta = now - timestamp
    return delta.days < days


def fetch_issues(repo, token, days=7):
    """Fetch recent issues from a repository."""
    since_date = (datetime.now() - timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")
    headers = {"Accept": "application/vnd.github.v3+json"}
    if token:
        headers["Authorization"] = f"token {token}"

    url = f"https://api.github.com/repos/{repo}/issues"
    params = {
        "state": "all",
        "since": since_date,
        "sort": "updated",
        "direction": "desc",
        "per_page": 100,
    }

    issues = []
    prs = []
    good_first_issues = []

    response = requests.get(url, params=params, headers=headers)
    if response.status_code != 200:
        print(f"Error fetching issues: {response.status_code}")
        return [], [], []

    for item in response.json():
        if is_recent(item["updated_at"], days):
            entry = {
                "id": item["number"],
                "title": item["title"],
                "url": item["html_url"],
                "user": item["user"]["login"],
                "updated_at": item["updated_at"],
                "body": item.get("body", "") or "",
                "labels": [label["name"] for label in item.get("labels", [])],
                "state": item["state"],
                "comments": item["comments"],
            }

            # Check if it's a good first issue
            label_names = [label["name"].lower() for label in item.get("labels", [])]
            is_good_first = any(
                name
                in [
                    "good first issue",
                    "good-first-issue",
                    "beginner friendly",
                    "beginner-friendly",
                    "easy",
                ]
                for name in label_names
            )

            if "pull_request" in item:
                # Get additional PR details
                if token:
                    pr_url = (
                        f"https://api.github.com/repos/{repo}/pulls/{item['number']}"
                    )
                    pr_response = requests.get(pr_url, headers=headers)
                    if pr_response.status_code == 200:
                        pr_data = pr_response.json()
                        entry["additions"] = pr_data.get("additions", 0)
                        entry["deletions"] = pr_data.get("deletions", 0)
                        entry["changed_files"] = pr_data.get("changed_files", 0)
                        entry["mergeable"] = pr_data.get("mergeable", None)
                        entry["draft"] = pr_data.get("draft", False)

                prs.append(entry)
            else:
                issues.append(entry)
                if is_good_first and item["state"] == "open":
                    good_first_issues.append(entry)

    return issues, prs, good_first_issues


def fetch_discussions(repo, token, days=7):
    """Fetch recent discussions from a repository."""
    headers = {"Accept": "application/vnd.github.v3+json"}
    if token:
        headers["Authorization"] = f"token {token}"

    # GraphQL query to fetch discussions
    query = """
    query($owner: String!, $name: String!) {
        repository(owner: $owner, name: $name) {
            discussions(first: 100, orderBy: {field: UPDATED_AT, direction: DESC}) {
                nodes {
                    number
                    title
                    url
                    author {
                        login
                    }
                    updatedAt
                    bodyText
                    category {
                        name
                    }
                    comments {
                        totalCount
                    }
                    answerChosenAt
                }
            }
        }
    }
    """

    owner, name = repo.split("/")
    variables = {"owner": owner, "name": name}

    url = "https://api.github.com/graphql"
    response = requests.post(
        url, json={"query": query, "variables": variables}, headers=headers
    )

    discussions = []
    if response.status_code != 200:
        print(f"Error fetching discussions: {response.status_code}")
        return discussions

    result = response.json()
    if (
        "data" in result
        and "repository" in result["data"]
        and "discussions" in result["data"]["repository"]
    ):
        for discussion in result["data"]["repository"]["discussions"]["nodes"]:
            if is_recent(discussion["updatedAt"], days):
                discussions.append(
                    {
                        "id": discussion["number"],
                        "title": discussion["title"],
                        "url": discussion["url"],
                        "user": discussion["author"]["login"]
                        if discussion["author"]
                        else "Anonymous",
                        "updated_at": discussion["updatedAt"],
                        "body": discussion.get("bodyText", "") or "",
                        "category": discussion.get("category", {}).get(
                            "name", "General"
                        ),
                        "comments": discussion.get("comments", {}).get("totalCount", 0),
                        "answered": discussion.get("answerChosenAt") is not None,
                    }
                )

    return discussions


def fetch_additional_good_first_issues(repo, token, count=5):
    """Fetch additional good first issues even if they're older."""
    headers = {"Accept": "application/vnd.github.v3+json"}
    if token:
        headers["Authorization"] = f"token {token}"

    url = f"https://api.github.com/repos/{repo}/issues"
    params = {
        "state": "open",
        "labels": "good first issue",
        "sort": "updated",
        "direction": "desc",
        "per_page": count,
    }

    additional_issues = []

    # Try with 'good first issue'
    response = requests.get(url, params=params, headers=headers)
    if response.status_code == 200:
        additional_issues.extend(response.json())

    # If we didn't get enough, try with 'good-first-issue'
    if len(additional_issues) < count:
        params["labels"] = "good-first-issue"
        response = requests.get(url, params=params, headers=headers)
        if response.status_code == 200:
            additional_issues.extend(response.json())

    # If still not enough, try with 'beginner friendly'
    if len(additional_issues) < count:
        params["labels"] = "beginner friendly"
        response = requests.get(url, params=params, headers=headers)
        if response.status_code == 200:
            additional_issues.extend(response.json())

    # Format the issues
    formatted_issues = []
    for item in additional_issues[:count]:
        formatted_issues.append(
            {
                "id": item["number"],
                "title": item["title"],
                "url": item["html_url"],
                "user": item["user"]["login"],
                "updated_at": item["updated_at"],
                "body": item.get("body", "") or "",
                "labels": [label["name"] for label in item.get("labels", [])],
                "state": item["state"],
                "comments": item["comments"],
            }
        )

    return formatted_issues


def format_data_for_llm(repo, issues, prs, discussions, good_first_issues, days=7):
    """Format data in a JSON structure that's friendly for LLM processing."""
    now = datetime.now()

    # Combine recent good first issues with additional ones
    # Remove duplicates by creating a dict with issue ID as key
    all_good_first_issues = {}
    for issue in good_first_issues:
        all_good_first_issues[issue["id"]] = issue

    result = {
        "metadata": {
            "repository": repo,
            "date_generated": now.strftime("%Y-%m-%d"),
            "period_days": days,
        },
        "pull_requests": [
            {
                "id": pr["id"],
                "title": pr["title"],
                "url": pr["url"],
                "author": pr["user"],
                "updated_at": pr["updated_at"],
                "description": pr["body"],
                "labels": pr["labels"],
                "state": pr["state"],
                "comments": pr["comments"],
                "additions": pr.get("additions", "unknown"),
                "deletions": pr.get("deletions", "unknown"),
                "changed_files": pr.get("changed_files", "unknown"),
                "draft": pr.get("draft", False),
            }
            for pr in prs
        ],
        "issues": [
            {
                "id": issue["id"],
                "title": issue["title"],
                "url": issue["url"],
                "author": issue["user"],
                "updated_at": issue["updated_at"],
                "description": issue["body"],
                "labels": issue["labels"],
                "state": issue["state"],
                "comments": issue["comments"],
            }
            for issue in issues
        ],
        "discussions": [
            {
                "id": discussion["id"],
                "title": discussion["title"],
                "url": discussion["url"],
                "author": discussion["user"],
                "updated_at": discussion["updated_at"],
                "description": discussion["body"],
                "category": discussion["category"],
                "comments": discussion["comments"],
                "answered": discussion.get("answered", False),
            }
            for discussion in discussions
        ],
        "good_first_issues": [
            {
                "id": issue["id"],
                "title": issue["title"],
                "url": issue["url"],
                "author": issue["user"],
                "updated_at": issue["updated_at"],
                "description": issue["body"],
                "labels": issue["labels"],
                "comments": issue["comments"],
            }
            for issue in all_good_first_issues.values()
        ],
    }

    return result


def summarize_with_openai(data, client, model):
    """Use OpenAI to summarize and prioritize the repository activity."""

    prompt = f"""
    You are an open-source community evangelist responsible for reporting GitHub repository activity and encouraging more contributions.

    I will provide you with JSON data containing recent pull requests, issues, and discussions from
    the repository {data["metadata"]["repository"]} for the past {data["metadata"]["period_days"]} days.

    Please analyze this data and provide:
    1. A concise summary of the overall activity and key themes
    2. The most important ongoing projects or initiatives based on the data
    3. Prioritized issues and PRs that need immediate attention
    4. Major discussions that should be highlighted
    5. Identify any emerging trends or patterns in development

    Additionally, include a section highlighting "Good First Issues" to encourage new contributors to join the project. Summarize what skills might be needed and why these issues are good entry points.

    IMPORTANT: For each PR you mention, ALWAYS include the contributor's GitHub username with @ symbol (e.g., @username) to properly credit their contributions. This is critical for recognizing contributors' work.

    CRITICAL FORMATTING INSTRUCTIONS:

    - When referring to PRs, issues, or discussions, use ONLY the GitHub reference format: #XXXX (number with # prefix)
    - DO NOT include the title after the reference number
    - DO NOT repeat the title in your explanation if you've already mentioned the reference number
    - AVOID listing large numbers of PRs in sequence - instead, summarize them by theme or use bulleted lists with no more than 3-5 items per bullet
    - For groups of related PRs, summarize the theme and mention 1-2 representative examples instead of listing all of them
    - When appropriate, use standard Markdown URL syntax [meaningful text](full link) instead of just the reference number

    Example of correct format:

    - #1234 by @username implements the core authentication framework
    - Multiple documentation updates were contributed by @contributor focusing on installation guides and API references

    Here's the JSON data:
    ```json
    {json.dumps(data, ensure_ascii=False)}
    ```

    Format your response as:

    *This weekly update is generated by LLMs. You're welcome to join our [Discord](https://opendal.apache.org/discord/) for in-depth discussions.*

    ## Overall Activity Summary
    [Provide a concise overview of activity]

    ## Key Ongoing Projects
    [List major projects/initiatives with brief descriptions - always mention contributors with @ symbol]

    ## Priority Items
    [List issues/PRs that need immediate attention - always mention contributors with @ symbol]

    ## Notable Discussions
    [Highlight important ongoing discussions - always mention contributors with @ symbol, use format like #1234: brief description]

    ## Emerging Trends
    [Identify patterns or trends]

    ## Good First Issues
    [List good first issues for new contributors with brief explanations of what makes them approachable, use format like #1234: brief description]
    """

    try:
        response = client.chat.completions.create(
            model=model,
            messages=[
                {
                    "role": "system",
                    "content": "You are an open-source community evangelist responsible for reporting GitHub repository activity and encouraging more contributions.",
                },
                {"role": "user", "content": prompt},
            ],
            temperature=0.3,
            max_tokens=4000,
        )
        return response.choices[0].message.content
    except Exception as e:
        print(f"Error with OpenAI API: {e}")
        return f"Error generating summary: {e}"


def main():
    parser = argparse.ArgumentParser(
        description="Generate a weekly summary of GitHub repository activity with OpenAI analysis."
    )
    parser.add_argument("repo", help="GitHub repository in the format owner/repo")
    parser.add_argument(
        "--days", type=int, default=7, help="Number of days to look back (default: 7)"
    )
    parser.add_argument("--output", help="Output file path (default: stdout)")
    parser.add_argument(
        "--raw", action="store_true", help="Output raw JSON data without OpenAI summary"
    )
    parser.add_argument(
        "--json-output", help="Path to save raw JSON data (in addition to summary)"
    )
    parser.add_argument(
        "--gfi-count",
        type=int,
        default=5,
        help="Number of good first issues to include (default: 5)",
    )

    args = parser.parse_args()

    token = get_github_api_token()

    # Fetch data from GitHub API
    print(f"Fetching data from {args.repo} for the last {args.days} days...")
    issues, prs, recent_good_first_issues = fetch_issues(args.repo, token, args.days)
    discussions = fetch_discussions(args.repo, token, args.days)

    # If we don't have enough good first issues from recent activity, fetch additional ones
    if len(recent_good_first_issues) < args.gfi_count:
        print("Fetching additional good first issues...")
        additional_gfi = fetch_additional_good_first_issues(
            args.repo, token, args.gfi_count - len(recent_good_first_issues)
        )
        good_first_issues = recent_good_first_issues + additional_gfi
    else:
        good_first_issues = recent_good_first_issues

    print(f"Found {len(good_first_issues)} good first issues.")

    # Generate LLM-friendly structured data
    structured_data = format_data_for_llm(
        args.repo, issues, prs, discussions, good_first_issues, args.days
    )

    # Save raw JSON data if requested
    if args.json_output:
        with open(args.json_output, "w", encoding="utf-8") as f:
            json.dump(structured_data, f, ensure_ascii=False, indent=2)
        print(f"Raw JSON data written to {args.json_output}")

    # If raw output is requested, just print the JSON and exit
    if args.raw:
        if args.output:
            with open(args.output, "w", encoding="utf-8") as f:
                json.dump(structured_data, f, ensure_ascii=False, indent=2)
            print(f"Raw data written to {args.output}")
        else:
            print(json.dumps(structured_data, ensure_ascii=False, indent=2))
        return

    # Initialize openai
    print("Initializing openai API for summarization...")
    client, model = init_openai_client()

    # Generate summary with Gemini
    print("Generating summary with OpenAI ...")
    summary = summarize_with_openai(structured_data, client, model)

    # Output the result
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(summary)
        print(f"Summary written to {args.output}")
    else:
        print(summary)


if __name__ == "__main__":
    main()
