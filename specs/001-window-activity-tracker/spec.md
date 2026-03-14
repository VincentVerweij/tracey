# Feature Specification: Window Activity Timetracking Tool

**Feature Branch**: `001-window-activity-tracker`  
**Created**: March 14, 2026  
**Status**: Draft  
**Input**: User description: "Create a timetracking tool that keeps track of the user's active windows to determine what the user is working on. The window tracking gives information about which process, and window titles were in use."

## Clarifications

### Session 2026-03-14

- Q: How do two devices identify as belonging to the same user and connect to the shared external database? → A: User-managed connection string — the user pastes a database connection URI into app settings; no built-in login or account system is included.
- Q: What application shell technology is used to deliver the UI and OS-level hooks (input monitoring, screenshots)? → A: Web-technology UI inside a native shell (Electron or Tauri) — web frontend for all UI, thin native layer for OS hooks; packaged as a single portable binary.
- Q: Are screenshots protected at rest (encrypted on disk) or stored as plain image files? → A: Plain image files — screenshots are saved as-is in the configured folder; security is the responsibility of OS-level folder permissions controlled by the user.
- Q: What happens when the user returns from an idle period and no timer was active when idle began? → A: Silently dismiss — no idle-return prompt is shown; the user simply continues with no running timer.
- Q: What time scope does the default time entry list show upon launch? → A: Entries are grouped by date and scrollable, paginated by a configurable number of entries per page.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Start Tracking Time on a Task (Priority: P1)

A user opens the app and begins tracking time against a project and task. They type a description using the quick-entry slash notation (e.g., `ClientProject/Bug Fix/Investigate login crash`), the system fuzzy-matches the project and task in real time, and starts the timer. The user sees the timer counting up at the top of the entry list. When they finish they stop the timer and the entry is saved.

**Why this priority**: This is the core daily workflow. Without the ability to start and stop a timer no other feature delivers value.

**Independent Test**: Can be fully tested by launching the app, creating a client/project/task, typing a slash-delimited entry in the quick-entry bar, pressing Enter, and confirming the timer starts and stops correctly with the entry visible in the list.

**Acceptance Scenarios**:

1. **Given** a user has projects and tasks set up, **When** they type `project/task/description` in the quick-entry bar, **Then** the system fuzzy-matches project and task in a live dropdown sorted by match strength.
2. **Given** the live dropdown is visible, **When** the user presses Tab or Enter on a match, **Then** the segment is confirmed and the cursor moves to the next segment.
3. **Given** a complete entry is confirmed, **When** the user presses Enter, **Then** a new timer starts and any previously running timer is automatically stopped and saved.
4. **Given** a timer is running, **When** the user stops it, **Then** the time entry is saved with the correct start and end datetimes in UTC, displayed in the user's configured local timezone.
5. **Given** a running timer has been stopped, **When** the user clicks Continue on the past entry, **Then** a new timer starts immediately using the current time as its start datetime, with the same description, project, and task copied from the original entry.

---

### User Story 2 - Idle Detection and On-Return Prompt (Priority: P1)

A user walks away from their computer mid-session. The system detects inactivity after a configurable timeout by monitoring mouse movement and keyboard input. When the user returns and interacts with the computer, the system shows a prompt asking how to classify the away period, offering Break, Meeting, Specify, or Keep Timer Running. The user chooses one and continues working.

**Why this priority**: Without idle handling, tracked time becomes inaccurate and the tool loses its core value proposition of honest time reporting.

**Independent Test**: Can be fully tested by setting a short inactivity timeout, waiting without mouse or keyboard input until the timeout elapses, then moving the mouse and verifying the return prompt appears with all four options.

**Acceptance Scenarios**:

