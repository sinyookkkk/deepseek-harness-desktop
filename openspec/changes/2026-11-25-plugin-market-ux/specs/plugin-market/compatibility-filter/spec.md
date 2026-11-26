## Purpose

Keeps plugins that cannot run on the current DSH out of the default Discover results, while keeping them reachable through an explicit, counted reveal so a missing entry never looks like a broken index.

## ADDED Requirements

### Requirement: Default market results contain only installable plugins

A market search SHALL exclude every plugin whose listed versions are all incompatible with the running DSH and not covered by a profile version exemption, and SHALL report how many matching plugins it excluded.

#### Scenario: Hidden when no version runs

- **WHEN** a plugin lists only versions whose `engines.dsh` rejects the running DSH and the profile holds no exemption for the exact pair
- **THEN** the default `pluginMarket.search` result omits that plugin from `entries` and from `total`
- **AND** the result's `incompatible` count includes it

#### Scenario: Kept when one version runs

- **WHEN** a plugin lists one compatible version and one incompatible version
- **THEN** the plugin appears in the default result with the compatible version as its default

#### Scenario: Exemption keeps the plugin visible

- **WHEN** every version of a plugin is incompatible but the profile holds an exemption for the running DSH and the installed version
- **THEN** the plugin appears in the default result

#### Scenario: Undeclared requirement means any version

- **WHEN** a plugin's versions declare no `engines.dsh`
- **THEN** the plugin appears in the default result

### Requirement: Paging counts only visible plugins

Offset, limit, `total`, and `nextOffset` SHALL be computed after the compatibility filter, so pages stay contiguous and a page is never shorter than the remaining visible matches.

#### Scenario: Filter before paging

- **WHEN** the first three matches include two uninstallable plugins and `limit` is 2
- **THEN** the first page returns the first two visible plugins
- **AND** the page's `total` counts visible matches only
- **AND** the next page continues after the visible entries, without repeating or skipping one

### Requirement: Incompatible plugins are exactly the uninstallable ones

A plugin SHALL be treated as incompatible only when no listed version is either compatible or exempted; the Host SHALL evaluate this with the same requirement check that guards installation.

#### Scenario: Compatibility is not re-derived

- **WHEN** the Host filters a plugin whose version range satisfies the running DSH
- **THEN** the plugin is visible even if a registry or Git source would later fail to install

### Requirement: The client reports and reveals hidden plugins

The Discover page SHALL show the number of hidden incompatible matches and SHALL let the user reveal them on demand.

#### Scenario: Hidden count is shown

- **WHEN** a search excludes at least one incompatible plugin
- **THEN** the result list shows a note naming how many plugins were hidden
- **AND** the note offers a control that reveals them

#### Scenario: Revealed entries stay blocked

- **WHEN** the user reveals incompatible plugins
- **THEN** they appear in the list with an “Incompatible” tag
- **AND** their Install action stays disabled
- **AND** the note offers a control that hides them again

#### Scenario: Nothing to reveal

- **WHEN** the search excludes no plugin
- **THEN** the note and its control are absent

#### Scenario: Empty result caused by the filter

- **WHEN** every match is incompatible so the visible result is empty
- **THEN** the page still shows the hidden count and the reveal control instead of only “no matches”

#### Scenario: Reveal survives query changes

- **WHEN** the user reveals incompatible plugins and then changes the search text, category, labels, sort, or index
- **THEN** the revealed state is kept in the query sent to the Host

### Requirement: The agent listing reports the same exclusions

The `plugin_market` tool SHALL apply the default filter and SHALL include the hidden count in its `list_plugins` result.

#### Scenario: Listing plugins for an agent

- **WHEN** an index lists an incompatible plugin and the model lists that index
- **THEN** the result's `entries` omit it
- **AND** the result reports how many matches were incompatible
