# Tracey — UX Tone of Voice

**Applies to**: All user-facing strings in Tracey.App — button labels, empty states, modal text, tooltips, error messages, confirmation prompts.

## Core Principles

### 1. Direct, not bossy
Say what needs to happen without making it a command. Prefer "Save entry" over "SAVE ENTRY". Prefer "What were you working on?" over "Enter description."

### 2. Brief, not terse
Every word earns its place. Don't truncate meaning — eliminate padding. No filler phrases like "Please note that..." or "In order to...".

### 3. Time-aware, not clinical
This is a tool about how people spend their time. Copy should acknowledge that. Use "away" not "inactive". Use "break" not "inactivity period". Don't expose internal terms like "TimeEntry" or "WindowActivityRecord".

### 4. Honest, not cheerful
Don't add exclamation marks or emoji to mask friction. If an action failed, say so plainly. "Couldn't save — check your connection" not "Oops! Something went wrong 😬".

### 5. Consistent, not creative
Use the same word for the same thing every time. Decisions:
- Timer is always "timer", never "clock" or "stopwatch"
- Stopping a timer is always "Stop timer", never "End timer" or "Finish"
- The away period is always "idle time", never "idle period" or "break time"
- A time record is a "time entry", never "log entry" or "record"
- Hierarchy: Client → Project → Task (always this order, always these words)

## Modal and Prompt Patterns

### Confirmation dialogs (destructive actions)
> **Delete [Client Name]?**  
> All projects and tasks under this client will be deleted. Time entries will be kept but unlinked.  
> [Cancel] [Delete client]

- Primary action is always the right button
- Destructive button is red/danger variant
- Never use "Are you sure?" — it's padding

### Idle-return prompt
> **You were away for [duration]**  
> What should we do with that time?  
> [Break] [Meeting] [Specify…] [Keep timer running]

- Four options, always in this order
- "Specify…" with ellipsis signals it opens more options
- No explanation of what each button does — the label says it

### Empty states
> No time entries yet.  
> Start the timer above to record your first entry.

- One sentence of fact, one sentence of action
- Never "Wow, nothing here!" — honest, not cheerful

### Error messages
- State what failed: "Couldn't save time entry."
- State why if known: "Connection to database lost."
- State what to do: "Check Settings → Database to reconnect."
- Never stack all three into one sentence

## Loading and Status Patterns

- Loading indicators name what's loading: "Loading entries…" not "Loading…"
- Use a real ellipsis character (…) not three dots (...)
- Success confirmations stay brief: "Settings saved." not "Your settings have been successfully saved."
- Connected state: "Connected" not "Connection established"

## Example String Pairs

| ❌ Avoid | ✅ Use |
|---------|-------|
| "Please enter a project name" | "Project name" (placeholder) |
| "Are you sure you want to delete this?" | "Delete [Name]?" |
| "Your timer is currently running" | "Timer running" |
| "No results found for your search" | "No matches" |
| "Click here to start tracking" | "Start timer" |
| "Inactivity detected" | "You were away" |
| "TimeEntry deleted successfully" | "Entry deleted" |
| "Inactivity Detection" (section title) | "Idle Detection" |
| "Connect & save" | "Connect" |
| "Confirm Delete" (button) | "Delete client" / "Delete project" |
| "Are you sure? Deleting X will…" | "All tasks under X will be deleted." |
| "Hover over the timeline or click a dot to preview a screenshot" | "Hover or click a dot to preview" |
| "+ Add Task" / "+ Add Project" | "+ Add task" / "+ Add project" |
