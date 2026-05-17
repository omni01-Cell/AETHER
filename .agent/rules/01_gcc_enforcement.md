---
trigger: always_on
description: Maintains the Git-Context-Controller (GCC) as your persistent long-term working memory. MUST trigger file-write tools.
---

# Git-Context-Controller (GCC) — Continuous Sync

<context>
The Git-Context-Controller (`.GCC/`) is your **long-term working memory**. It is the only persistent state that survives between sessions. Every significant action MUST be physically written to `.GCC/main.md` via your file-editing tools before the next action begins.
</context>

<hooks>
<hook id="decision_milestone">
<trigger>Resolving critical bugs, selecting architectures, completing subtasks, or making persistent choices.</trigger>
<action>
1. TOOL INVOCATION: Use your file-system tool to read `.GCC/main.md`.
2. TOOL INVOCATION: Use your file-system tool to write/edit the file, recording the date, decision, rationale, and updating "Current Status".
3. Await tool success confirmation before generating the response.
</action>
</hook>

<hook id="divergence_plan_b">
<trigger>An approach fails after multiple attempts AND a new strategy is proposed.</trigger>
<action>
1. TOOL INVOCATION: Write the failure context to `.GCC/branches/attempt_[name]_failed.md`.
2. TOOL INVOCATION: Edit `.GCC/main.md` to reference the failure under "Abandoned Branches".
</action>
</hook>

<hook id="knowledge_sync">
<trigger>Retrieving or rating knowledge chunks via LoopRAG tools.</trigger>
<action>
1. TOOL INVOCATION: Edit `.GCC/main.md` to append the chunk reference (ID, source, score) under "Supabase Chunks Used".
</action>
</hook>

<hook id="epic_transition">
<trigger>The main ## Objective is completed and a completely new phase/epic begins.</trigger>
<action>
1. TOOL INVOCATION: Do NOT delete the old objective. Move its summary to the "## 🏆 Major Milestones (Archived Epics)" section. With [phase X] and date [X-X-X] example : ## 🏆 Major Milestones (Archived Epics)
- **[Phase 1] [2025-04-22]
2. TOOL INVOCATION: Write the new objective under "## Objective".
</action>
</hook>
</hooks>

<confirmation_tag>
CRITICAL DIRECTIVE (ZERO HALLUCINATION): 
NEVER output this tag unless you have ACTUALLY EXECUTED the file-write tool and verified the file on disk was modified. Textual roleplay is strictly banned.

Only output this verbatim AFTER a verified successful file-write operation:

<gcc_sync>
[X] .GCC/main.md successfully written via tool — [one-line summary]
</gcc_sync>
</confirmation_tag>

<template id="gcc_main">
Maintain this strictly formatted structure in `.GCC/main.md`:

# Current task context

## 🏆 Major Milestones (Archived Epics)
- [YYYY-MM-DD][Name of the completed major phase/epic and its key outcomes]

## Objective
[What THIS CURRENT session is building or solving]

## Decisions made
- [YYYY-MM-DD] Chose X over Y because [reason]

## Current status
- ✅ Done: [list for current objective only]
- 🔄 In progress: [current item]
- ⏳ Pending: [list]

## Next action
[Single, concrete next step]

## Abandoned branches
- [YYYY-MM-DD] [approach] → see .GCC/branches/[filename].md

## Supabase chunks used
- chunk_id: [id] | source: [book/article] | score: [0.00]

</template>

<scope>
Apply to every message if `.GCC/` exists. If not, TOOL INVOCATION: create `.GCC/main.md` using the template.
</scope>