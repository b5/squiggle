Purpose: Airtable Clone for Laptop-Scale Data
  * Local 

High Level Requirements:
* keep large amounts of disparate data synced from external sources
* means of scoping data based on facets of a user's identity
* connect facets with others
* permit access to data with others
* sync that shared data with others

--

* [x] Mon Nov 11 - Schema Validate, Write, Create
* [ ] Tue Nov 12 - UI Display Speedrun
  * Paginate through schema Data
  * List Bots
  * Move Bot config to DB, frontend
  * Construct Blob of events
  * Add events from ticket
  * Share Ticket
* [ ] Wed Nov 13 - Simplified Search
* [ ] Thu Nov 14 - Ticket-Based Share flow
* [ ] Fri Nov 15 - Demo for team

--
Initial MVP Flow:
* [ ] List available bots
* [ ] Enable bot
* [ ] Display Run Status
* [ ] Review Local Data
* [ ] Search Local Data
-- --
* [ ] Connect with Friends
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
  * [ ] build out project scores database:
    * [x] add a single event for every entry
  * [ ] app-side schema viewer
* [ ] Schemas are JSON Schemas
* [ ] Events 1
  * [x] Jobs are associated with accounts
  * [ ] bots schema
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