1. **Given** the user has been inactive (no mouse or keyboard input) for longer than the configured inactivity timeout, **When** the system detects inactivity, **Then** idle tracking begins and a new sub-timer is started for the idle period.
2. **Given** an idle period is active, **When** the user moves the mouse or presses a key, **Then** the system immediately presents a modal prompt with four options: Break, Meeting, Specify, and Keep Timer Running.
3. **Given** the prompt is shown and the user selects Break, **Then** the idle period is logged as a break entry, and the active work timer resumes from the current moment.
4. **Given** the prompt is shown and the user selects Meeting, **Then** the time entry creation form opens pre-filled with the idle duration, ready for the user to assign a project and task.
5. **Given** the prompt is shown and the user selects Specify, **Then** a free-text input with the full task picker (project, task, tags) is shown so the user can precisely classify the idle period.
6. **Given** the prompt is shown and the user selects Keep Timer Running, **Then** the prompt is dismissed, the idle period is treated as continued work on the running timer, and tracking continues uninterrupted.

---

### User Story 3 - Manage Clients, Projects, and Tasks (Priority: P2)

A user sets up their organizational hierarchy by creating clients, assigning projects to clients, and creating tasks under each project. They can archive clients or projects to hide them from active use without losing historical data, and can delete clients with an explicit confirmation.

**Why this priority**: The organizational hierarchy is required before time entries can be meaningfully categorized. It must exist before most other features are useful.

**Independent Test**: Can be fully tested by creating a client with a name and color, adding two projects to it, adding tasks to each project, archiving one project, and verifying it no longer appears in the project dropdown while the other remains.

**Acceptance Scenarios**:

1. **Given** the user creates a client with a name, optional logo, and color, **When** they save it, **Then** the client is available when assigning projects.
2. **Given** a client exists, **When** the user creates a project under it with a name, **Then** the project appears in the project picker filtered by client.
3. **Given** a project exists, **When** the user creates a task under it, **Then** the task is available during time entry creation when that project is selected.
4. **Given** a client or project is archived, **When** the user opens the project/task dropdown during time entry creation, **Then** archived clients and projects do not appear in the list.
5. **Given** an archived client or project, **When** the user unarchives it, **Then** it reappears in the active dropdown list.
6. **Given** a client has projects and linked time entries, **When** the user deletes the client and confirms the deletion prompt, **Then** the client and all its projects and tasks are deleted, and linked time entries become orphaned but are not deleted.

---

### User Story 4 - Screenshot Timeline Review (Priority: P2)

A user reviews their workday by scrolling through a visual timeline of automatically captured screenshots. Screenshots are taken at a configurable interval and whenever the active window changes. The user can see which application or document they were viewing at any point during the day.

**Why this priority**: Screenshot review provides objective evidence of what was worked on, increasing the value of the activity tracking context beyond just window titles.

**Independent Test**: Can be fully tested by launching the app, waiting for one screenshot interval to elapse and switching windows to trigger additional captures, then opening the timeline view and verifying screenshots appear at the correct times with the ability to scroll.

**Acceptance Scenarios**:

1. **Given** the app is running, **When** the configured screenshot interval elapses, **Then** a screenshot is captured and saved to the configured storage location.
2. **Given** the app is running, **When** the active process, window, or window title changes, **Then** a screenshot is immediately captured regardless of the interval timer.
3. **Given** screenshots have been collected, **When** the user opens the timeline view, **Then** they can scroll through a chronological timeline and see screenshots linked to their capture time.
4. **Given** the user scrolls to a point on the timeline, **When** a screenshot exists at or nearest to that time, **Then** the screenshot is displayed in the viewer.
5. **Given** a retention window has been configured (e.g., 30 days), **When** a screenshot exceeds the retention window, **Then** it is automatically deleted.
6. **Given** the user configures a custom screenshot save folder, **When** screenshots are captured, **Then** they are saved to the specified folder. If no folder is configured, they are saved in the folder containing the portable executable.

---

### User Story 5 - Keyboard-First Quick Entry with Fuzzy Matching (Priority: P2)

A power user creates time entries exclusively from the keyboard without touching the mouse. They type into the persistent quick-entry bar at the top of the entry list, using slashes to delimit project, optional task, and description segments. The system fuzzy-matches each segment and offers real-time suggestions with VS Code-style narrowing.

**Why this priority**: The quick-entry bar is the primary interface for fast, uninterrupted time tracking in a professional workflow.

**Independent Test**: Can be fully tested with at least two projects and tasks configured, by typing partial project names with typos into the quick-entry bar and verifying that fuzzy matches surface correctly ordered, with arrow-key and Tab/Enter confirmation working without touching the mouse.

