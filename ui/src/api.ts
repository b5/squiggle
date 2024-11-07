import { useState } from "react";
import { invoke, InvokeArgs } from "@tauri-apps/api/core";

export interface ApiEnvelope<O> {
  isLoading: boolean;
  data?: O;
}

function ApiFactory<I, O>(method_name: string): ((i: I) => ApiEnvelope<O>) {
  return function(input: I) {
    const [envelope, setEnvelope] = useState<ApiEnvelope<O>>({
      isLoading: false,
    });
    invoke<O>(method_name, input as InvokeArgs).then((res) => {
      console.log(res);
      setEnvelope({ isLoading: false, data: res });
    });
    return envelope;
  }
}

export interface Pagniation {
  offset?: number;
  limit?: number;
}

export interface SchemaItem {
  id: string;
  name: string;
  description: string;
}

export const useListSchemas = ApiFactory<Pagniation, [SchemaItem]>("schemas_list");