---
name: magic-pr
description: >-
  Write concise PR descriptions of completed Magic Studio work.
  Use for PR drafting, not code review or planning.
---
# Magic PR

Describe completed work using these headings in order. Keep it short.
Use observed evidence; never invent results, IDs or resume commands.
Invoking this skill does not authorize submitting or merging a PR.

## Solution
Summarize the solution and resulting behavior in 1–5 sentences.

## What
State the changes and related issue, if one exists.

## Why
Explain the problem solved and why the change matters.

## Where
List the changed file paths and what changed in each.

## magic-cli audit stats
Record the actual command used and its exit status.
Report native tool names and values. Show before → after only when
both results use comparable scope, tools and configuration.
Label incomparable results; do not imply measured progress from them.
List failed, skipped, unavailable and not-run checks explicitly.
Missing audit evidence stays unavailable or not run.

## Agent session
Put the agent name, harness and session ID at the bottom.
Include the exact verified resume command, or state unavailable.
Mark unknown session details unavailable; never guess a command.
