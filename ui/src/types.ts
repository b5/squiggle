
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