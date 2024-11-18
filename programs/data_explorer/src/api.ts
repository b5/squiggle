import { useEffect, useState } from "react";
import { invoke, InvokeArgs } from "@tauri-apps/api/core";

export interface ApiEnvelope<O> {
  isLoading: boolean;
  data?: O;
}

function ApiFactory<I, O>(method_name: string): ((i: I) => ApiEnvelope<O>) {
  return function(input: I) {
    const [envelope, setEnvelope] = useState<ApiEnvelope<O>>({
      isLoading: true,
    });

    useEffect(() => {
      invoke<O>(method_name, input as InvokeArgs).then((res) => {
        setEnvelope({ isLoading: false, data: res });
      });
    }, []);

    return envelope;
  }
}

export interface Pagination {
  offset?: number;
  limit?: number;
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

export const useQuerySchemas = ApiFactory<Pagination, [Schema]>("schemas_list");
export const useQuerySchema = ApiFactory<{ schema: string }, Schema>("schemas_get");
export const useQueryRows = ApiFactory<{ schema: string } & Pagination, [Row]>("rows_query");