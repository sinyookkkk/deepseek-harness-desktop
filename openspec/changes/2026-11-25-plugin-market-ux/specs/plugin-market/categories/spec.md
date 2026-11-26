## Purpose

Lets a plugin market publish browsable categories and labels in the same index DSH already reads, so the Discover page can group and filter plugins without a client release.

## ADDED Requirements

### Requirement: Index declares categories and labels

A market index SHALL be able to declare category and label ids with localized titles, and each plugin SHALL reference only declared ids.

#### Scenario: Categories render as filters

- **WHEN** an index declares categories `essentials` and `graphics` with Chinese and English titles
- **THEN** the Discover page shows one pill per declared category plus an “All” pill
- **AND** selecting a pill filters the list to plugins whose `metadata.category` matches

#### Scenario: Undeclared id is rejected

- **WHEN** a plugin references a category or label id that the index does not declare
- **THEN** the index build fails with the plugin name and the undeclared id

#### Scenario: Labels filter without regressing search or sort

- **WHEN** an index declares labels `vision` and `search` and plugins carry one or both
- **THEN** the Discover page shows a Labels menu with those declared labels
- **AND** selecting both labels keeps only plugins that carry both
- **AND** text search and every sort option still return the same ordering rules as before the category metadata was added

### Requirement: Category post-processing is deterministic

The category post-processor SHALL leave `public/index.json` byte-identical when the source data and mapping are unchanged, SHALL exit 1 for a data error, and SHALL exit 2 for a missing or malformed input file.

#### Scenario: Repeated run

- **WHEN** the post-processor runs twice with the same index and mapping
- **THEN** the second run writes no file change

#### Scenario: Data error

- **WHEN** a plugin has no mapping or references an undeclared id
- **THEN** the command exits 1 and names the offending plugin or id

### Requirement: Every plugin has an explicit category

The published market index SHALL assign every listed plugin an explicit category from the declared set; an unmapped plugin SHALL fail the build instead of falling back to “other”.

#### Scenario: New source without a mapping

- **WHEN** the source list adds a plugin whose name is absent from the category mapping
- **THEN** the build fails and lists the missing package name

#### Scenario: Complete mapping

- **WHEN** every listed plugin is mapped
- **THEN** the generated index has a `metadata.category` on every plugin
- **AND** the set of categories used by plugins is a subset of the declared categories

### Requirement: “Other” is an explicit whitelist

The build SHALL allow `other` only for package names listed in `otherAllow`, SHALL report each category's plugin count, and SHALL require every non-other declared category to contain at least one plugin.

#### Scenario: Unlisted plugin uses other

- **WHEN** a plugin not in `otherAllow` is mapped to `other`
- **THEN** the build fails and names the plugin

#### Scenario: Other count is reported

- **WHEN** the build succeeds
- **THEN** it prints the plugin count for every category, including the packages placed in `other`

#### Scenario: Empty category

- **WHEN** a declared non-other category has no plugin
- **THEN** the build fails and names the category

#### Scenario: Stale mapping after a plugin is removed

- **WHEN** the mapping contains a package name no longer present in the index
- **THEN** the build warns and continues

### Requirement: Categories are published without a client release

Applying the category metadata SHALL NOT require a new DSH runtime; a refreshed index SHALL be enough for a client that already renders declared categories.

#### Scenario: Refresh after index publication

- **WHEN** the market publishes an index with categories and the user refreshes Discover
- **THEN** the category pills appear without installing a new DSH version
