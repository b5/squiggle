~~Airtable Clone for Laptop-Scale Data~~~

Sqgl: 
Squiggle: a free, hackable, local-first version of notion on steriods.
* it's cozy - 
* archival grade
* integrate with anything -
* fully hackable - write programs that extend the full power of webassembly & 

High Level Requirements:
* keep large amounts of disparate data synced from external sources
* means of scoping data based on facets of a user's identity
* connect facets with others
* permit access to data with others
* sync that shared data with others

## Frontend Features
* [ ] Profiles:
  * [ ] List
  * [ ] Create
  * [ ] Switch
  * [ ] Edit
  * [ ] Delete
* [ ] Spaces:
  * [ ] Switch
  * [ ] Create
  * [ ] Delete
  * [ ] Edit
* [ ] Programs
  * [ ] Program Page
  * [ ] Run Program from page
  * [ ] Program start / stop execution feedback
  * [ ] Program stdout feed
  * [ ] Drag-Drop Import 
  * [ ] Create Program Share Ticket
  * [ ] Fetch from ticket paste in searchbar
* [ ] Schemas
  * [ ] File Export: JSON, CSV
  * [ ] Share via ticket
  * [ ] Schemas in sidebar
* [ ] Search:
  * [ ] Clean up Command Bar
  * [ ] 
* [ ] Presence
  * [ ] User Profile Liveness indicators
* [ ] Sync
  * [ ] 

## Backend Features
* [ ] Write Space Events into Spaces
* [ ] Capabilities
  * [ ] Enumerate Capability Systems
* [ ] Sync
  * [ ] Mutation Event Broadcast

## Documentation
* [ ] Outline Design Document
* [ ] Marketing Page Outline

-- 

* [ ] Need a "hard refresh" option, ideally keyboard shortcut
* [x] bundler should move into node

--

* [x] Mon Nov 11 - Schema Validate, Write, Create
* [ ] Tue Nov 12 - UI Display Speedrun
  * [x] Paginate through schema Data
    * [ ] Settings program
* [ ] Wed Nov 13 - Everything is a program
  * [ ] program constructor function in repo
    * [x] node CLI command for building programs
  * [x] become browser
* [ ] Thu Nov 14 - Ticket-Based Share flow
  * [ ] std schemas
  * [ ] Construct Blob of events
  * [ ] Add events from ticket
* [ ] Fri Nov 15 - Demo for team
  * [x] inline content into events table in JSON format 
* [ ] Sunday Nov 17
  * [x] execute programs from iroh collection source
  * [x] List Programs within UI
  * [x] Share Program via ticket
  * [ ] Fetch Program via ticket (in UI)
  * [ ] Execute program
  * [ ] Bullet-point overview of system design
* [ ] Wed Nov 20
  * [ ] Rename "schemas" to "tables", list tables in sidebar
  * [ ] Space Switcher, persist choice
  * [ ] Program configuration
  * [ ] Program ticket sharing flow
--
Initial MVP Flow:
* [ ] List available programs
* [ ] Run Program
* [ ] Display Run Status
* [ ] Review Local Data
* [ ] Search Local Data
-- --
* [ ] One-off sharing

-- --
* [x] Sqlite Events Table
* [x] VM Compiles 
* [x] list accounts on frontend
* [x] Automation 1
  * [x] Hello-world WASM Execution
  * [x] Tasks & Jobs
  * [x] Wire up job kickoff to the frontend
  * [x] interface to write blobs (re-using fog uploads dir)
  * [x] write a task that creates one or more blobs
* App Detour 1:
  * [x] build out project scores database:
    * [x] add a single event for every entry
  * [ ] app-side schema viewer
* [x] Schemas are JSON Schemas
* [ ] Events 1
  * [x] Jobs are associated with accounts
  * [x] bots schema
  * [ ] jobs schema
  * [ ] create event on job run
  * [ ] interface to write events
  * [ ] written from a job get a collection
* [ ] Data Modeling 1
  * [ ] URL-as-anchor datums
  * [ ] Mechanism for Grouping Datums
  * [ ] People-Specific Data Modeling
    * [ ] Github User Data Example
    * [ ] Twitter User Data Example
  * [ ] Reduce function that constructs account context?
* [ ] App Detour 2
  * [ ] encode existing notion table as jsonschema:
      * can we just use notion's schema for this?
      * let's start with some simple illustration that adheres to jsonschema
      * [ ] add schema as a collection: 1th blob is schema, prior schemas are 2-Nth blobs
* [ ] Sync 1
  * Connect & Sync All events
* [ ] Search Index
* [ ] Automation 2
  * [ ] List Bots on Frontend
  * [ ] List Jobs on Frontend
  * [ ] Run user-selectable Job from Frontend
* [ ] Multi-tenancy
  * [ ] Account Creation
  * [ ] 
* [ ] Access Control 1
  * [ ] 
* [ ] Blob Persistence
* [x] Pass config to bots
* [ ] Store Secrets for bots
* [ ] set up some sort of auto-update for tauri UI