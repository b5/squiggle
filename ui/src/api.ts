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

export interface Pagniation {
  offset?: number;
  limit?: number;
}

export interface Schema {
  hash: string;
  name: string;
  description: string;
}

export interface Event {
  hash: string;
  data: string;
}

export const useListSchemas = ApiFactory<Pagniation, [Schema]>("schemas_list");
export const useQueryEvents = ApiFactory<{ schema: string, offset: number, limit: number }, [Event]>("events_query");