**Acceptance Scenarios**:

1. **Given** the user types a partial project name in the quick-entry bar, **When** the system processes the input, **Then** a live dropdown appears beneath the input sorted by fuzzy-match strength, case-insensitively and tolerant of minor spelling differences.
2. **Given** the user types `projectname/`, **When** the first slash is entered, **Then** the system locks in the project segment and begins fuzzy matching against tasks for that project in the second segment.
3. **Given** the user types `project/task/description`, **When** three segments are present, **Then** the parser treats them as (project, task, description) with no ambiguity.
4. **Given** the user types `project/description`, **When** only one slash is present, **Then** the parser treats the segments as (project, description) with no task assigned.
5. **Given** the user types `project/task/`, **When** the trailing slash is present but no description follows, **Then** the parser assigns the task with an empty description and does not guess.
6. **Given** a project name matches projects under more than one client, **When** the project segment is confirmed, **Then** an inline disambiguation dropdown appears listing the matching clients, navigable with arrow keys and Enter—this is the only interruption in the flow.
7. **Given** the user navigates the dropdown with arrow keys, **When** they press Tab or Enter, **Then** the highlighted match is confirmed and focus moves to the next segment.

---

### User Story 6 - Tag Management and Assignment (Priority: P3)

A user pre-creates a set of tags (e.g., "Deep Work", "Administrative", "Code Review") and assigns one or more tags to time entries. Tags cannot be created on the fly during entry logging. When a tag is deleted, any time entries that referenced it are not deleted—the tag link is simply removed.

**Why this priority**: Tags enrich reporting and filtering but are not required for core time tracking to function.

**Independent Test**: Can be fully tested by creating several tags in the settings area, assigning one to a time entry, deleting the tag, and verifying the time entry still exists without the deleted tag attached.

**Acceptance Scenarios**:

1. **Given** the user opens the tag management screen, **When** they create a tag with a name, **Then** the tag is available for assignment during time entry creation or editing.
2. **Given** the user is creating or editing a time entry, **When** they assign one or more tags, **Then** the entry is saved with those tags linked.
3. **Given** a tag exists, **When** the user attempts to delete it, **Then** a warning is shown indicating that all links to existing time entries will be removed.
4. **Given** the user confirms a tag deletion, **When** the deletion completes, **Then** the tag no longer appears in the tag list or in any time entry detail; affected time entries are otherwise unmodified.
5. **Given** a user types the beginning of a description in the quick-entry bar or description field, **When** auto-complete suggestions appear, **Then** selecting a suggestion starts a new entry with the same description, project, task, and tags copied from the matched historical entry.

---

### User Story 7 - Long-Running Timer Notification (Priority: P3)

The system notifies the user when an active timer has been running longer than a configurable threshold (default 8 hours). The user can configure one or more notification channels (email or Telegram) with the settings specific to each channel.

**Why this priority**: Alerts are a safety net, not a core tracking function. They improve the tool's usefulness for users who work long hours but are not required for basic tracking.

**Independent Test**: Can be fully tested by setting the timer threshold to a short value (e.g., 5 minutes), starting a timer, waiting, and verifying a notification is delivered on a configured channel (e.g., Telegram message received).

**Acceptance Scenarios**:

1. **Given** a timer is running and has exceeded the configured notification threshold, **When** the threshold is crossed, **Then** the system sends a notification through all configured channels.
2. **Given** the user configures an email notification channel, **When** they provide the mail provider settings (e.g., SMTP credentials), **Then** the system can send notification emails using those settings.
3. **Given** the user configures a Telegram notification channel, **When** they provide bot and chat settings, **Then** the system sends Telegram messages using that bot when a notification is triggered.
4. **Given** multiple notification channels are configured, **When** a notification is triggered, **Then** all configured channels receive the notification.
5. **Given** a notification channel type does not exist yet, **When** a developer adds support for a new channel, **Then** they implement only the required message-and-settings abstraction without modifying existing channel code.

---

### User Story 8 - Cloud Sync and Cross-Device Visibility (Priority: P3)

