import { useEffect, useState } from "react";
import { invoke, InvokeArgs } from "@tauri-apps/api/core";

import { User, Program, Schema, Row, Space } from "@/types";

export interface SpaceParam {
  space: string;
}

export interface Pagination {
  offset?: number;
  limit?: number;
}

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

export const useListSpaces = ApiFactory<Pagination, [Space]>("spaces_list");
export const useQueryUsers = ApiFactory<SpaceParam & Pagination, [User]>("users_list");
export const useQueryPrograms = ApiFactory<SpaceParam & Pagination, [Program]>("programs_list");
export const useRunProgram = ApiFactory<SpaceParam & { id: string }, Program>("programs_run");
export const useQuerySchemas = ApiFactory<SpaceParam & Pagination, [Schema]>("schemas_list");
export const useQuerySchema = ApiFactory<SpaceParam & { schema: string }, Schema>("schemas_get");
export const useQueryRows = ApiFactory<SpaceParam & { schema: string } & Pagination, [Row]>("rows_query");
