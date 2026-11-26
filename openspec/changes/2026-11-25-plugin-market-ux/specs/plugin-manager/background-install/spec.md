## Purpose

Keeps plugin installation running independently of the install dialog, gives the user persistent feedback across panels, and lets a refreshed page reattach to the Host-owned task.

## ADDED Requirements

### Requirement: Closing the dialog does not cancel an install

While the client task is in `checking`, `starting`, `running`, `cancelling`, `applying`, or `unconfirmed`, closing the install dialog SHALL hide it and keep the task running; only an explicit Cancel action SHALL stop the task. Once the task is settled (`done`, `failed`, `unknown`), closing the dialog SHALL dismiss the result.

#### Scenario: Close during installation

- **WHEN** an installation is running and the user closes the dialog with X, Escape, or the backdrop
- **THEN** the dialog hides
- **AND** the installation continues
- **AND** the task bar remains available

#### Scenario: Close during the pre-install check

- **WHEN** the pre-install check is running and the user closes the dialog
- **THEN** the dialog hides and the task bar shows the “checking” phase
- **AND** the check continues until it settles or the user explicitly cancels

#### Scenario: Close a settled result

- **WHEN** a failed or unknown result is open and the user closes the dialog
- **THEN** the task bar and the install notice for that result are dismissed

#### Scenario: Explicit cancel

- **WHEN** the user presses Cancel in the dialog or on the task bar
- **THEN** the existing cancellation flow runs, including the too-late/applying refusal

### Requirement: A global task bar reports the active task

The client SHALL show a task bar outside the Plugins panel while the task is in `checking`, `starting`, `running`, `cancelling`, `applying`, or `unconfirmed`, and SHALL keep the last `failed` or `unknown` result until the user dismisses it.

#### Scenario: Switch panels

- **WHEN** an installation is running and the user opens a chat session
- **THEN** the task bar remains visible with the plugin name, phase, View, and Cancel

#### Scenario: Failure stays visible

- **WHEN** an installation fails while the dialog is hidden
- **THEN** the task bar shows a persistent “failed” state with View and Close

#### Scenario: View from another panel

- **WHEN** the user presses View on the task bar while a chat session is open
- **THEN** the app switches to the Plugins panel and opens the install dialog for that task

### Requirement: Install results use a dedicated notification channel

Install completion, failure, approval-needed, and cancellation notices SHALL NOT be overwritten by unrelated refresh or plugin-operation notices.

#### Scenario: Refresh failure while an install finishes

- **WHEN** a background install finishes and a bundle-list refresh also fails
- **THEN** both outcomes remain available; the install notice is not replaced by the refresh failure

#### Scenario: Build-script approval

- **WHEN** an installation stops because install scripts need approval
- **THEN** the task bar and notification say approval is needed
- **AND** no script runs until the user explicitly approves

### Requirement: Market installs start in the background

After the Discover confirmation, the install SHALL start without opening a second install dialog; a check failure SHALL open the dialog with its reason, while an execution failure SHALL stay in the task bar and notification.

#### Scenario: Normal market install

- **WHEN** the user confirms an entry in Discover
- **THEN** the confirmation closes and the task starts in the background
- **AND** no second install dialog appears

#### Scenario: Check refusal

- **WHEN** the pre-install check refuses the spec or cannot reach the source
- **THEN** the install dialog opens and shows the reason
- **AND** the entered spec and input error remain visible

#### Scenario: Execution failure

- **WHEN** the package manager fails after the check succeeded
- **THEN** the install does not steal focus
- **AND** the task bar and install notification report the failure with a View action

### Requirement: Host exposes an install task query

The Host SHALL return an active install by request id, and SHALL retain a bounded recent result (at most 8 entries for 10 minutes) so a refreshed client can settle a task that finished during the reload. A settled task SHALL NOT be returned by the active-only wait call.

#### Scenario: Active task

- **WHEN** the client queries a request id whose installation is still running
- **THEN** the Host returns the spec, start time, and `installing`, `cancelling`, or `applying` phase

#### Scenario: Recently settled task

- **WHEN** the client queries a request id that finished within the retention window
- **THEN** the Host returns the change result

#### Scenario: Unknown or expired task

- **WHEN** the request id never existed, expired, or its manager rejected before storing a result
- **THEN** the Host returns null

#### Scenario: Settled task is not active

- **WHEN** the client calls the active-only wait for a task that already settled
- **THEN** the call returns null, and the client settles through the recent-result query

### Requirement: Refresh restores the Host-owned task

A refreshed client SHALL reattach to an active Host task or reconstruct a recently settled result, using a credential-free session record and a sanitized display of the Host spec.

#### Scenario: Refresh during installation

- **WHEN** the page refreshes while the Host task is running
- **THEN** the task bar and phase return
- **AND** the client waits for the result and applies it once

#### Scenario: Refresh just after completion

- **WHEN** the page refreshes after the Host task finished but within the retention window
- **THEN** the client shows the completion or failure result without repeating analytics, applied-state changes, highlight, or toggle tracking

#### Scenario: Pre-refresh logs are not replayed

- **WHEN** a restored task is shown before any new log chunk arrives
- **THEN** the dialog says the pre-refresh output was not retained
- **AND** the message disappears on the first new chunk or when the task settles

#### Scenario: Restored spec is displayed safely

- **WHEN** the client restores a task whose Host spec contains URL credentials or a token
- **THEN** the dialog displays a sanitized form and the raw spec is not written to session storage

#### Scenario: Refresh during the client-side check

- **WHEN** the page refreshes before the Host task was created
- **THEN** the client says the check was interrupted and asks the user to start again
- **AND** no fake running task is shown

#### Scenario: Initial recovery query fails

- **WHEN** the first `getInstall` call fails with a network error
- **THEN** the client keeps the session record, shows no fake task, and retries once on connection reset or the next Plugins visit

#### Scenario: Old Host has no recovery query

- **WHEN** `getInstall` is unknown to the Host
- **THEN** the client clears the session record and logs the incompatibility

#### Scenario: Wait query races completion

- **WHEN** the wait call returns null because the task settled between calls
- **THEN** the client queries `getInstall` once more and settles from the recent result
- **AND** only two null queries make the task unknown

#### Scenario: Wait query fails

- **WHEN** the wait call fails with an RPC error
- **THEN** the client queries `getInstall` once more
- **AND** if that also fails, the task enters `unconfirmed`, keeps the record and task bar, and still offers Cancel/reconcile; it does not become a startable second task

### Requirement: Only one install task runs at a time

Starting another install while the task is in `checking`, `starting`, `running`, `cancelling`, `applying`, or `unconfirmed` SHALL focus the existing task instead of starting a second one; retrying a failed task SHALL remain possible with the same task identity.

#### Scenario: Second market install

- **WHEN** a task is checking and the user confirms another entry
- **THEN** the existing task's dialog opens
- **AND** no second install request starts

#### Scenario: Retry after failure

- **WHEN** the dialog shows a failed installation and the user presses Retry
- **THEN** the same task restarts with the current spec and task identity
- **AND** the saved session record is updated for the retry

### Requirement: Existing safety behavior is unchanged

The change SHALL keep the trust confirmation, pinned version and source display, registry selection, explicit cancellation, build-script approval, and sanitized restored-spec display behavior.

#### Scenario: Confirmation still names the source

- **WHEN** the user starts an install from Discover
- **THEN** the confirmation still shows the version, author, source, and trust notice

#### Scenario: Cancellation remains explicit

- **WHEN** the user hides the dialog without pressing Cancel
- **THEN** the task is not reported as cancelled