A user who works on two computers (e.g., a desktop and a laptop) starts a timer on the desktop and switches to the laptop mid-afternoon. On the laptop, the app shows the currently running timer started on the desktop. The user stops it from the laptop and the change is reflected on both devices.

**Why this priority**: Cloud sync enables the tool to serve users across multiple devices, but the tool remains fully usable on a single device without it.

**Independent Test**: Can be fully tested with two instances of the app connected to the same external database: start a timer on instance A, refresh instance B, and verify the timer is visible and stoppable from instance B.

**Acceptance Scenarios**:

1. **Given** the app is connected to an external database, **When** a time entry is created or modified, **Then** it is synchronized to the external database within a short time.
2. **Given** a timer is running on device A, **When** a user opens the app on device B connected to the same account, **Then** the running timer from device A is visible on device B.
3. **Given** the user is offline, **When** they create or modify a time entry, **Then** the entry is stored in the local cache and synchronized when connectivity is restored.
4. **Given** window activity data (process names, window titles) is collected, **When** it is synchronized, **Then** it is stored in the external database for future machine-learning use; screenshots are never transmitted or synchronized.
5. **Given** a time entry is modified on two devices while offline, **When** both devices reconnect, **Then** the conflict is resolved using a last-write-wins strategy based on modification timestamp.

---

### User Story 9 - Run as Portable Application Without Admin Rights (Priority: P3)

A user on a corporate machine without administrator privileges downloads a single executable file, places it anywhere on their filesystem, and runs it. The app functions fully without requiring installation, registry access, or elevated permissions.

**Why this priority**: The portable requirement constrains deployment but does not affect the core feature set. It becomes critical when the tool needs to be rolled out in restricted environments.

**Independent Test**: Can be fully tested by running the executable from a standard user account on a Windows machine without admin rights, completing a full timer start/stop cycle, and verifying no installation prompts or permission errors appear.

**Acceptance Scenarios**:

1. **Given** a user without administrator rights, **When** they run the portable executable from any local folder, **Then** the application starts and all features function without requesting elevated permissions.
2. **Given** the portable executable is launched for the first time, **When** no configuration exists, **Then** the app creates its configuration and data files in the executable's folder or a user-writable location.
3. **Given** the app is running on Windows, **When** a user moves it to a different folder, **Then** it continues to function, and screenshot storage defaults to the new executable folder.

---

### Edge Cases

- What happens when the user returns from idle and no timer was running before the idle period? The idle-return prompt is silently suppressed; the user returns to the app with no active timer and no automatic entry is created. The app must not take a screenshot on every change — a debounce or minimum interval between window-switch-triggered screenshots must be applied.
- What happens when two time entries overlap during manual entry creation? The system must warn the user of the overlap and require confirmation or correction.
- What happens when the external database is unreachable for an extended period? All operations continue locally; the sync queue is preserved and replayed once connectivity returns.
- What happens when a project is assigned to a time entry and that project is later archived? The time entry retains its project and task link, but the project no longer appears in active dropdowns.
- What happens when auto-complete matches a historical entry whose project or task has since been deleted? The auto-complete suggestion appears but the orphaned project/task assignment is flagged on the new entry.
- What happens if the screenshot storage folder becomes full or inaccessible? The app captures a failure silently, logs the error, and notifies the user via an in-app message without crashing.
- What happens when the user changes their configured timezone? Stored UTC datetimes remain unchanged; all display values are recalculated using the new timezone setting.

## Requirements *(mandatory)*

### Functional Requirements

#### Activity Monitoring

