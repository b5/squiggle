const { print, event_create, event_mutate, event_query } = Host.getFunctions();

export function log(s: string) {
  const mem = Memory.fromString(s);
  print(mem.offset);
  mem.free();
}

export function query(schema: string, query: string): Entry[] {
  const s = Memory.fromString(schema);
  const q = Memory.fromString(query);
  const offset = event_query(s.offset, q.offset);
  s.free();
  q.free();
  return Memory.find(offset).readJsonObject();
}

export interface Entry {
  id: string;
  schema: string;
  data: any;
}

export function addEntry(schema: string, entry: any): Entry {
  const s = Memory.fromString(schema);
  const d = Memory.fromJsonObject(entry);
  const offset = event_create(s.offset, d.offset);
  s.free();
  d.free();
  return Memory.find(offset).readJsonObject();
}

export function updateEntry(schema: string, id: string, entry: any): Entry {
  const s = Memory.fromString(schema);
  const i = Memory.fromString(id);
  const d = Memory.fromJsonObject(entry);
  const offset = event_mutate(s.offset, i.offset, d.offset);
  s.free();
  i.free();
  d.free();
  return Memory.find(offset).readJsonObject();
}