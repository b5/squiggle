
export interface Space {
  name: string;
}

export interface User {

}

export interface Program {
}

export interface HashLink {
  hash: string;
  value?: any;
}

export interface Schema {
  name: string;
  description: string;
  content: HashLink;
}

export interface Row {
  content: HashLink;
}

export type Tag = [string, string, string?];

export enum EventKind {
  MutateUser = 100000,
  DeleteUser = 100001,
  MutateSpace = 100002,
  DeleteSpace = 100003,
  MutateProgram = 100004,
  DeleteProgram = 100005,
  MutateSchema = 100006,
  DeleteSchema = 100007,
  MutateRow = 100008,
  DeleteRow = 100009,
}

export interface Event {
  id: string,
  pubkey: string,
  createdAt: number,
  kind: EventKind,
  tags: [Tag],
  content: HashLink,
}