- **FR-001**: The system MUST monitor which application process and window title is currently active on the user's desktop and record the active window at each observed moment.
- **FR-002**: The system MUST track mouse movement and keyboard input events to determine whether the user is active or inactive.
- **FR-003**: The system MUST allow users to configure an inactivity timeout (default: 5 minutes) after which they are considered inactive.
- **FR-004**: When the user transitions from inactive to active, the system MUST immediately display a prompt presenting four options: Break, Meeting, Specify, and Keep Timer Running — but ONLY if a timer was actively running when the idle period began.
- **FR-005**: If no timer was running when the idle period began, the system MUST NOT display the idle-return prompt when the user returns; the user continues with no active timer and no entry is created for the idle period.
- **FR-006**: Selecting Break MUST log the idle period as a break time entry and resume the active work timer from the current moment.
- **FR-007**: Selecting Meeting MUST open the time entry form pre-filled with the idle duration so the user can quickly assign a project and task.
- **FR-008**: Selecting Specify MUST open a free-text input with the full project/task picker for precise classification of the idle period.
- **FR-009**: Selecting Keep Timer Running MUST dismiss the prompt and treat the entire idle period as continued work on the running timer without any separate log entry.

#### Screenshot Capture

- **FR-009**: The system MUST capture a screenshot at a configurable interval (default: 1 minute).
- **FR-010**: The system MUST capture an additional screenshot whenever the active process, window handle, or window title changes, regardless of the interval timer.
- **FR-011**: When the window title changes more frequently than a minimum debounce period, the system MUST NOT capture a screenshot on every change; it MUST wait for the title to stabilize before capturing.
- **FR-012**: Screenshots MUST be saved locally to the configured storage folder (default: the folder containing the portable executable).
- **FR-013**: The user MUST be able to configure a custom folder where screenshots are saved.
- **FR-014**: The system MUST automatically delete screenshots older than a configurable number of days (default: 30 days) using a rolling retention window.
- **FR-015**: The system MUST present a scrollable timeline view where screenshots are displayed at the time they were captured, allowing the user to browse their activity history visually.
- **FR-016**: Screenshots MUST NOT be synchronized to any external service or database.
- **FR-017**: Screenshots MUST be stored as plain image files (e.g., PNG or JPEG) in the configured folder without any application-level encryption; the user is responsible for securing the folder using OS-level access controls.

#### Time Entry Management

- **FR-017**: Every time entry MUST have a description, a start datetime, and an end datetime stored in UTC.
- **FR-018**: A time entry MAY be linked to a project and a task under that project.
- **FR-019**: A time entry MAY have one or more predefined tags attached.
- **FR-020**: The system MUST allow only one active (running) timer at any given moment; starting a new entry MUST automatically stop and save the currently running entry.
- **FR-021**: The user MUST be able to manually create a time entry by specifying a start datetime, end datetime, description, and optionally a project, task, and tags.
- **FR-022**: Every past time entry MUST have a Continue button that creates a new timer starting at the current moment, copying the description, project, task, and tags from the original entry.
- **FR-023**: The system MUST display all time entry datetimes in the user's configured local timezone while storing them internally in UTC.
- **FR-024**: The user MUST be able to configure their local timezone in the application settings.
- **FR-025**: The description input MUST show an auto-complete dropdown when the user begins typing, populated with descriptions from previously completed time entries.
- **FR-026**: The auto-complete dropdown MUST appear as the user types and filter suggestions in real time.
- **FR-027**: Selecting an auto-complete suggestion (by pressing Enter or clicking) MUST start a new timer with the same description, project, task, and tags as the matched historical entry.
- **FR-028**: Time entry data MUST be automatically saved when the user moves focus away from the entry input without requiring an explicit save action.
- **FR-029**: The time entry list MUST display entries grouped by date in descending order (most recent date first), with entries within each day listed chronologically.
- **FR-030**: The time entry list MUST be paginated; the number of entries loaded per page MUST be configurable by the user, with a sensible default (e.g., 50 entries per page).
- **FR-031**: The user MUST be able to scroll through all date groups and load additional pages without losing their scroll position within the current page.

#### Quick-Entry Bar

- **FR-029**: The app MUST display a persistent input bar at the top of the time entry list, always visible.
- **FR-030**: The quick-entry bar MUST accept a slash-delimited string where one slash means two segments (project, description) and two slashes mean three segments (project, task, description).
- **FR-031**: The parser MUST NOT infer missing segments; if the user wants to assign a task with no description, they MUST include the trailing slash (e.g., `project/task/`).
- **FR-032**: As the user types each segment, the system MUST perform real-time fuzzy matching against existing projects and tasks, case-insensitively and tolerant of minor spelling differences.
- **FR-033**: Fuzzy-match results MUST appear in a live dropdown sorted by match strength, narrowing character by character (identical in behavior to VS Code's Ctrl+P file search).
- **FR-034**: The user MUST be able to navigate the dropdown with arrow keys and confirm a segment with Tab or Enter before moving to the next segment.
- **FR-035**: The client MUST be inferred silently from the project name when the project name is unique across all clients.
- **FR-036**: When a project name matches more than one client, the system MUST display an inline disambiguation dropdown listing the matching clients, navigable by arrow keys and confirmed with Enter; this MUST be the only forced interruption in the quick-entry flow.

#### Client, Project, and Task Management

- **FR-037**: Users MUST be able to create clients with a name, an optional logo image, and a color.
- **FR-038**: Users MUST be able to create projects scoped to a client; project names MUST be unique within a client but MAY be shared across different clients.
- **FR-039**: Users MUST be able to create tasks scoped to a project; a task has a name.
- **FR-040**: Clients and projects MUST be individually archiveable; archived clients and projects MUST NOT appear in any active dropdown or picker.
- **FR-041**: Archived clients and projects MUST be restorable (unarchiveable) at any time.
- **FR-042**: Deleting a client MUST cascade the deletion to all its projects and tasks; the system MUST display a confirmation prompt warning the user before proceeding.
- **FR-043**: Time entries linked to a deleted client's projects MUST become orphaned (project/task link removed) but MUST NOT themselves be deleted.

#### Tag Management

- **FR-044**: Tags MUST be created by the user in a dedicated management area and MUST NOT be creatable on the fly during time entry logging.
- **FR-045**: A time entry MAY have zero or more tags assigned; tags are selected from the predefined list during entry creation or editing.
- **FR-046**: Deleting a tag MUST display a warning explaining that the tag will be unlinked from all existing time entries.
- **FR-047**: After a tag is deleted, affected time entries MUST be unmodified except for the removal of the deleted tag link.

#### Notifications

- **FR-048**: The system MUST notify the user when a timer has been continuously running for longer than a configurable threshold (default: 8 hours).
- **FR-049**: The notification system MUST be designed as an abstraction with a message and a channel-specific settings object, so new notification channels can be added without modifying existing channels.
- **FR-050**: The system MUST include a built-in email notification channel configurable with mail provider settings (e.g., outbound mail server, sender address, recipient address).
- **FR-051**: The system MUST include a built-in Telegram notification channel configurable with bot token and target chat settings.
- **FR-052**: The user MUST be able to configure multiple notification channels simultaneously; all configured channels receive all triggered notifications.

#### Cloud Synchronization and Local Cache

- **FR-053**: The user MUST be able to configure the external database connection by providing a connection URI in the application settings; the application MUST NOT include a built-in account registration, login, or authentication system.
- **FR-054**: Time entries MUST be synchronized to the external database configured via the user-supplied connection URI so they are accessible across multiple devices that share the same connection URI.
- **FR-055**: The currently running timer MUST be visible on all devices sharing the same connection URI in near-real time.
- **FR-056**: Window activity records (active process name, window title, timestamp) MUST be synchronized to the external database; these will be used for machine-learning-based automatic tagging in a future effort.
- **FR-057**: Screenshots MUST NOT be synchronized to any external service.
- **FR-058**: The system MUST maintain a local cache of time entry data to support fast creation and modification and to enable full functionality when the device is offline or the external database is unreachable.
- **FR-059**: When connectivity is restored after an offline period, the local cache MUST automatically sync pending changes to the external database.
- **FR-060**: Conflicts arising from the same entry being modified on two devices while offline MUST be resolved using a last-write-wins strategy based on the modification timestamp.

#### Portability and Platform Support

- **FR-061**: The application MUST run as a portable executable requiring no installation and no administrator rights.
- **FR-062**: The application MUST be packaged as a single self-contained binary using a web-technology-in-native-shell approach (Tauri), with the web UI frontend embedded in the binary and OS-level capabilities (input monitoring, screenshot capture, system tray integration) handled by the native layer.
- **FR-063**: On first launch, the application MUST create all required configuration and data files in the executable's directory or a user-writable location without requiring system-level access.
- **FR-064**: The application MUST be primarily supported on Windows but MUST be architected to run on other operating systems; the native OS hooks (active window detection, global input monitoring) MUST have platform-specific implementations that are swappable per OS.

### Key Entities

- **Time Entry**: Represents a logged period of work. Has a description, start datetime (UTC), end datetime (UTC), optional project link, optional task link, and zero or more tag links. Can be running (no end datetime) or completed.
- **Timer**: The single globally active running time entry. Only one exists at a time. Starting a new timer closes the current one.
- **Client**: Represents a client organization. Has a name, optional logo, a color, and an archived flag. Parent of projects.
- **Project**: Represents a work project scoped to one client. Has a name and an archived flag. Parent of tasks.
- **Task**: Represents a specific task under a project. Has a name. Scoped to one project.
- **Tag**: A predefined label created by the user. Has a name. Can be linked to zero or more time entries.
- **Window Activity Record**: A timestamped snapshot of the user's active process name, window handle, and window title. Collected continuously and synchronized to the external database.
- **Screenshot**: A captured image of the user's screen at a point in time. Has a file path, capture timestamp, and associated window title. Stored locally only, never synchronized.
- **Notification Channel**: An abstract unit describing how to reach the user. Has a message payload and a channel-specific settings object. Implementations: Email, Telegram.
- **User Preferences**: User-scoped configuration including local timezone, inactivity timeout, screenshot interval, screenshot retention days, screenshot storage folder, timer notification threshold, configured notification channels, the external database connection URI, and the time entry list page size.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can start a timed work entry from scratch in under 15 seconds using only the keyboard, without touching the mouse.
- **SC-002**: The idle-return prompt appears within 3 seconds of the user first moving the mouse or pressing a key after an inactive period.
- **SC-003**: Screenshots captured during an 8-hour workday are viewable in the timeline and linked to the correct minute, with no more than a 60-second deviation from the actual capture time.
- **SC-004**: Time entries created while offline are reliably synchronized to the external database within 60 seconds of the network connection being restored, with no data loss.
- **SC-005**: A user can install and fully use the application on a Windows machine without administrator rights, with zero installation errors or permission prompts.
- **SC-006**: The application starts and is ready for interaction within 5 seconds on a standard consumer laptop.
- **SC-007**: Window activity records are synchronized to the external database within 30 seconds of being captured, when connectivity is available.
- **SC-008**: A timer running on one device is visible on a second device connected to the same account within 10 seconds of the timer starting.
- **SC-009**: Screenshots older than the configured retention window are automatically cleaned up without user intervention, reclaiming disk space within 24 hours of the retention deadline.
- **SC-010**: Adding a new notification channel requires no changes to existing channel implementations, verifiable by adding a third channel type alongside Email and Telegram without modifying either.

## Assumptions

- The default inactivity timeout is 5 minutes, as this is a common standard for productivity tools.
- The default screenshot retention period is 30 days, balancing storage cost with historical review needs.
- Project names are unique within a client but may appear under multiple clients; this is the only case that triggers an inline disambiguation step.
- Conflict resolution for concurrent offline edits uses last-write-wins based on modification timestamp, as this is the simplest strategy adequate for single-user multi-device use.
- The external database connection is configured by the user by pasting a connection URI into application settings; no built-in account system or authentication is provided. The user is responsible for provisioning and securing their own database instance (e.g., a self-hosted or cloud Postgres/Supabase instance).
- Window activity records are batched and synchronized periodically (e.g., every few minutes) rather than in real time, to minimize network overhead.
- The screenshot debounce period for rapid window title changes is 2 seconds by default to avoid excessive captures from browsers that update the title frequently.
- The application shell is a web-technology-in-native-shell binary (Tauri). The web UI handles all visual presentation; the native layer provides OS hooks for global input monitoring, active window detection, screenshot capture, and system tray / notification integration. This satisfies the portable binary requirement without admin rights.
- Screenshots are stored as plain image files with no application-level encryption. Privacy and access control are delegated to OS folder permissions. The user is advised to place the screenshot folder in a location accessible only to their own OS account.
